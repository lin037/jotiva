"""Check actual WebKit edits, filesystem conflicts and recovery across two Jotiva processes."""

from contextlib import contextmanager
from http.server import BaseHTTPRequestHandler, HTTPServer
import json
from pathlib import Path
import subprocess
import tempfile
from threading import Thread
import time

from run import TARGET, build_test_app


@contextmanager
def launch(binary):
    app = subprocess.Popen([binary])
    try:
        yield app
    finally:
        if app.poll() is None:
            app.terminate()
            app.wait(timeout=10)


def stage(data, app, name):
    deadline = time.monotonic() + 40
    while app.poll() is None and time.monotonic() < deadline:
        for path in (data / "notes").glob("[0-9]*.json"):
            markdown = json.loads(path.read_bytes())["markdown"]
            tag, _, payload = markdown.partition("\n")
            if tag == "#probe:error":
                raise AssertionError(payload)
            if tag == "#probe:" + name:
                return json.loads(payload)
        time.sleep(.04)
    raise RuntimeError(f"Waiting for {name}, application exit code {app.poll()}")


class ImageUpload(BaseHTTPRequestHandler):
    def do_POST(self):
        self.server.uploads.append((self.path, self.rfile.read(int(self.headers["Content-Length"]))))
        result = json.dumps({"url": f"http://127.0.0.1:{self.server.server_port}/fixture.png"}).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(result)))
        self.end_headers()
        self.wfile.write(result)

    def do_GET(self):
        if self.path != "/fixture.png":
            self.send_error(404)
            return
        body = self.server.uploads[-1][1]
        start = body.index(b"\x89PNG")
        end = body.index(b"IEND", start) + 8
        self.send_response(200)
        self.send_header("Content-Type", "image/png")
        self.end_headers()
        self.wfile.write(body[start:end])

    def log_message(self, *_):
        pass


def exercise(work, server):
    data = work / "data"
    data.mkdir()
    root = work / "workspace"
    root.mkdir()
    archive = root / "archive"
    archive.mkdir()
    blog = root / "blog"
    (blog / "posts").mkdir(parents=True)
    (blog / "posts/entry.md").write_text("Blog image rule fixture")
    (archive / "review.md").write_text("Archive image rule fixture")
    (root / "images.md").write_text("\n\nPhoto fixture")
    disk = root / "meeting.md"
    disk.write_text("original-file")
    removable = root / "removable.md"
    removable.write_text("offline snapshot")
    legacy = data / "notes/其他/旧版测试.md"
    legacy.parent.mkdir(parents=True)
    legacy_markdown = "# 旧版保留笔记\n\n替换应用后继续读取。"
    legacy.write_text(legacy_markdown)
    legacy_record = legacy.parent.parent / "42.json"
    legacy_record.write_text(json.dumps({
        "id": 42, "name": legacy.name, "source": "Jot", "markdown": legacy_markdown,
        "version": 5, "updated": 1, "location": {"folder": None, "path": str(legacy)},
    }))
    legacy_snapshot = legacy_record.read_bytes()
    (data / "workspace.json").write_text(json.dumps([str(root)]))
    (data / "settings.json").write_text(json.dumps({"quick":"Control+Shift+KeyU", "search":None, "dock":None}))
    shared = work / "shared-images"
    shared.mkdir()
    (data / "images.json").write_text(json.dumps({
        "config": {"defaultMode": "fixed", "directory": str(shared.resolve()), "rules": [
            {"path": str(root.resolve()), "mode": "relative", "subfolder": "notes/media"},
            {"path": str(archive.resolve()), "mode": "http"},
        ], "endpoint": f"http://127.0.0.1:{server.server_port}/upload"}, "directories": [str(shared.resolve())],
    }))
    identifier = f"io.github.lin037.jot.qa.storage{time.time_ns():x}"
    binary = build_test_app(work, identifier, 1, query="storage=1", data=data)
    with launch(binary) as app:
        opened = stage(data, app, "storage-open")
        assert Path(opened["path"]).samefile(disk) and "edited-in-Jot" in disk.read_text()
        assert (shared / "inbox.png").is_file()
        assert (root / "notes/media/粘贴 1.png").read_bytes() == (root / "access/粘贴 1.png").read_bytes()
        image_settings = json.loads((data / "images.json").read_text())
        assert str(root.resolve() / "access") in image_settings["directories"]
        assert any(rule["path"] == str(blog.resolve()) and rule["directory"] == "${ruleDir}/../access"
                   for rule in image_settings["config"]["rules"])
        disk.write_text("preserve-this-disk-version")
        (root / "changed.md").touch()
        original = Path(stage(data, app, "storage-archive")["path"])
        assert original.parent.samefile(data / "notes/其他")
        assert f"http://127.0.0.1:{server.server_port}/fixture.png" in (archive / "review.md").read_text()
        assert len(server.uploads) == 1
        route, body = server.uploads[0]
        assert route == "/upload" and b'name="file"' in body and b'\x89PNG\r\n\x1a\n' in body
        (archive / "review.md").unlink()
        archive.rmdir()
        stage(data, app, "storage-saved")
    removable.unlink()
    with launch(binary) as app:
        recovered = stage(data, app, "storage-reopened")
    assert set(recovered) == {"original", "copy", "external", "offline"}
    for item in recovered.values():
        assert Path(item["path"]).read_text() == item["markdown"]
    copy = Path(recovered["copy"]["path"])
    assert copy.parent.samefile(original.parent) and not copy.samefile(original)
    assert "keep-this-local-edit" in copy.read_text() and "edited-in-Jot" in copy.read_text()
    assert Path(recovered["original"]["path"]).samefile(original)
    assert original.read_text() == "归档失败后保留的正文"
    assert Path(recovered["external"]["path"]).samefile(disk)
    assert disk.read_text() == "preserve-this-disk-version"
    assert not removable.exists()
    assert Path(recovered["offline"]["path"]).parent.samefile(original.parent)
    assert recovered["offline"]["markdown"] == "offline snapshot"
    assert legacy.read_text() == legacy_markdown and legacy_record.read_bytes() == legacy_snapshot
    print("Passed settings centering, folder-specific image routing, native screenshot paste, file preview after restart, legacy storage and conflict recovery.")


if __name__ == "__main__":
    TARGET.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=TARGET.parent, prefix="jot-native-storage-") as temporary, HTTPServer(("127.0.0.1", 0), ImageUpload) as server:
        server.uploads = []
        Thread(target=server.serve_forever, daemon=True).start()
        try:
            exercise(Path(temporary).resolve(), server)
        finally:
            server.shutdown()
