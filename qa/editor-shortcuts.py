"""Use the bundled editor and Worker to verify custom shortcuts in WebKit."""
from functools import partial
from http.server import ThreadingHTTPServer
import json
import os
from threading import Thread

from playwright.sync_api import expect, sync_playwright

from tooltips import IPC, PROJECT, Static

TEST_IPC = """
const original = window.__TAURI_INTERNALS__.invoke;
const stored = () => JSON.parse(localStorage.getItem('shortcutOverrides') || '{}');
window.changeKeys = changes => {
  localStorage.setItem('shortcutOverrides', JSON.stringify(changes));
  window.testEvent('desktop:shortcuts-changed', changes);
};
note.markdown = 'hello';
window.__TAURI_INTERNALS__.invoke = async (cmd, args) => {
  if (cmd === 'save_note') { note.markdown = args.markdown; return ++note.version; }
  if (cmd === 'document_menu') { window.menu = args; return; }
  const result = await original(cmd, args);
  if (cmd === 'read_settings') return {...result, preferences: {...result.preferences, local: stored()}};
  return result;
};
"""


def verify(page, url):
    page.add_init_script(IPC + TEST_IPC)
    page.goto(url)
    content = page.locator(".cm-content")
    expect(content).to_be_visible()
    page.wait_for_function("window.menu?.editable")
    content.focus()
    page.keyboard.press("Control+Shift+P")
    palette = page.get_by_role("listbox", name="编辑命令")
    expect(palette).to_be_visible()
    registered = set(palette.locator("[data-jot-command]").evaluate_all(
        "items => items.map(item => item.dataset.jotCommand)"
    ))
    catalog = json.loads((PROJECT / "src/desktop/shortcut-editor.json").read_text())
    # The desktop owns the two-mode toggle. AppKit owns undo/redo; Tab navigation is contextual.
    fixed = {"history.undo", "history.redo", "table.next-cell", "table.previous-cell"}
    assert registered == {row["editor"] for row in catalog} - {"mode.cycle"} | fixed
    page.keyboard.press("Escape")
    content.focus()
    page.keyboard.press("Meta+/")
    page.wait_for_function("menu.mode === 'source'")
    page.evaluate("changeKeys({'mode.cycle':'Control+F8','format.bold':'Control+F9','search.open':'Control+F10'})")
    page.keyboard.press("Meta+/")
    assert page.evaluate("menu.mode") == "source"
    page.keyboard.press("Control+F8")
    page.wait_for_function("menu.mode === 'live-preview'")
    page.keyboard.press("Control+F8")
    page.wait_for_function("menu.mode === 'source'")

    content.focus()
    page.keyboard.press("Meta+a")
    page.keyboard.press("Meta+b")
    assert "**" not in content.inner_text()
    page.keyboard.press("Control+F9")
    expect(content).to_have_text("**hello**")
    page.keyboard.press("Meta+f")
    expect(page.locator(".cm-search")).to_have_count(0)
    page.keyboard.press("Control+F10")
    expect(page.locator(".cm-search")).to_be_visible()
    page.locator(".cm-search input[name=search]").press("Escape")

    page.reload()
    expect(content).to_be_visible()
    page.wait_for_function("window.menu?.editable")
    content.focus()
    page.keyboard.press("Control+F10")
    expect(page.locator(".cm-search")).to_be_visible()
    page.locator(".cm-search input[name=search]").press("Escape")
    expect(page.locator(".cm-search")).to_have_count(0)
    page.evaluate("changeKeys({'search.open':null})")
    content.focus()
    page.keyboard.press("Control+F10")
    page.keyboard.press("Meta+f")
    expect(page.locator(".cm-search")).to_have_count(0)
    page.evaluate("changeKeys({})")
    page.keyboard.press("Meta+f")
    expect(page.locator(".cm-search")).to_be_visible()
    print("Editor format, document search and display-mode bindings changed, cleared, reloaded and restored.")


if __name__ == "__main__":
    with ThreadingHTTPServer(("127.0.0.1", 0), partial(Static, directory=PROJECT / "dist")) as server:
        Thread(target=server.serve_forever, daemon=True).start()
        try:
            with sync_playwright() as playwright:
                browser = playwright.webkit.launch(executable_path=os.getenv("WEBKIT_EXECUTABLE"))
                verify(browser.new_page(), f"http://127.0.0.1:{server.server_port}/")
                browser.close()
        finally:
            server.shutdown()
