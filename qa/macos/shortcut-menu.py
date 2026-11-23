"""Exercise AppKit key equivalents, recording and Carbon registration in an isolated Jotiva."""
import json
from pathlib import Path
import tempfile
import time

from run import HERE, TARGET, build_test_app
from storage import launch, stage


def exercise(work):
    data = work / "data"
    data.mkdir()
    (data / "settings.json").write_text(json.dumps({"quick": "Control+Shift+F18", "search": None, "dock": None}))
    identifier = f"io.github.lin037.jot.qa.shortcuts{time.time_ns():x}"
    binary = build_test_app(work, identifier, 1, probe="shortcut-menu.js", data=data,
                            observer=HERE / "shortcut-menu.rs", native_probe=HERE / "shortcut-menu.m")
    with launch(binary) as app:
        stage(data, app, "remapped")
    assert json.loads((data / "settings.json").read_text())["local"] == {"settings": "Super+KeyJ"}
    with launch(binary) as app:
        stage(data, app, "restored")
    assert json.loads((data / "settings.json").read_text())["local"] == {"settings": None}
    print("AppKit recording, menu rebind, Carbon conflict rollback, restart and unbinding passed.")


if __name__ == "__main__":
    TARGET.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=TARGET.parent, prefix="jot-native-keys-") as temporary:
        exercise(Path(temporary).resolve())
