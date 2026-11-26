"""Use the bundled editor and Worker to verify custom shortcuts in WebKit."""
from functools import partial
from http.server import ThreadingHTTPServer
import json
import os
from threading import Thread

from playwright.sync_api import expect, sync_playwright

from scenarios.images import check_image_settings_and_picker
from tooltips import IPC, PROJECT, Static

TEST_IPC = """
const original = window.__TAURI_INTERNALS__.invoke;
const stored = () => JSON.parse(localStorage.getItem('shortcutOverrides') || '{}');
window.changeKeys = changes => {
  localStorage.setItem('shortcutOverrides', JSON.stringify(changes));
  window.testEvent('desktop:shortcuts-changed', changes);
};
note.markdown = 'hello';
window.savedNote = note;
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
    page.locator(".cm-search input[name=search]").press("Escape")
    print("Editor format, document search and display-mode bindings changed, cleared, reloaded and restored.")


def verify_markdown(page, url, windows=False):
    platform = """
    window.__jotPlatform.os = 'windows';
    Object.defineProperty(navigator,'platform',{configurable:true,value:'Win32'});
    """ if windows else ""
    page.add_init_script(IPC + TEST_IPC + platform)
    page.goto(url)
    content = page.locator(".cm-content")
    expect(content).to_be_visible()
    page.wait_for_function("window.menu?.editable")
    primary = "Control" if windows else "Meta"
    content.focus()

    def saved(markdown):
        page.wait_for_function("text => window.savedNote.markdown === text", arg=markdown, timeout=6000)

    page.keyboard.press("Control+Shift+Digit9")
    saved("- [ ] hello")
    page.keyboard.press(primary + "+Alt+Enter")
    saved("- [x] hello")
    page.locator(".jot-live-preview-task-checkbox").click()
    saved("- [ ] hello")
    page.keyboard.press(primary + "+z")
    saved("- [x] hello")
    if windows:
        return

    def replace(value):
        content.focus()
        page.keyboard.press("Meta+a")
        page.keyboard.insert_text(value)
        saved(value)

    rows = "9) A\n2) B\n  5. child\n2) C"
    replace(rows)
    page.locator('[data-jot-toolbar-command="toolbar.open-more"]').click()
    page.locator('[data-jot-toolbar-choice="list.renumber"]').click()
    saved("9) A\n10) B\n  5. child\n11) C")
    page.keyboard.press("Meta+z")
    saved(rows)
    for fence in ["~~~", chr(96)*3]:
        replace(fence)
        page.keyboard.press("Enter")
        page.keyboard.insert_text("x")
        saved(fence+"\nx\n"+fence)
    replace(r"Inline \(x+1\)" + "\n\n" + r"\[" + "\ny+2\n" + r"\]" + "\n\nend")
    expect(page.locator(".katex")).to_have_count(2)
    print("Passed task shortcuts, checkbox, undo, list renumbering, code fences and inline/display math.")


if __name__ == "__main__":
    with ThreadingHTTPServer(("127.0.0.1", 0), partial(Static, directory=PROJECT / "dist")) as server:
        Thread(target=server.serve_forever, daemon=True).start()
        try:
            with sync_playwright() as playwright:
                browser = playwright.webkit.launch(executable_path=os.getenv("WEBKIT_EXECUTABLE"))
                verify(browser.new_page(), f"http://127.0.0.1:{server.server_port}/")
                for windows in (False, True):
                    test_page = browser.new_page()
                    verify_markdown(test_page, f"http://127.0.0.1:{server.server_port}/", windows)
                    test_page.close()
                check_image_settings_and_picker(browser, f"http://127.0.0.1:{server.server_port}/", IPC)
                browser.close()
        finally:
            server.shutdown()
