"""Import the example CSS through Settings and exercise the four bundled views."""
from functools import partial
from http.server import ThreadingHTTPServer
import os
from threading import Thread

from playwright.sync_api import expect, sync_playwright

from tooltips import IPC, PROJECT, Static

THEME = PROJECT / "showcase/themes/ink-and-steel.css"
STYLESHEET_IPC = r"""
const originalInvoke = window.__TAURI_INTERNALS__.invoke;
const storageKey = 'qa:appearance';
const current = () => JSON.parse(localStorage.getItem(storageKey)) ?? appearance;
const update = value => window.testEvent('desktop:appearance-changed', value);
window.__TAURI_INTERNALS__.invoke = (command, args) => {
  if (command === 'read_appearance') return Promise.resolve(current());
  if (command === 'save_appearance' || command === 'set_stylesheet') {
    const value = {...current(), ...(args.preferences ?? {stylesheet:args.stylesheet})};
    localStorage.setItem(storageKey, JSON.stringify(value));
    update(value);
    return Promise.resolve();
  }
  return originalInvoke(command, args);
};
window.addEventListener('storage', event => {
  if (event.key === storageKey) update(current());
});
"""


def check_dialog_backdrop(page, expected):
    page.get_by_role("button", name="添加来源窗口").click()
    dialog = page.locator(".window-picker[open]")
    expect(dialog).to_be_visible()
    assert dialog.evaluate("el => getComputedStyle(el, '::backdrop').backgroundColor") == expected
    page.keyboard.press("Escape")
    expect(dialog).to_have_count(0)


def verify(browser, url):
    context = browser.new_context(color_scheme="light")
    context.add_init_script(IPC + STYLESHEET_IPC)
    settings = context.new_page()
    settings.goto(url + "?view=settings")
    workspace = context.new_page()
    quick = context.new_page()
    for page, label in ((workspace, "main"), (quick, "quick")):
        page.goto(url + "?view=" + label)
        expect(page.locator(".cm-content")).to_be_visible()
    root = workspace.locator("html")
    baseline = root.evaluate("el => getComputedStyle(el).backgroundColor")
    check_dialog_backdrop(workspace, "rgba(0, 0, 0, 0.25)")
    settings.locator("#theme-mode").select_option("dark")
    expect(root).to_have_attribute("data-jot-scheme", "dark")
    check_dialog_backdrop(workspace, "rgba(0, 0, 0, 0.5)")
    settings.locator("#theme-mode").select_option("light")
    expect(root).to_have_attribute("data-jot-scheme", "light")

    settings.locator('input[type="file"]').set_input_files(THEME)
    expect(settings.get_by_text(THEME.name, exact=True)).to_be_visible()
    document = context.new_page()
    document.goto(url + "?view=document-1")
    expect(document.locator(".cm-content")).to_be_visible()
    for page in (settings, workspace, quick, document):
        expect(page.locator("html")).to_have_css("background-color", "rgb(247, 244, 234)")
        expect(page.locator("style[data-jot-custom-theme]")).to_have_count(1)

    expect(workspace.locator(".jot-titlebar")).to_have_css("background-color", "rgb(37, 42, 48)")
    expect(workspace.locator(".jot-sidebar")).to_have_css("background-color", "rgb(229, 221, 202)")
    expect(workspace.locator(".jot-titlebar input")).to_have_css("color", "rgb(33, 37, 42)")
    check_dialog_backdrop(workspace, "rgba(0, 0, 0, 0.3)")
    for page, size in ((workspace, 16), (quick, 15), (document, 16)):
        expect(page.locator(".cm-content")).to_have_css("font-size", f"{size}px")
    expect(workspace.locator(".cm-content")).to_have_css("font-family", 'Georgia, "Songti SC", serif')

    settings.locator("#theme-mode").select_option("dark")
    for page in (settings, workspace, quick, document):
        expect(page.locator("html")).to_have_css("background-color", "rgb(35, 38, 43)")
    workspace.reload()
    expect(root).to_have_css("background-color", "rgb(35, 38, 43)")
    check_dialog_backdrop(workspace, "rgba(0, 0, 0, 0.45)")

    settings.locator("#theme-mode").select_option("light")
    settings.get_by_role("button", name="移除", exact=True).click()
    for page in (settings, workspace, quick, document):
        expect(page.locator("style[data-jot-custom-theme]")).to_have_count(0)
    expect(root).to_have_css("background-color", baseline)
    check_dialog_backdrop(workspace, "rgba(0, 0, 0, 0.25)")
    context.close()
    print("Theme import, four views, editor fonts, dark mode, reload and removal passed.")


if __name__ == "__main__":
    assert (PROJECT / "dist/index.html").is_file(), "Run pnpm build first"
    with ThreadingHTTPServer(("127.0.0.1", 0), partial(Static, directory=PROJECT / "dist")) as server:
        Thread(target=server.serve_forever, daemon=True).start()
        try:
            with sync_playwright() as playwright:
                browser = playwright.webkit.launch(executable_path=os.getenv("WEBKIT_EXECUTABLE"))
                verify(browser, f"http://127.0.0.1:{server.server_port}/")
                browser.close()
        finally:
            server.shutdown()
