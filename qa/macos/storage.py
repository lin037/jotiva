"""Check actual WebKit edits, filesystem conflicts and recovery across two Jotiva processes."""

from contextlib import contextmanager
import json
from pathlib import Path
import subprocess
import tempfile
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


def exercise(work):
    data = work / "data"
    data.mkdir()
    root = work / "workspace"
    root.mkdir()
    archive = root / "archive"
    archive.mkdir()
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
    identifier = f"io.github.lin037.jot.qa.storage{time.time_ns():x}"
    binary = build_test_app(work, identifier, 1, query="storage=1", data=data)
    with launch(binary) as app:
        opened = stage(data, app, "storage-open")
        assert Path(opened["path"]).samefile(disk) and "edited-in-Jot" in disk.read_text()
        disk.write_text("preserve-this-disk-version")
        (root / "changed.md").touch()
        original = Path(stage(data, app, "storage-archive")["path"])
        assert original.parent.samefile(data / "notes/其他")
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
    print("Passed legacy-note loading, native edit, conflict recovery, archive failure and restart.")


if __name__ == "__main__":
    TARGET.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=TARGET.parent, prefix="jot-native-storage-") as temporary:
        exercise(Path(temporary).resolve())
