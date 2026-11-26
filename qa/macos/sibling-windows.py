"""Exercise native sibling note windows and restart recovery on macOS."""
import json
from pathlib import Path
import select
import subprocess
import argparse
import tempfile
import time

from run import HERE, TARGET, build_test_app


def check(work, data, binary, keep_main, repeat_shortcut, restart):
    subprocess.run(["xcrun", "clang", "-fobjc-arc", "-Wno-deprecated-declarations", str(HERE / "fixture.m"),
                    "-framework", "AppKit", "-framework", "ApplicationServices", "-framework", "Carbon",
                    "-o", str(work / "fixture")], check=True)
    data.mkdir()
    shortcut = "Control+Shift+Space" if repeat_shortcut else "Control+Shift+KeyU"
    (data / "settings.json").write_text(json.dumps({"quick": shortcut, "search": None, "dock": None}))
    fixture = None
    app = subprocess.Popen([binary])
    def stage(kind):
        end = time.monotonic() + 34
        while time.monotonic() < end:
            for path in (data / "notes").glob("*.json"):
                text = json.loads(path.read_bytes()).get("markdown", "")
                tag, _, payload = text.partition("\n")
                if tag == "#probe:error":
                    raise AssertionError(payload)
                if tag == "#probe:" + kind:
                    return json.loads(payload)
            if app.poll() is not None:
                break
            time.sleep(.07)
        raise TimeoutError(kind)

    try:
        stage("main-ready")
        fixture = subprocess.Popen([work / "fixture", str(app.pid), "space" if repeat_shortcut else "u"], stdin=subprocess.PIPE,
                                   stdout=subprocess.PIPE, text=True)
        def response():
            if not select.select([fixture.stdout], [], [], 5)[0]:
                raise TimeoutError("Host fixture")
            return fixture.stdout.readline().strip()
        def send(cmd):
            fixture.stdin.write(cmd + "\n")
            fixture.stdin.flush()
        ready = response()
        assert ready.startswith("ready "), ready
        if not restart:
            assert ready.endswith(" 1"), "macOS keyboard access required"
            send("A")
            time.sleep(.45)
            send("status")
            assert response() == f"status {fixture.pid} Fixture A"
            send("s")
            assert response().startswith("key posted ")
            if repeat_shortcut:
                stage("repeat")
                send("s")
                assert response().startswith("key posted ")
        pair = stage("pair")
        assert "Fixture A" in pair["source"], pair
        def stack():
            send("note-stack")
            return response().split(",")
        paths = [Path(file) for file in pair["paths"]]
        assert [p.read_text() for p in paths] == ["original-note", "second-note"]
        if restart:
            assert app.wait(timeout=12) == 0
            send("B")
            app = subprocess.Popen([binary])
            stage("bindings-restored")
            print("Jotiva loaded both saved associations to the same host.", flush=True)

            def restore_stack(host, expected):
                end = time.monotonic() + 8
                while time.monotonic() < end:
                    send("status")
                    focused = response() == f"status {fixture.pid} Fixture {host}"
                    send(f"note-stack {app.pid}")
                    current = response().split(",")
                    if focused and current.count("Jotiva 便签") == expected:
                        return
                    time.sleep(.08)
                raise AssertionError(f"Expected {expected} attached note windows; WindowServer reported {current}")

            for host, count in (("B", 0), ("A", 2), ("B", 0), ("A", 2)):
                send(host)
                restore_stack(host, count)
            assert [p.read_text() for p in paths] == ["original-note", "second-note"]
            print("Both saved attachments displayed on their original host after a Jotiva restart and subsequent switches.", flush=True)
            return
        order = stack()
        assert order.count("Jotiva 便签") == 2, order
        if keep_main:
            assert order.index("A") < order.index("Jotiva"), order
        else:
            assert "Jotiva" not in order, order
        if keep_main:
            send("k")
            assert response().startswith("key posted ")
            stage("reselected")
        subprocess.run([work / "fixture", "close-tab", str(app.pid)], check=True, timeout=5)
        stage("success")
        assert "original-note" in paths[0].read_text() and "+still-editable" in paths[0].read_text()
        assert paths[1].read_text() == "second-note"
        print("Two simultaneously visible native note windows at", pair["positions"],
              "shared Fixture A. WindowServer order:", order, ". Cmd+W closed only the second.", flush=True)
    finally:
        for process in (fixture, app):
            if process and process.poll() is None:
                process.terminate()
                process.wait(timeout=5)


if __name__ == "__main__":
    cli = argparse.ArgumentParser()
    cli.add_argument("--main-open", action="store_true", help="Also check that Jotiva's main window stays behind the host")
    cli.add_argument("--repeat-shortcut", action="store_true", help="Create both notes with the default global shortcut")
    cli.add_argument("--restart", action="store_true", help="Verify both attached windows return after restarting Jotiva")
    options = cli.parse_args()
    keep_main = options.main_open
    identifier = f"io.github.lin037.jot.qa.siblings{time.time_ns():x}"
    TARGET.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=TARGET.parent, prefix="jot-siblings-") as directory:
        work = Path(directory)
        query = "main=open" if keep_main else "main=closed"
        if options.repeat_shortcut:
            query += "&repeat=1"
        if options.restart:
            query += "&restart=1"
        data = work / "data"
        binary = build_test_app(work, identifier, 1, probe="sibling-windows.js", query=query, data=data)
        check(work, data, binary, keep_main, options.repeat_shortcut, options.restart)
