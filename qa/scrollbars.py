"""Check the painted scrollbar in WebKit, including containers added after startup."""
from functools import partial
from http.server import ThreadingHTTPServer
from io import BytesIO
import os
from threading import Thread

from PIL import Image
from playwright.sync_api import sync_playwright

from tooltips import IPC, PROJECT, Static


def verify(page, url):
    page.add_init_script(IPC + """
      note.markdown = Array.from({length:90}, (_,i) => '# 章节 ' + i + '\\n\\n会议记录').join('\\n\\n');
    """)
    page.goto(url)
    rail = page.locator('body > .jot-direct-scrollbar:visible')
    rail.wait_for()
    for selector in ['.document-tabs', '.jot-editor-toolbar']:
        assert page.locator(selector).first.evaluate("e => getComputedStyle(e).scrollbarWidth === 'auto'"), selector
    page.evaluate("""() => {
      const panel = document.createElement('section');
      panel.id = 'overflow-probe';
      panel.style.cssText = 'position:fixed;left:16px;top:66px;width:160px;height:108px;overflow:scroll;z-index:100';
      const content = document.createElement('div');
      content.style.cssText = 'width:320px;height:240px';
      panel.append(content);
      document.body.append(panel);
    }""")
    panel = page.locator('#overflow-probe')
    page.mouse.move(1, 1)
    for scheme, color in [('light', (210, 30, 100)), ('dark', (30, 200, 135))]:
        page.evaluate("""([scheme, color]) => {
          document.documentElement.dataset.jotScheme = scheme;
          document.documentElement.style.setProperty('--jot-scrollbar-thumb', 'rgb(' + color + ')');
        }""", [scheme, color])
        image = Image.open(BytesIO(page.screenshot())).convert('RGB')
        bounds = panel.bounding_box()
        x, y = int(bounds['x']), int(bounds['y'])
        w, h = int(bounds['width']), int(bounds['height'])
        thumb = rail.locator('.jot-direct-scrollbar__thumb').bounding_box()
        mid = int(thumb['y'] + thumb['height'] / 2)
        vertical = sum(image.getpixel((i, y + 14)) == color for i in range(x+w-15, x+w))
        horizontal = sum(image.getpixel((x + 14, i)) == color for i in range(y+h-15, y+h))
        editor = sum(image.getpixel((i, mid)) == color
                     for i in range(int(thumb['x'])-1, int(thumb['x']+thumb['width'])+1))
        assert (vertical, horizontal, editor) == (4, 4, 4), (scheme, vertical, horizontal, editor)

    assert page.locator('.jot-direct-scrollbar-host:visible').first.evaluate(
        "e => getComputedStyle(e).scrollbarWidth === 'none'")  # The editor must not get a second rail.
    assert panel.evaluate("e => { e.scrollTo(32, 40); return e.scrollLeft === 32 && e.scrollTop === 40 }")
    print("WebKit painted matching 4px thumbs on both native axes and the editor in light and dark themes.")


if __name__ == '__main__':
    with ThreadingHTTPServer(('127.0.0.1', 0), partial(Static, directory=PROJECT / 'dist')) as server:
        Thread(target=server.serve_forever, daemon=True).start()
        try:
            with sync_playwright() as playwright:
                browser = playwright.webkit.launch(executable_path=os.getenv('WEBKIT_EXECUTABLE'))
                verify(browser.new_page(viewport={'width': 960, 'height': 620}), f'http://127.0.0.1:{server.server_port}/')
                browser.close()
        finally:
            server.shutdown()
