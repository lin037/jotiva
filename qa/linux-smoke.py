"""Exercise the real WebKitGTK application on a private X11 or Wayland desktop."""
from contextlib import ExitStack, contextmanager
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time

project = Path(__file__).resolve().parents[1]
native = project / "src-tauri"
target = Path(os.environ.get("CARGO_TARGET_DIR", native / "target")).resolve()
cache = Path.home() / ".cache" / "jotiva-qa"
cache.mkdir(parents=True, exist_ok=True)
wayland = "--wayland" in sys.argv


@contextmanager
def child(args, env, **kwargs):
    process = subprocess.Popen(args, env=env, **kwargs)
    try:
        yield process
    finally:
        if process.poll() is None:
            process.terminate()
        process.wait(timeout=10)


def drag_caption(pid, point, env):
    windows = subprocess.check_output(
        ["xdotool", "search", "--all", "--onlyvisible", "--pid", str(pid), "--name", "^" + point["title"] + "$"],
        env=env, text=True,
    )
    window = windows.splitlines()[-1]
    subprocess.run([
        "xdotool", "windowactivate", "--sync", window, "mousemove", "--window", window,
        str(point["x"]), str(point["y"]), "mousedown", "1", "sleep", "0.15",
        "mousemove_relative", "--", "8", "6", "sleep", "0.15", "mousemove_relative", "--", "50", "30",
        "sleep", "0.15", "mouseup", "1",
    ], env=env, check=True, timeout=5)


with tempfile.TemporaryDirectory(dir=cache) as directory, ExitStack() as stack:
    work = Path(directory)
    dist = work / "dist"
    shutil.copytree(project / "dist", dist)
    shutil.copyfile(project / "qa/desktop-probe.js", dist / "probe.js")
    shutil.copyfile(project / "qa/terminal-probe.js", dist / "terminal-probe.js")
    host_title = f"Jotiva-QA-{os.getpid()}"
    scenario = "wayland=1" if wayland else "host=" + host_title
    page = dist / "index.html"
    page.write_text(page.read_text().replace("</body>", f'<script type="module" src="/probe.js?{scenario}"></script></body>'))
    doc = work / "跨平台验收.md"
    doc.write_text("# smoke\n")
    library = work / "library"
    nested = library / "posts"
    nested.mkdir(parents=True)
    (nested / "a ' $ file.md").write_text("jotiva-drop-proof\n")
    (work / "profile").mkdir(exist_ok=True)
    (work / "profile/workspace.json").write_text(json.dumps([str(library)]))
    config = {"identifier": f"io.github.lin037.jot.desktop.qa{os.getpid()}",
              "build": {"frontendDist": os.path.relpath(dist, native)},
              "app": {"appDirectoriesOverride": str(work / "profile")}}
    env = dict(os.environ, TAURI_CONFIG=json.dumps(config))
    subprocess.run(["cargo", "build", "--locked", "--target-dir", str(target), "--features", "tauri/custom-protocol"],
                   cwd=native, env=env, check=True)

    host = None
    if wayland:
        runtime = work / "runtime"
        runtime.mkdir(mode=0o700)
        env.update(GDK_BACKEND="wayland", XDG_RUNTIME_DIR=str(runtime), WAYLAND_DISPLAY="wayland-qa")
        # Weston obtains a virtual seat from Xvfb. The app itself gets only the Wayland socket.
        stack.enter_context(child(["weston", "--backend=x11", "--renderer=pixman", "--socket=wayland-qa",
                                   "--idle-time=0", "--log=" + str(work / "weston.log")], env))
        env.pop("DISPLAY", None)
        for _ in range(100):
            if (runtime / "wayland-qa").is_socket():
                break
            time.sleep(0.05)
        else:
            raise RuntimeError((work / "weston.log").read_text())
    else:
        env["GDK_BACKEND"] = "x11"
        stack.enter_context(child(["openbox"], env, stdout=subprocess.DEVNULL))
        host = stack.enter_context(child(["zenity", "--info", "--title", host_title, "--text", host_title], env))
        subprocess.run(["xdotool", "search", "--sync", "--onlyvisible", "--name", "^" + host_title + "$"],
                       check=True, timeout=10, env=env, stdout=subprocess.DEVNULL)

    stack.enter_context(child(["thunar", "--daemon"], env))
    log = work / "filemanager.log"
    stream = stack.enter_context(log.open("w"))
    match = "type='method_call',interface='org.freedesktop.FileManager1',member='ShowItems'"
    stack.enter_context(child(["stdbuf", "-oL", "dbus-monitor", "--session", match], env, stdout=stream))
    app = stack.enter_context(child([target / "debug/Jotiva", doc], env))
    deadline = time.monotonic() + 70
    opened_menu = False
    dragged = False
    while time.monotonic() < deadline:
        results = {json.loads(record.read_bytes())["markdown"] for record in (work / "profile/notes").glob("[0-9]*.json")}
        errors = [line for line in results if line.startswith("# QA error")]
        if errors:
            raise RuntimeError(errors)
        if host and "# QA attached" in results and host.poll() is None:
            host.terminate()
        if not wayland and "# QA menu" in results and not opened_menu:
            time.sleep(0.3)
            subprocess.run(["xdotool", "key", "--delay", "90", "Home", *["Down"] * 6,
                            "Right", "Home", "Return"], env=env, check=True, timeout=5)
            opened_menu = True
        drag = next((s for s in results if s.startswith("# QA drag ")), None)
        if drag and not dragged:
            drag_caption(app.pid, json.loads(drag.removeprefix("# QA drag ")), env)
            dragged = True
        if {"# QA settings", "# QA main", "# QA document"} <= results:
            break
        if app.poll() is not None:
            raise RuntimeError(f"Application exited early with {app.returncode}")
        time.sleep(0.25)
    else:
        raise TimeoutError("WebKitGTK probe timed out: " + repr(results))
    assert app.wait(timeout=12) == 0, "Confirmed quit did not exit cleanly"
    shell_pid = int((nested / "terminal-pid.txt").read_text())
    assert shell_pid > 0 and not Path(f"/proc/{shell_pid}").exists(), "Closed terminal shell is still running"
    assert (work / "terminal-context.txt").read_text().splitlines() == [str(work), str(work / "profile/notes/其他")]
    assert "Desktop edit" in doc.read_text()
    assert doc.as_uri() in log.read_text(), "Missing FileManager1.ShowItems for the test document"
    print("WebKitGTK passed on", env["GDK_BACKEND"], "with Markdown save, caption controls, multiple windows and file-manager reveal.")
