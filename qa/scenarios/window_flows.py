"""Isolated macOS keyboard journeys. Pass one or more scenario names to share a build."""
import argparse
import json
import os
import plistlib
from pathlib import Path
import select
import shutil
import subprocess
import sys
import tempfile
import time
HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'macos'))
from run import build_test_app
CMD, ALT, CTRL = (1 << 20, 1 << 19, 1 << 18)
KEY_CODES = {'1': 18, '2': 19, '3': 20, '0': 29, 'p': 35, 'return': 36, 'n': 45, 'w': 13, 'k': 40, 'escape': 53, 'backquote': 50, 'right': 124, 'down': 125, 'tab': 48, 'o': 31, 'backslash': 42, 'slash': 44}

def wait(read, name, seconds=8):
    until = time.monotonic() + seconds
    while time.monotonic() < until:
        value = read()
        if value:
            return value
        time.sleep(0.016)
    raise AssertionError(name)

def exercise(case, work, binary, fixture_binary, data, space_display):
    state = work / 'state'
    state.mkdir(exist_ok=True)
    for path in state.glob('*.json'):
        path.unlink()
    data.mkdir()
    (data / 'settings.json').write_text(json.dumps({'quick': 'Control+Shift+KeyU', 'search': None, 'dock': None}))
    folder = work / 'files'
    folder.mkdir(exist_ok=True)
    (folder / '01 First.md').write_text('first document')
    (folder / '02 Second.md').write_text('second document')
    (data / 'workspace.json').write_text(json.dumps([str(folder.resolve())]))
    app = subprocess.Popen([binary], env={**os.environ, 'JOT_QA_STATE': str(state)})
    fixture = None

    def ui(label):
        path = state / (label + '.json')
        return json.loads(path.read_bytes()) if path.exists() else {}

    def send(cmd, reply=True):
        fixture.stdin.write(cmd + '\n')
        fixture.stdin.flush()
        if reply:
            assert select.select([fixture.stdout], [], [], 5)[0], cmd
            return fixture.stdout.readline().strip()

    def key(name, flags=CMD):
        code = KEY_CODES[name]
        wait(lambda: send(f'key {code} {flags}') == 'posted', 'key needs Jotiva focus')

    def frames():
        return json.loads(send('frames'))

    def notes():
        return [w['frame'] for w in frames()['windows'] if w['title'] == 'Jotiva 便签']

    def host(name):
        send(name, False)
        wait(lambda: send('status') == f'status {fixture.pid} Fixture {name}', f'host {name}')

    def capture():
        assert send('s').startswith('key posted')
        wait(lambda: ui('quick').get('input') and ui('quick').get('focused'), 'editable capture')

    def inside(frame, screen):
        x, y, w, h = screen
        return frame['X'] >= x - 2 and frame['Y'] >= y - 2 and (frame['X'] + frame['Width'] <= x + w + 2) and (frame['Y'] + frame['Height'] <= y + h + 2)

    def notes_on(display, count=1):
        visible = notes()
        return len(visible) == count and all(inside(note, screens[display]) for note in visible)

    def displays():
        print('Physical screen work areas', screens, flush=True)
        assert send('type-digit') == 'posted'
        wait(lambda: '2' in ui('quick').get('text', ''), 'ordinary system keyboard input')
        assert inside(notes()[0], screens[0]), notes()
        key('return')
        wait(lambda: send('status') == f'status {fixture.pid} Fixture A', 'return without folding')
        assert send('screen A 1') == 'moved'
        wait(lambda: notes_on(1), 'mounted note followed host to second display')
        for n in (0, 1) * 5:
            host('B')
            wait(lambda: not notes(), 'note hid on other host')
            assert send(f'screen A {n}') == 'moved'
            start = time.perf_counter()
            host('A')
            wait(lambda: notes_on(n), 'restored on host display')
            print(f'Hot visible return display {n}: {(time.perf_counter() - start) * 1000:.1f} ms including test polling', flush=True)
        capture()
        key('n')
        wait(lambda: notes_on(1, 2), 'two mounted notes on second display')
        key('w')
        wait(lambda: len(notes()) == 1, 'close only the sibling')
        host('A')
        assert send('fullscreen') == 'toggled'
        wait(lambda: frames()['fullscreen'] and notes(), 'full-screen host Space')
        time.sleep(0.9)
        wait(lambda: notes_on(1), 'fullscreen note on hosting display')
        stack = send('note-stack').split(',')
        assert stack.index('Jotiva 便签') < stack.index('A'), stack
        assert send('fullscreen') == 'toggled'
        wait(lambda: not frames()['fullscreen'] and notes(), 'ordinary desktop restored')

    def fullscreen_input():
        key('n')
        wait(lambda: len(notes()) == 2, 'separate sibling before entering full screen')
        key('w')
        wait(lambda: len(notes()) == 1, 'original note remains')
        host('A')
        assert send('screen A 1') == 'moved'
        host('A')
        wait(lambda: notes_on(1), 'mount on second screen before entering full screen')
        assert send('fullscreen') == 'toggled'
        wait(lambda: frames()['fullscreen'], 'full-screen transition finished')
        host('A')
        wait(lambda: notes_on(1), 'note in fullscreen Space')
        def note_above_host():
            stack = send('note-stack').split(',')
            return 'A' in stack and 'Jotiva 便签' in stack and stack.index('Jotiva 便签') < stack.index('A')
        wait(note_above_host, 'note above full-screen host')
        assert send('click-host A') == 'clicked'
        wait(lambda: send('status') == f'status {fixture.pid} Fixture A', 'full-screen host focused')
        before = ui('quick')['text']
        previous_epoch = ui('quick')['epoch']
        assert send('s').startswith('key posted')
        wait(lambda: ui('quick').get('epoch', -1) > previous_epoch and ui('quick').get('input') and ui('quick').get('view') == 'edit', 'full-screen hotkey delivered to editor')
        assert send('type-digit') == 'posted'
        wait(lambda: ui('quick').get('text', '').count('2') > before.count('2'), 'first digit in full-screen note')
        assert frames()['fullscreen'] and 'A' in send('note-stack').split(','), 'left full-screen host'

    def ordinary_spaces():
        display = start_display
        def switch(desktop):
            x, y = screens[display][:2]
            return subprocess.check_output(['/usr/bin/osascript', '-l', 'JavaScript', str(HERE / 'spaces.js'),
                                            str(display), str(x), str(y), desktop], text=True).strip()

        key('return')
        host('A')
        assert send('click-host A') == 'clicked'
        for pinned in (False, True):
            if pinned:
                capture()
                key('p', CMD | ALT)
                wait(lambda: ui('quick').get('pin') == 'true', 'pin before desktop change')
                key('return')
                host('A')
            previous = switch('other')
            try:
                wait(lambda: send('host-space') == 'away', 'host left active desktop')
                if pinned:
                    wait(lambda: notes_on(display), 'pinned note visible on second desktop')
                else:
                    wait(lambda: not notes(), 'unpinned note hidden on second desktop')
            finally:
                switch(previous)
                wait(lambda: send('host-space') == 'here', 'return to original desktop')
            wait(lambda: notes_on(display), 'note restored on original desktop')
            assert '1' in ui('quick')['text'] and ui('quick')['view'] == 'edit'
        print('Ordinary desktop round trips on display', display, flush=True)

    def documents():
        key('backquote')
        wait(lambda: ui('main').get('focused') and ui('main').get('input'), 'main from quick')
        key('3')
        wait(lambda: ui('main').get('tree') == 'archive', 'archive focus')
        key('2')
        wait(lambda: ui('main').get('tree') == 'workspace', 'workspace focus')
        key('right', 0)
        wait(lambda: ui('main').get('childrenReady'), 'workspace files loaded')
        key('down', 0)
        wait(lambda: ui('main').get('row') == '01 First.md', 'first file focused')
        key('return', 0)
        wait(lambda: ui('main').get('input') and 'first document' in ui('main').get('text', ''), 'first file open')
        key('2')
        wait(lambda: ui('main').get('row') == '01 First.md', 'return to current file')
        key('down', 0)
        wait(lambda: ui('main').get('row') == '02 Second.md', 'second file focused')
        key('return', 0)
        wait(lambda: ui('main').get('input') and 'second document' in ui('main').get('text', ''), 'second file open')
        key('tab', CTRL)
        wait(lambda: ui('main').get('input') and 'first document' in ui('main').get('text', ''), 'previous document')
        key('k')
        wait(lambda: ui('main').get('search'), 'search all documents')
        key('0', 0)
        key('2', 0)
        wait(lambda: any(('02 Second' in item for item in ui('main').get('results', []))), 'indexed search result')
        key('return', 0)
        wait(lambda: ui('main').get('input') and 'second document' in ui('main').get('text', ''), 'open search result')
        key('1')
        wait(lambda: ui('main').get('input'), 'editor focus')
        key('o', CMD | ALT)

        def doc():
            return next((ui(p.stem) for p in state.glob('document-*.json') if ui(p.stem).get('ready')), {})
        wait(lambda: doc().get('input'), 'separate editable document')
        key('backquote')
        wait(lambda: ui('main').get('focused'), 'return to main')
        key('tab', CTRL)
        wait(lambda: 'first document' in ui('main').get('text', '') and ui('main').get('input'), 'different document on the left')
        key('backslash')
        wait(lambda: doc().get('focused'), 'side by side')
        def tiled():
            windows = frames()['windows']
            main = next((w['frame'] for w in windows if w['title'] == 'Jotiva'), None)
            right = next((w['frame'] for w in windows if w['title'] == '02 Second.md'), None)
            return (main and right and abs(right['X'] - main['X'] - main['Width']) <= 2
                    and abs(right['Y'] - main['Y']) <= 2)
        wait(tiled, 'adjacent native frames')
        assert 'second document' in doc()['text'] and 'first document' in ui('main')['text']
        key('slash')
        wait(lambda: doc().get('mode') == 'source', 'source')
        key('slash')
        wait(lambda: doc().get('mode') == 'live-preview', 'live preview')

    def capture_and_pin():
        key('p', CMD | ALT)
        wait(lambda: ui('quick').get('pin') == 'true', 'pin')
        host('B')
        time.sleep(0.25)
        assert notes(), 'pinned note lost'
        host('A')
        capture()
        key('p', CMD | ALT)
        wait(lambda: ui('quick').get('pin') == 'false', 'unpin')
        key('return')
        wait(lambda: send('status') == f'status {fixture.pid} Fixture A', 'return to host')
        assert ui('quick')['view'] == 'edit'
        host('B')
        wait(lambda: not notes(), 'unpinned note hidden')
        host('A')
        capture()
        key('k')
        wait(lambda: ui('quick').get('search'), 'search')
        key('escape', 0)
        wait(lambda: ui('quick').get('view') == 'edit', 'search returned')
    try:
        wait(lambda: ui('main').get('focused'), 'main focused', 25)
        fixture = subprocess.Popen([fixture_binary, str(app.pid), 'u'], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
        assert fixture.stdout.readline().strip().endswith(' 1')
        screens = frames()['screens']
        if case in ('displays', 'fullscreen'):
            assert len(screens) >= 2, screens
        start_display = space_display if case == 'spaces' and space_display is not None else 0
        assert 0 <= start_display < len(screens), screens
        assert send(f'screen A {start_display}') == 'moved'
        host('A')
        capture()
        key('1', 0)
        wait(lambda: '1' in ui('quick').get('text', ''), 'first character')
        {'displays': displays, 'fullscreen': fullscreen_input, 'spaces': ordinary_spaces, 'documents': documents, 'capture': capture_and_pin}[case]()
        print(case, 'passed', flush=True)
    except Exception:
        print('Last UI', [(p.name, p.read_text()[:650]) for p in state.glob('*.json')], flush=True)
        if fixture:
            print('WindowServer', frames(), send('status'), flush=True)
        raise
    finally:
        if fixture and fixture.poll() is None and frames()['fullscreen']:
            send('fullscreen')
            time.sleep(1)
        for proc in (fixture, app):
            if proc and proc.poll() is None:
                proc.terminate()
                proc.wait(timeout=8)
if __name__ == '__main__':
    cli = argparse.ArgumentParser()
    cli.add_argument('cases', nargs='+', choices=['displays', 'fullscreen', 'spaces', 'capture', 'documents'])
    cli.add_argument('--space-display', type=int, help='Display with two ordinary Spaces, zero based')
    args = cli.parse_args()
    cases = args.cases
    with tempfile.TemporaryDirectory(prefix='jot-window-flows-') as tmp:
        work = Path(tmp)
        ident = f'io.github.lin037.jot.qa.flow{time.time_ns():x}'
        app = build_test_app(work, ident, 1, probe='../scenarios/observer.js', observer=HERE / 'observer.rs')
        subprocess.run(['xcrun', 'clang', '-fobjc-arc', '-Wno-deprecated-declarations', str(HERE.parent / 'macos/fixture.m'), '-framework', 'AppKit', '-framework', 'ApplicationServices', '-framework', 'Carbon', '-o', str(work / 'fixture')], check=True)
        # WindowServer keyboard ownership in foreign full-screen Spaces depends on application activation.
        # Give each test process its own application identity, as in an installed desktop build.
        def app_bundle(executable, name, bundle_id):
            bundle = work / f"{name}.app"
            macos = bundle / "Contents" / "MacOS"
            macos.mkdir(parents=True)
            binary = macos / name
            shutil.copy2(executable, binary)
            info = {"CFBundleIdentifier": bundle_id, "CFBundleName": name, "CFBundleExecutable": name,
                    "CFBundlePackageType": "APPL", "CFBundleVersion": "1", "NSHighResolutionCapable": True}
            (bundle / "Contents" / "Info.plist").write_bytes(plistlib.dumps(info))
            subprocess.run(["codesign", "--force", "--sign", "-", str(bundle)], check=True)
            return binary
        app = app_bundle(app, "Jotiva", ident)
        fixture_binary = app_bundle(work / "fixture", "Host", ident + ".host")
        data = Path.home() / 'Library/Application Support' / ident
        failed = []
        for case in cases:
            assert not data.exists()
            try:
                exercise(case, work, app, fixture_binary, data, args.space_display)
            except AssertionError as error:
                print(case, 'failed:', error, flush=True)
                failed.append(case)
            finally:
                if data.exists():
                    shutil.rmtree(data)
        if failed:
            raise SystemExit(f'Failed scenarios: {failed}')
