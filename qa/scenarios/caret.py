"""Compare the actually painted WebKit caret with the right edge of a new Markdown heading."""
from io import BytesIO
from time import monotonic

from PIL import Image


def check_heading_caret(page):
    page.add_style_tag(content=".jot-editor .cm-content {caret-color:rgb(255,0,255)!important}")
    for trial in range(3):
        page.locator(".cm-content").focus()
        started = monotonic()
        page.keyboard.type("#")
        page.wait_for_function("document.querySelector('.jot-live-preview-source-heading')?.textContent === '#'")
        frame = page.evaluate("""() => {
            const range = document.createRange();
            range.selectNodeContents(document.querySelector('.jot-live-preview-source-heading'));
            const {right, y, height} = range.getBoundingClientRect();
            return {right, y, height};
        }""")
        left = int(frame["right"]) - 22
        clip = {"x": left, "y": int(frame["y"]), "width": 34, "height": int(frame["height"])}
        image = Image.open(BytesIO(page.screenshot(caret="initial", clip=clip))).convert("RGB")
        xs = [left + x for y in range(image.height) for x in range(image.width)
              if (p := image.getpixel((x, y)))[0] > 160 and p[2] > 160 and p[1] < 110]
        elapsed = (monotonic() - started) * 1000
        assert elapsed < 400 and xs and abs(max(xs) - frame["right"]) <= 2, (trial, elapsed, xs[:5], frame)
        print(f"Heading {trial+1}: caret x={max(xs)}, glyph edge={frame['right']:.1f}, first capture {elapsed:.0f} ms")
        page.keyboard.press("Backspace")
        page.wait_for_function("document.querySelector('.cm-content').textContent === ''")
