"""Check all window shortcut bindings and their input guards in WebKit."""
import json
import os
from pathlib import Path
import subprocess
import tempfile

from playwright.sync_api import sync_playwright

PROJECT = Path(__file__).resolve().parents[1]
BINDINGS = [
    ("Meta+KeyN", "newDocument"), ("Meta+KeyS", "save"), ("Meta+KeyK", "search"),
    ("Meta+Shift+KeyO", "overview"), ("Meta+Shift+KeyL", "sidebar"), ("Meta+Comma", "settings"),
    ("Meta+KeyQ", "quit"), ("Meta+KeyW", "closeActive"), ("Meta+Backquote", "main"),
    ("Alt+Meta+KeyP", "pin"), ("Meta+Enter", "returnHost"), ("Meta+Backslash", "tile"),
    ("Meta+Digit1", "editor"), ("Meta+Digit2", "workspace"), ("Meta+Digit3", "archive"),
    ("Control+Tab", "previousDocument"),
]
BUILD = """
import { build } from 'vite';
await build({ configFile: false, logLevel: 'error', build: {
  outDir: process.argv[2], lib: { entry: process.argv[1], name: 'ShortcutTest', formats: ['iife'], fileName: () => 'keys.js' },
}});
"""
DISPATCH = """async ({ chord, options = {}, prevent = false }) => {
  const keys = chord.split('+');
  const edit = document.querySelector('textarea');
  if (prevent) edit.addEventListener('keydown', event => event.preventDefault(), {once: true});
  window.hits.length = 0;
  const event = new KeyboardEvent('keydown', {
    bubbles: true, cancelable: true, code: keys.at(-1), metaKey: keys.includes('Meta'),
    ctrlKey: keys.includes('Control'), altKey: keys.includes('Alt'), shiftKey: keys.includes('Shift'), ...options,
  });
  edit.dispatchEvent(event);
  await Promise.resolve();
  return { hits: window.hits, consumed: event.defaultPrevented };
}"""


def verify(page):
    for chord, action in BINDINGS:
        assert page.evaluate(DISPATCH, {"chord": chord}) == {"hits": [action], "consumed": True}, chord
    assert page.evaluate(DISPATCH, {"chord": "Escape"}) == {"hits": ["escape"], "consumed": True}

    for options in ({"repeat": True}, {"isComposing": True}):
        assert page.evaluate(DISPATCH, {"chord": "Meta+KeyS", "options": options}) == {
            "hits": [], "consumed": False,
        }, options
    assert page.evaluate(DISPATCH, {"chord": "Meta+KeyS", "prevent": True}) == {
        "hits": [], "consumed": True,
    }
    assert page.evaluate(DISPATCH, {"chord": "Meta+Shift+KeyS"}) == {"hits": [], "consumed": False}
    page.evaluate("document.querySelector('dialog').showModal()")
    assert page.evaluate(DISPATCH, {"chord": "Meta+KeyS"}) == {"hits": [], "consumed": False}
    page.evaluate("document.querySelector('dialog').close(); delete window.handlers.returnHost")
    assert page.evaluate(DISPATCH, {"chord": "Meta+Enter"}) == {"hits": [], "consumed": False}
    page.evaluate("applyKeys({save:'Super+Shift+KeyS', search:null})")
    assert page.evaluate(DISPATCH, {"chord":"Meta+KeyS"}) == {"hits": [], "consumed": False}
    assert page.evaluate(DISPATCH, {"chord":"Meta+KeyK"}) == {"hits": [], "consumed": False}
    assert page.evaluate(DISPATCH, {"chord":"Meta+Shift+KeyS"}) == {"hits": ["save"], "consumed": True}
    page.evaluate("applyKeys({})")
    assert page.evaluate(DISPATCH, {"chord":"Meta+KeyS"}) == {"hits": ["save"], "consumed": True}
    print("16 window shortcuts and Escape passed, with repeat, IME, modal and unbound-action checks.")


with tempfile.TemporaryDirectory(prefix="jot-shortcuts-") as directory:
    work = Path(directory)
    entry = work / "entry.ts"
    keys = json.dumps([action for _, action in BINDINGS])
    entry.write_text(f"""
import {{ bindWindowShortcuts, applyLocalShortcuts }} from {json.dumps(str(PROJECT / "src/desktop/shortcuts.ts"))};
window.applyKeys = applyLocalShortcuts;
window.hits = [];
window.handlers = Object.fromEntries({keys}.map(action => [action, () => window.hits.push(action)]));
bindWindowShortcuts(window.handlers, () => window.hits.push('escape'));
""")
    subprocess.run(["node", "--input-type=module", "-e", BUILD, str(entry), str(work / "dist")],
                   check=True, cwd=PROJECT)
    with sync_playwright() as playwright:
        browser = playwright.webkit.launch(executable_path=os.getenv("WEBKIT_EXECUTABLE"))
        page = browser.new_page()
        page.set_content("<textarea></textarea><dialog></dialog>")
        page.add_script_tag(path=work / "dist/keys.js")
        verify(page)
        browser.close()
