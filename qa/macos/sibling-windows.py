"""Exercise new attached note through its real menu with the main window closed."""
import json
from pathlib import Path
import select
import shutil
import subprocess
import argparse
import tempfile
import time

from run import HERE, build_test_app


def check(work, data, binary, keep_main):
    subprocess.run(["xcrun", "clang", "-fobjc-arc", "-Wno-deprecated-declarations", str(HERE / "fixture.m"),
                    "-framework", "AppKit", "-framework", "ApplicationServices", "-framework", "Carbon",
                    "-o", str(work / "fixture")], check=True)
    data.mkdir()
    (data / "settings.json").write_text(json.dumps({"quick": "Control+Shift+KeyU", "search": None, "dock": None}))
    fixture = None
    app = subprocess.Popen([binary])
    def stage(kind):
        end = time.monotonic() + 34
        while time.monotonic() < end and app.poll() is None:
            for path in (data / "notes").glob("*.json"):
                text = json.loads(path.read_bytes()).get("markdown", "")
                tag, _, payload = text.partition("\n")
                if tag == "#probe:error":
                    raise AssertionError(payload)
                if tag == "#probe:" + kind:
                    return json.loads(payload)
            time.sleep(.07)
        raise TimeoutError(kind)

    try:
        stage("main-ready")
        fixture = subprocess.Popen([work / "fixture", str(app.pid), "u"], stdin=subprocess.PIPE,
                                   stdout=subprocess.PIPE, text=True)
        def response():
            if not select.select([fixture.stdout], [], [], 5)[0]:
                raise TimeoutError("Host fixture")
            return fixture.stdout.readline().strip()
        def send(cmd):
            fixture.stdin.write(cmd + "\n")
            fixture.stdin.flush()
        assert response().endswith(" 1"), "macOS keyboard access required"
        send("A")
        time.sleep(.45)
        send("status")
        assert response() == f"status {fixture.pid} Fixture A"
        send("s")
        assert response().startswith("key posted ")
        pair = stage("pair")
        assert "Fixture A" in pair["source"], pair
        def stack():
            send("note-stack")
            return response().split(",")
        order = stack()
        assert order.count("Jotiva 便签") == 2, order
        if keep_main:
            assert order.index("A") < order.index("Jotiva"), order
        else:
            assert "Jotiva" not in order, order
        paths = [Path(file) for file in pair["paths"]]
        assert [p.read_text() for p in paths] == ["original-note", "second-note"]
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
    keep_main = cli.parse_args().main_open
    identifier = f"io.github.lin037.jot.qa.siblings{time.time_ns():x}"
    with tempfile.TemporaryDirectory(prefix="jot-siblings-") as directory:
        work = Path(directory)
        binary = build_test_app(work, identifier, 1, probe="sibling-windows.js", query="main=open" if keep_main else "main=closed")
        data = Path.home() / "Library/Application Support" / identifier
        assert not data.exists()
        try:
            check(work, data, binary, keep_main)
        finally:
            if data.exists():
                shutil.rmtree(data)
