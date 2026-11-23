"""Run the macOS window checks in a staged build."""

import argparse
import json
import math
from pathlib import Path
import select
import shutil
import statistics
import subprocess
import tempfile
import time

HERE = Path(__file__).resolve().parent
PROJECT = HERE.parents[1]
TARGET = PROJECT / "qa/.cache/target"
WARM_SAMPLES = 12


def distribution(samples):
    ordered = sorted(value for value, _ in samples)
    return {
        "n": len(ordered),
        "median_ms": round(statistics.median(ordered), 1),
        "p95_ms": ordered[math.ceil(len(ordered) * .95) - 1],
        "samples_ms": [value for value, _ in samples],
    }


def exercise(work, application, data, quick_key, smoke):
    fixture = None
    other_app = None
    last_id = 0
    notes = data / "notes"
    workspace = work / "workspace"
    workspace.mkdir()
    imported = work / "会议归档"
    imported.mkdir()
    imported_note = imported / "已有议程.md"
    imported_note.write_text("已有的资料保持在原路径")
    document = workspace / "meeting.md"
    document.write_text("original-file")
    removed = workspace / "removed.md"
    removed.write_text("document-on-removable-volume")
    data.mkdir(exist_ok=True)
    notes.mkdir()
    previous_text = "# 旧会议笔记\n\n确认资料完整。"
    (notes / "41.json").write_text(json.dumps({
        "id": 41, "version": 1, "source": "Jot", "updated": 1, "markdown": previous_text,
    }))
    (data / "workspace.json").write_text(json.dumps([str(workspace.resolve())]))

    def note_files():
        return sorted((path for path in notes.glob("*.json") if path.stem.isdecimal()),
                      key=lambda path: int(path.stem))
    started = time.monotonic()
    with (work / "jot.log").open("w") as log:
        app = subprocess.Popen([application], stdout=log, stderr=log)

    def wait(kind, timeout=8):
        nonlocal last_id
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline and app.poll() is None:
            for file in note_files():
                identity = int(file.stem)
                if identity <= last_id:
                    continue
                markdown = json.loads(file.read_text())["markdown"]
                if markdown.startswith("#probe:error\n"):
                    raise RuntimeError(markdown)
                if markdown.startswith(f"#probe:{kind}\n"):
                    last_id = identity
                    print(f"Passed {kind}", flush=True)
                    return json.loads(markdown.split("\n", 1)[1])
            time.sleep(.035)
        stages = [json.loads(path.read_text())["markdown"].split("\n", 1)[0] for path in note_files()]
        recorded = [stage for stage in stages if stage.startswith("#probe:")]
        raise RuntimeError(f"Waiting for {kind} after {time.monotonic() - started:.1f}s, app status {app.poll()}, recorded {recorded}")

    def send(command, response=False):
        fixture.stdin.write(command + "\n")
        fixture.stdin.flush()
        if response:
            if not select.select([fixture.stdout], [], [], 4)[0]:
                raise RuntimeError("AppKit fixture stopped responding")
            return fixture.stdout.readline().strip()

    def wait_host(host):
        expected = f"status {fixture.pid} Fixture {host}"
        deadline = time.monotonic() + 3
        while send("status", True) != expected:
            if time.monotonic() >= deadline:
                raise RuntimeError(f"Fixture {host} did not receive focus")
            time.sleep(.05)

    def capture_time(host="A"):
        wait_host(host)
        sent = send("s", True)
        assert sent.startswith("key posted "), sent
        recorded = wait("sample")
        assert f"Fixture {host}" in recorded["source"], recorded
        elapsed = recorded["measured"] - float(sent.split()[2])
        assert 0 <= elapsed < 3000, elapsed
        time.sleep(.15)
        return round(elapsed, 1), recorded["page"]

    def wait_quick_closed(host, note_id):
        deadline = time.monotonic() + 3
        while send("quick-window", True) != "quick closed":
            if time.monotonic() >= deadline:
                saved = json.loads((notes / f"{note_id}.json").read_bytes())
                raise RuntimeError(f"Quick still present for {note_id}, source={saved.get('source')!r}, "
                                   f"bound={bool(saved.get('origin'))}, {send('status', True)}, "
                                   f"{send('geometry', True)}, {send('menu-close', True)}")
            time.sleep(.05)
        wait_host(host)

    try:
        opened = wait("disk-saved", timeout=35)
        assert Path(opened["file"]).samefile(document)
        assert "edited-in-Jot" in document.read_text()
        document.write_text("changed-by-another-editor")
        # Deliver the same kAEOpenDocuments event Finder sends, targeting this isolated test PID.
        companion = document.with_name("finder-second.md")
        companion.write_text("second-file-from-Finder")
        subprocess.run([work / "fixture", "open-markdown", str(app.pid), str(document), str(companion)], check=True, timeout=10)
        wait("document-mode-ready")
        for mode in ("source", "live-preview"):
            subprocess.run([work / "fixture", "document-mode", str(app.pid)], check=True, timeout=5)
            wait("document-mode-" + mode)
        wait("disk-reloaded")
        assert "close-saved" in companion.read_text()
        document.write_text("preserve-this-disk-version")
        (workspace / "changed.md").touch()
        wait("unlink-document", timeout=25)
        removed.unlink()
        created = wait("folder-create", timeout=28)
        new_file, archived = Path(created["file"]), Path(created["archive"])
        assert new_file.parent.samefile(workspace) and archived.parent.samefile(imported)
        assert new_file.read_text() == "目录 A📌" and archived.read_text() == "归档 B📎"
        assert Path(created["existing"]).samefile(imported_note) and imported_note.read_text() == "已有的资料保持在原路径"
        inbox, migrated = Path(created["inbox"]), Path(created["migrated"])
        assert inbox.parent.samefile(notes / "其他") and migrated.parent.samefile(inbox.parent)
        assert inbox.read_text() == "草稿 C📃" and migrated.read_text() == previous_text
        destination = wait("receive-files")
        dragged = [work / name for name in ("外部拖入 A.md", "外部拖入 B.markdown")]
        for file in dragged:
            file.write_text("native file drag " + file.name)
        subprocess.run([work / "file-drag", str(destination["x"]), str(destination["y"]), *dragged], check=True, timeout=8)
        wait("close-active-tab")
        subprocess.run([work / "fixture", "close-tab", str(app.pid)], check=True, timeout=5)
        initial = wait("ready", timeout=28)
        opened_paths = {doc["file"] for entry in note_files() if (doc := json.loads(entry.read_bytes())).get("file")}
        for file in dragged:
            assert str(file.resolve()) in opened_paths and file.read_text() == "native file drag " + file.name
        assert not removed.exists()
        assert document.read_text() == "preserve-this-disk-version"
        assert initial["outlinePassed"]
        assert initial["paneWidthsPassed"] and initial["settingsPassed"]
        assert initial["trusted"], "Allow the test application to observe its fixture windows"
        assert initial["shortcutError"] is None, initial["shortcutError"]
        binding = "Control+Shift+KeyU" if quick_key == "u" else "Control+Shift+Space"
        assert initial["preferences"] == {"quick": binding, "search": None, "dock": None, "local": {}}
        other_app = subprocess.Popen([work / "host-focus", "host"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
        assert select.select([other_app.stdout], [], [], 5)[0] and other_app.stdout.readline().strip() == "ready"
        other_app.stdin.write("I\n")
        other_app.stdin.flush()
        assert select.select([other_app.stdout], [], [], 4)[0]
        remote_window = int(other_app.stdout.readline())
        fixture = subprocess.Popen(
            [work / "fixture", str(app.pid), quick_key], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True,
        )
        assert select.select([fixture.stdout], [], [], 5)[0]
        fixture_status = fixture.stdout.readline().strip()
        assert fixture_status.endswith(" 1"), "The fixture needs keyboard event permission"
        send("A")
        time.sleep(.35)
        first = capture_time()
        timings = {"first_ms": first[0]}
        host = "A"
        if not smoke:
            warm = [capture_time() for _ in range(WARM_SAMPLES)]
            assert all(page == first[1] for _, page in warm)
            print("Warm capture", distribution(warm), flush=True)
            send("B")
            time.sleep(16)
            send("A")
            time.sleep(.25)
            recreated = capture_time()
            assert recreated[1] != first[1]
            host = "B"
            send(host)
            time.sleep(16)
            send(host)
            from_tray = capture_time(host)
            assert from_tray[1] != recreated[1]
            timings.update(warm=distribution(warm), recreated_with_summary_ms=recreated[0], tray_without_window_ms=from_tray[0])
        assert send("s", True).startswith("key posted ")
        drag = wait("drag-screen", timeout=15)
        assert send(f"expose {drag['target']}", True) == "exposed"
        time.sleep(.25)
        result = send(f"drag {drag['x']} {drag['y']} {drag['target']}", True)
        assert result == "drag sent", (result, drag)
        switched = wait("switch-drag", timeout=18)
        assert send(f"switchdrag {switched['x']} {switched['y']} {switched['target']}", True) == "drag sent"
        attachment = wait("reattached", timeout=18)
        assert attachment["target"] != host
        host = attachment["target"]
        send(host)
        wait_host(host)
        assert send("s", True).startswith("key posted ")
        quick_drag = wait("quick-drag-ready")
        host = quick_drag["target"]
        assert send(f"expose-away {quick_drag['x']} {quick_drag['y']} {host}", True) == "exposed"
        drag_status = send(f"drag {quick_drag['x']} {quick_drag['y']} {host}", True)
        assert drag_status == "drag sent", (drag_status, quick_drag, send("geometry", True), send("status", True))
        layout = wait("layout-ready")
        def geometry():
            value = send("geometry", True).split()
            return tuple(map(float, value[1:])) if len(value) == 5 else None
        before = geometry()
        assert before and send("resize", True) == "resize sent", before
        time.sleep(.3)
        resized = geometry()
        assert resized and abs(before[2] - resized[2]) > 20 and abs(before[3] - resized[3]) > 20, (before, resized)
        assert send(f"shift-host {host}", True) == "host moved"
        time.sleep(.25)
        assert geometry() == resized, "Moving the host repositioned the floating note"
        sibling = "A" if host == "B" else "B"
        send(sibling)
        wait_host(sibling)
        time.sleep(.35)
        assert send(f"click-host {host}", True) == "clicked"
        wait_host(host)
        deadline = time.monotonic() + 3
        returned = geometry()
        while returned != resized and time.monotonic() < deadline:
            time.sleep(.05)
            returned = geometry()
        assert returned and all(abs(a-b) < 2 for a,b in zip(resized, returned)), (resized, returned)
        wait_host(host)  # Passive note presentation leaves typing in the clicked document.
        print("Passed passive mouse return with original note size and keyboard focus", flush=True)
        assert send("s", True).startswith("key posted ")
        native = wait("native-close")
        assert native["id"] == layout["id"]
        assert native["id"] == attachment["id"]
        assert send("quick-window", True) == "quick present"
        sent = send("native-close", True)
        assert sent.startswith("native close 0 "), sent
        assert 0 <= float(sent.split()[3]) - native["typedAt"] < 650
        wait_quick_closed(host, native["id"])
        saved_path = notes / f'{native["id"]}.json'
        assert "save-through-close" in json.loads(saved_path.read_text())["markdown"]
        assert send("s", True).startswith("key posted ")
        keyboard = wait("key-close")
        assert keyboard["id"] != native["id"] and keyboard["page"] != native["page"]
        assert "origin" not in json.loads(saved_path.read_text())
        saved_path = notes / f'{keyboard["id"]}.json'
        sent = send("w", True)
        assert sent.startswith("key posted "), sent
        assert 0 <= float(sent.split()[2]) - keyboard["typedAt"] < 650
        wait_quick_closed(host, keyboard["id"])
        assert "save-through-key-close" in json.loads(saved_path.read_text())["markdown"]
        assert send("s", True).startswith("key posted ")
        try:
            reopened = wait("reopened-after-close")
        except RuntimeError as error:
            views = (send("status", True), send("geometry", True), send("menu-close", True))
            recent = [(p.stem, json.loads(p.read_bytes())["markdown"][:45]) for p in note_files()[-5:]]
            raise RuntimeError(f"{error}; reopened window {views}, latest notes {recent}") from error
        assert reopened["id"] not in (native["id"], keyboard["id"]) and reopened["page"] != keyboard["page"]
        assert "origin" not in json.loads(saved_path.read_text())
        assert reopened["source"].endswith(f"Fixture {host}")
        cross = wait("cross-host-query", timeout=25)
        assert cross["from"].endswith("Fixture " + host) and cross["to"].endswith("Focus-B"), cross
        assert send("front-host", True) == host, cross
        assert send("return", True).startswith("key posted ")
        try:
            opened_remote = wait("cross-host-opened")
        except RuntimeError as error:
            raise RuntimeError(f"{error}; keyboard {send('status', True)}, layer {send('front-host', True)}, "
                               f"note {send('geometry', True)}, expected {other_app.pid} {remote_window}") from error
        assert opened_remote["id"] == cross["id"]
        assert send("front-host", True) == f"{other_app.pid} {remote_window}", cross
        assert send("geometry", True).startswith("frame "), "Recalled note is obscured"
        assert send("k", True).startswith("key posted ")
        assert wait("close-search")["id"] == reopened["id"]
        assert send("w", True).startswith("key posted ")
        assert wait("search-close-kept-note")["id"] == reopened["id"]
        folded = wait("folded-drag")
        assert send(f"movecard {folded['x']} {folded['y']}", True) == "drag sent"
        quitting = wait("quit")
        sent = send("q", True)
        assert sent.startswith("key posted "), sent
        assert 0 <= float(sent.split()[2]) - quitting["typedAt"] < 650
        assert app.wait(timeout=6) == 0
        assert quitting["newShortcutWorked"]
        saved = json.loads((notes / f'{quitting["id"]}.json').read_text())
        assert "save-through-quit" in saved["markdown"]
        return {
            **timings,
            "startup_webviews": initial["windows"]["webviews"],
            "local_shortcuts": ["Command+K", "Escape", "Command+Z", "Command+W", "Command+N", "Command+Q", "Command+/"],
            "outline": True,
            "pane_widths": True,
            "settings_button": True,
            "reattach_existing_note": True,
            "markdown_in_workspace_archive_and_inbox": True,
            "external_file_drop_opened_two_tabs": True,
            "command_w_closed_only_active_tab": True,
            "existing_markdown_opened_in_place": True,
            "legacy_draft_restored_as_markdown": True,
            "first_title_updates_visible_inbox_path": True,
            "disk_file_reopened_by_apple_event": True,
            "multi_file_finder_open_and_document_close_saved": True,
            "native_mode_cycle_exactly_once_per_key": True,
            "preserved_conflicting_edits": True,
            "missing_file_recovered_in_single_editable_copy": initial["missingFileRecovered"],
            "list_geometry": initial["listGeometryPassed"],
            "source_file_and_text_drops_and_recall": attachment["fileAndTextDropped"],
            "native_drag_to_external_window": attachment["nativeDrag"],
            "native_drag_from_quick_window": True,
            "native_folded_drag_and_file_rename": True,
            "search_raised_other_host_and_its_note": True,
            "closing_search_preserved_attachment_and_editing": True,
            "host_motion_keeps_note_screen_position": True,
            "native_close_released_window": True,
            "command_w_released_window": True,
            "undo_preserved": True,
            "close_saved": True,
            "quit_saved": True,
        }
    except Exception:
        print((work / "jot.log").read_text())
        raise
    finally:
        for process in (fixture, other_app, app):
            if process is not None and process.poll() is None:
                process.terminate()
                process.wait(timeout=5)


def build_test_app(work, identifier, samples, probe="probe.js", query=None, observer=None, data=None, native_probe=None):
    dist = work / "dist"
    subprocess.run(["pnpm", "build", "--outDir", str(dist)], cwd=PROJECT, check=True)
    shutil.copyfile(HERE / probe, dist / "probe.js")
    index = dist / "index.html"
    query = query if query is not None else f"samples={samples}"
    index.write_text(index.read_text().replace(
        "</body>", f'<script type="module" src="/probe.js?{query}"></script></body>',
    ))

    native = work / "src-tauri"
    shutil.copytree(PROJECT / "src-tauri", native, ignore=shutil.ignore_patterns("target", "gen"))
    catalog = work / "src/desktop"
    catalog.mkdir(parents=True)
    for source in (PROJECT / "src/desktop").glob("shortcut-*.json"):
        shutil.copyfile(source, catalog / source.name)
    if native_probe:
        shutil.copyfile(native_probe, native / "src/qa_probe.m")
        build = native / "build.rs"
        build.write_text(build.read_text().replace(
            'cc::Build::new()', 'cc::Build::new().file("src/qa_probe.m")',
        ))
    if observer:
        shutil.copyfile(observer, native / "src/qa_observer.rs")
        main = native / "src/main.rs"
        program = main.read_text().replace("mod desktop;", "mod qa_observer;\nmod desktop;")
        main.write_text(program.replace("tauri::generate_handler![", "tauri::generate_handler![qa_observer::qa_frame,"))
    # These files differ between probes. copytree preserves their source mtimes,
    # so a shared Cargo target could otherwise reuse the previous probe's build script.
    (native / "build.rs").touch()
    (native / "src/main.rs").touch()
    config_path = native / "tauri.conf.json"
    config = json.loads(config_path.read_text())
    config["identifier"] = identifier
    if data is not None:
        config["app"]["appDirectoriesOverride"] = str(data.resolve())
    config["app"]["security"]["capabilities"] = [
        "default", "document",
        {
            "identifier": "qa-window-size",
            "windows": ["main"],
            "permissions": ["core:window:allow-set-size"],
        },
        {
            "identifier": "qa-close-settings",
            "windows": ["settings"],
            "permissions": ["core:window:allow-close"],
        },
    ]
    config_path.write_text(json.dumps(config, indent=2))
    subprocess.run([
        "cargo", "build", "--locked", "--target-dir", str(TARGET),
        "--features", "tauri/custom-protocol",
    ], cwd=native, check=True)
    return TARGET / "debug/Jotiva"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--quick-key", choices=("space", "u"), default="space", help="Key sent with Control+Shift by the test fixture")
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--smoke", action="store_true", help="Check native interactions with a single capture, without the idle-reclaim timings")
    mode.add_argument("--build-only", action="store_true", help="Build the isolated test app without launching any windows")
    options = parser.parse_args()
    identifier = f"io.github.lin037.jot.qa.t{time.time_ns():x}"
    TARGET.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=TARGET.parent, prefix="jot-macos-qa-") as temporary:
        work = Path(temporary)
        samples = 1 if options.smoke else WARM_SAMPLES + 3
        application = build_test_app(work, identifier, samples)
        if options.build_only:
            print(f"Built isolated test app: {application}")
            return

        subprocess.run([
            "xcrun", "clang", "-fobjc-arc", "-Wno-deprecated-declarations",
            str(HERE / "fixture.m"), "-framework", "AppKit", "-framework",
            "ApplicationServices", "-framework", "Carbon", "-o", str(work / "fixture"),
        ], check=True)
        subprocess.run(["xcrun", "clang", "-fobjc-arc", str(HERE / "file-drag.m"), "-framework", "AppKit",
                        "-framework", "ApplicationServices", "-o", str(work / "file-drag")], check=True)
        observer_check = work / "host-focus"
        subprocess.run([
            "xcrun", "clang", "-fobjc-arc", "-Wno-deprecated-declarations",
            str(HERE / "host-focus.m"), str(PROJECT / "src-tauri/src/platform/macos/objc/identity.m"),
            "-framework", "AppKit", "-framework", "ApplicationServices", "-o", str(observer_check),
        ], check=True)
        subprocess.run([observer_check], check=True)
        data = Path.home() / "Library/Application Support" / identifier
        assert not data.exists()
        try:
            if options.quick_key == "u":
                data.mkdir()
                (data / "settings.json").write_text(json.dumps({"quick": "Control+Shift+KeyU", "search": None, "dock": None}))
            result = exercise(work, application, data, options.quick_key, options.smoke)
            print(json.dumps(result, ensure_ascii=False, indent=2))
        finally:
            if data.exists():
                shutil.rmtree(data)


if __name__ == "__main__":
    main()
