"""Verify preset colors in rendered Mermaid diagrams and theme imports across desktop views."""
from functools import partial
from http.server import ThreadingHTTPServer
import io
import json
import os
import re
from threading import Thread

from PIL import Image
from playwright.sync_api import expect, sync_playwright

from tooltips import IPC, PROJECT, Static

THEME = PROJECT / "docs/assets/ink-and-steel.css"
BLUEPRINT = PROJECT / "docs/assets/blueprint.css"
STYLESHEET_IPC = r"""
const originalInvoke = window.__TAURI_INTERNALS__.invoke;
const storageKey = 'qa:appearance';
const themesKey = 'qa:theme-files';
const current = () => JSON.parse(localStorage.getItem(storageKey)) ?? appearance;
const themeFiles = () => JSON.parse(localStorage.getItem(themesKey)) ?? {};
const update = value => window.testEvent('desktop:appearance-changed', value);
const publish = value => {
  const files = themeFiles();
  const view = {...value, themeCss:files[value.themeFile] ?? null};
  localStorage.setItem(storageKey, JSON.stringify(view));
  update(view);
};
const filename = title => /\.css$/i.test(title) ? title : title + '.css';
window.__TAURI_INTERNALS__.invoke = (command, args) => {
  if (command === 'read_appearance') return Promise.resolve(current());
  if (command === 'list_theme_files') {
    window.themeListRequests = (window.themeListRequests ?? 0) + 1;
    return Promise.resolve(Object.keys(themeFiles()).sort());
  }
  if (command === 'save_appearance' || command === 'set_stylesheet') {
    publish({...current(), ...(args.preferences ?? {stylesheet:args.stylesheet})});
    return Promise.resolve();
  }
  if (command === 'choose_theme_css') return Promise.resolve(window.themePick ?? null);
  if (command === 'install_theme') {
    const files = themeFiles();
    const name = args.title ? filename(args.title) : args.stylesheet.name;
    const conflict = Object.keys(files).find(file => file.toLowerCase() === name.toLowerCase());
    if (conflict && conflict !== args.replace) return Promise.resolve(conflict);
    files[conflict ?? name] = args.stylesheet.css;
    localStorage.setItem(themesKey, JSON.stringify(files));
    if (current().themeFile === (conflict ?? name)) publish(current());
    return Promise.resolve(null);
  }
  if (command === 'rename_theme') {
    const files = themeFiles();
    const name = filename(args.title);
    files[name] = files[args.file];
    delete files[args.file];
    localStorage.setItem(themesKey, JSON.stringify(files));
    const value = current();
    if (value.themeFile === args.file) publish({...value, themeFile:name});
    return Promise.resolve();
  }
  return originalInvoke(command, args);
};
window.addEventListener('storage', event => {
  if (event.key === storageKey) update(current());
});
"""


def save_global_css(page, css):
    page.get_by_role("textbox", name="全局样式微调").fill(css)
    page.locator(".settings-global-css").get_by_role("button", name="保存", exact=True).click()


def check_dialog_backdrop(page, expected):
    page.get_by_role("button", name="添加来源窗口").click()
    dialog = page.locator(".window-picker[open]")
    expect(dialog).to_be_visible()
    assert dialog.evaluate("el => getComputedStyle(el, '::backdrop').backgroundColor") == expected
    page.keyboard.press("Escape")
    expect(dialog).to_have_count(0)


def check_task_paint(browser, url):
    context = browser.new_context(device_scale_factor=2)
    context.add_init_script(IPC + "\nnote.markdown = '# Tasks\\n\\n- [x] live\\n- [x] projection\\n- [ ] open';")
    page = context.new_page()
    page.goto(url + "?view=main")
    boxes = page.locator(".jot-task-checkbox:visible")
    expect(boxes).to_have_count(3)
    current = boxes.first
    def pixels():
        return Image.open(io.BytesIO(current.screenshot())).convert("RGB").tobytes()
    for mode in ("light", "dark"):
        page.evaluate("async mode => testEvent('desktop:appearance-changed', {...await __TAURI_INTERNALS__.invoke('read_appearance'), mode})", mode)
        current.evaluate("box => box.disabled = false")
        expect(current).to_have_css("appearance", "none")
        before = pixels()
        page.locator(".workspace-search input").focus()
        assert before == pixels()
        # The component's document projection renders this same native input disabled.
        current.evaluate("box => box.disabled = true")
        assert before == pixels()
    expect(boxes.nth(2)).not_to_be_checked()
    context.close()


def check_diagram_presets(browser, url):
    # Select rendered marks and labels, including diagram families with independent colors.
    diagrams = {
        "flowchart-v2": ("flowchart LR\n A --> B", [
            (".node rect", "fill", "surface"), (".flowchart-link", "stroke", "muted")]),
        "gantt": ("gantt\n dateFormat YYYY-MM-DD\n section Plan\n Draw :2026-01-01, 2d", [
            ("rect.task", "fill", "accent-fill"), ("text.taskText", "fill", "on-accent")]),
        "sequence": ("sequenceDiagram\n Alice->>Bob: Hi\n Note over Bob: Ready", [
            ("rect.actor", "fill", "surface"), ("rect.note", "fill", "quote"), ("text.noteText", "fill", "text")]),
        "classDiagram": ("classDiagram\n Note --> Author", [
            (".outer-path path", "fill", "surface"), (".outer-path path:last-child", "stroke", "accent")]),
        "stateDiagram": ("stateDiagram-v2\n Draft --> Saved", [
            (".node rect", "fill", "surface"), (".transition", "stroke", "muted")]),
        "er": ("erDiagram\n NOTE }o--|| AUTHOR : has", [
            (".node rect", "fill", "surface"), (".relationshipLine", "stroke", "muted")]),
        "pie": ('pie\n "Drafts" : 4\n "Saved" : 2\n "Archived" : 1', [
            ("path.pieCircle", "fill", "series-1"), ("path.pieCircle + path", "fill", "series-2"),
            ("path.pieCircle + path + path", "fill", "series-3"), ("text.slice", "fill", "text")]),
        "gitGraph": ("gitGraph\n commit\n branch notes\n checkout notes\n commit", [
            ("circle.commit0", "fill", "series-1"), ("circle.commit1", "fill", "series-2"),
            (".branch-label0 text", "fill", "text")]),
        "mindmap": ("mindmap\n  root((Notes))\n    Draft\n    Saved", [
            (".section-root circle", "fill", "series-1"), (".section-0 .node-bkg", "fill", "series-2")]),
        "timeline": ("timeline\n title Notes\n 2025 : Draft\n 2026 : Saved", [
            (".node-bkg", "fill", "series-1")]),
        "xychart": ('xychart-beta\n title "Notes"\n x-axis [A, B]\n y-axis 0 --> 10\n bar [4, 6]', [
            ("rect.background", "fill", "bg"), (".chart-title text", "fill", "text"),
            (".label text", "fill", "text")]),
    }
    markdown = "Notes\n\n" + "\n\n".join(f"```mermaid\n{source}\n```" for source, _ in diagrams.values())
    context = browser.new_context(color_scheme="light", viewport={"width": 1250, "height": 6800})
    context.add_init_script(IPC + STYLESHEET_IPC + "\nnote.markdown = " + json.dumps(markdown) + ";")
    workspace, settings = context.new_page(), context.new_page()
    workspace.goto(url + "?view=main")
    settings.goto(url + "?view=settings")
    root = workspace.locator("html")
    expect(workspace.locator(".jot-mermaid svg")).to_have_count(len(diagrams), timeout=20000)
    roles = sorted({role for _, checks in diagrams.values() for _, _, role in checks}
                   | {f"series-{i}" for i in range(1, 13)})

    def check_colors():
        palette = workspace.evaluate("""roles => {
            const sample = document.createElement('span');
            sample.hidden = true;
            document.body.append(sample);
            const ctx = document.createElement('canvas').getContext('2d', {willReadFrequently: true});
            const colors = Object.fromEntries(roles.map(role => {
                sample.style.backgroundColor = 'var(--jot-color-' + role + ')';
                ctx.fillStyle = getComputedStyle(sample).backgroundColor;
                ctx.fillRect(0, 0, 1, 1);
                return [role, 'rgb(' + [...ctx.getImageData(0,0,1,1).data].slice(0,3).join(', ') + ')'];
            }));
            sample.remove();
            return colors;
        }""", roles)
        for kind, (_, checks) in diagrams.items():
            svg = workspace.locator(f'.jot-mermaid svg[aria-roledescription="{kind}"]')
            for selector, prop, role in checks:
                expect(svg.locator(selector).first).to_have_css(prop, palette[role])
        assert len({palette[f"series-{i}"] for i in range(1, 4)}) == 3
        def luminance(role):
            rgb = [float(n) / 255 for n in re.findall(r"\d+", palette[role])]
            linear = [v / 12.92 if v <= 0.04045 else ((v + 0.055) / 1.055) ** 2.4 for v in rgb]
            return sum(v * weight for v, weight in zip(linear, (0.2126, 0.7152, 0.0722)))
        pairs = [("text", "surface"), ("text", "quote"), ("muted", "bg"), ("on-accent", "accent-fill")]
        for fg, bg in pairs + [("text", f"series-{i}") for i in range(1, 13)]:
            lo, hi = sorted((luminance(fg), luminance(bg)))
            assert (hi + 0.05) / (lo + 0.05) >= 4.5, (fg, bg, palette[fg], palette[bg])

    presets = {
        "default": ("rgb(255, 255, 255)", "rgb(34, 36, 40)"),
        "material": ("rgb(255, 251, 255)", "rgb(20, 18, 24)"),
        "ink": ("rgb(247, 244, 234)", "rgb(35, 38, 43)"),
        "blueprint": ("rgb(246, 250, 252)", "rgb(16, 37, 51)"),
    }
    available = settings.locator("#theme-preset option").evaluate_all("options => options.map(o => o.value)")
    assert set(available) == set(presets), "Every preset in Settings needs light and dark coverage"
    for preset, backgrounds in presets.items():
        settings.locator("#theme-preset").select_option(preset)
        for mode, background in zip(("light", "dark"), backgrounds):
            settings.locator("#theme-mode").select_option(mode)
            expect(root).to_have_css("background-color", background)
            check_colors()
    # Apply the same CSS on top of Default and then override a single category.
    settings.locator("#theme-preset").select_option("default")
    save_global_css(settings, BLUEPRINT.read_text())
    expect(root).to_have_css("background-color", presets["blueprint"][1])
    check_colors()
    settings.locator("#theme-preset").select_option("blueprint")
    save_global_css(settings, ":root { --jot-color-series-2: #326e52; }")
    expect(workspace.locator(".jot-mermaid .commit1").first).to_have_css("fill", "rgb(50, 110, 82)")
    check_colors()
    context.close()
    print(f"Verified {len(diagrams)} diagram types across {len(presets)} presets in both schemes, plus CSS import.")


def check_diagram_palette(browser, url):
    markdown = """正文

[链接](https://example.com)

```mermaid
flowchart LR
  A[Markdown 源码] -->|解析| B[语法树]
```

```mermaid
gantt
  dateFormat YYYY-MM-DD
  axisFormat %m-%d
  section 项目
  设计 :2026-11-02, 3d
  研发 :2026-11-05, 5d
  交付 :crit, 2026-11-08, 2d
```
"""
    context = browser.new_context(color_scheme="light", viewport={"width": 1250, "height": 1000})
    context.add_init_script(IPC + STYLESHEET_IPC + "\nnote.markdown = " + json.dumps(markdown) + ";")
    workspace, settings = context.new_page(), context.new_page()
    workspace.goto(url + "?view=main")
    settings.goto(url + "?view=settings")
    node = workspace.locator(".jot-mermaid .node rect").first
    task = workspace.locator(".jot-mermaid rect.task").first
    link = workspace.locator(".jot-live-preview-link")
    expect(task).to_have_css("fill", "rgb(69, 105, 147)")
    workspace.evaluate("window.previousEditor = document.querySelector('.cm-content')")
    settings.get_by_role("button", name="松绿").click()
    expect(task).to_have_css("fill", "rgb(62, 112, 95)")
    workspace.emulate_media(color_scheme="dark")
    expect(task).to_have_css("fill", "rgb(153, 195, 175)")
    settings.locator("#theme-preset").select_option("material")
    expect(node).to_have_css("fill", "rgb(33, 31, 38)")
    expect(task).to_have_css("fill", "rgb(208, 188, 255)")
    settings.locator("#theme-preset").select_option("ink")
    expect(task).to_have_css("fill", "rgb(255, 205, 112)")
    settings.locator("#theme-mode").select_option("light")
    expect(task).to_have_css("fill", "rgb(241, 189, 80)")
    original_link = link.evaluate("el => getComputedStyle(el).color")
    original_node = node.evaluate("el => getComputedStyle(el).fill")

    # Apply a copy of the shipped CSS with three light palette entries edited.
    css = THEME.read_text().replace("#f1bd50", "#85d5b6").replace("#e5ddca", "#d5e8de").replace("#21252a", "#12332b")
    save_global_css(settings, css)
    expect(task).to_have_css("fill", "rgb(133, 213, 182)")
    expect(settings.locator(".jot-button.primary").first).to_have_css("background-color", "rgb(133, 213, 182)")
    expect(workspace.locator(".jot-sidebar")).to_have_css("background-color", "rgb(213, 232, 222)")
    expect(workspace.locator(".cm-content")).to_have_css("color", "rgb(18, 51, 43)")
    expect(workspace.locator(".jot-mermaid .nodeLabel").first).to_have_css("color", "rgb(18, 51, 43)")
    expect(workspace.locator(".jot-mermaid .taskText").first).to_have_css("fill", "rgb(33, 27, 16)")
    expect(workspace.locator(".jot-mermaid rect.crit0")).to_have_css("fill", "rgb(133, 213, 182)")
    expect(workspace.locator(".jot-mermaid rect.crit0")).to_have_css("stroke", "rgb(140, 32, 27)")
    expect(node).to_have_css("fill", "rgb(230, 238, 228)")
    assert original_node != node.evaluate("el => getComputedStyle(el).fill")
    assert original_link != link.evaluate("el => getComputedStyle(el).color")
    expect(node).to_have_css("stroke", "rgb(65, 96, 76)")
    expect(workspace.locator(".jot-mermaid .flowchart-link")).to_have_css("stroke", "rgb(79, 74, 64)")
    expect(workspace.locator(".jot-mermaid .grid .tick").first).to_have_css("stroke", "rgb(98, 96, 86)")
    settings.locator("#base-size").fill("18")
    settings.get_by_role("button", name="应用", exact=True).click()
    flow = workspace.locator(".jot-mermaid svg").first
    workspace.wait_for_function("""() => Math.abs(parseFloat(getComputedStyle(
        document.querySelector('.jot-mermaid svg')).fontSize) - 18 * 16 / 13) < .05""")
    assert flow.evaluate("el => getComputedStyle(el).fontFamily === getComputedStyle(document.querySelector('.cm-content')).fontFamily")
    settings.locator("#theme-mode").select_option("dark")
    expect(task).to_have_css("fill", "rgb(255, 205, 112)")
    save_global_css(settings, "")
    settings.locator("#theme-mode").select_option("light")
    expect(task).to_have_css("fill", "rgb(241, 189, 80)")
    expect(node).to_have_css("fill", original_node)
    # A small overlay on the Ink preset can also recolor just the light scheme.
    save_global_css(settings, ':root[data-jot-scheme="light"] { --ink-accent: #85d5b6; }')
    expect(task).to_have_css("fill", "rgb(133, 213, 182)")
    expect(node).to_have_css("stroke", "rgb(65, 96, 76)")
    expect(node).to_have_css("fill", original_node)
    settings.locator("#theme-mode").select_option("dark")
    expect(task).to_have_css("fill", "rgb(255, 205, 112)")
    assert workspace.evaluate("previousEditor === document.querySelector('.cm-content')")
    context.close()
    print("Markdown, flowchart and Gantt follow palette edits and live appearance changes.")


def check_installed_themes(browser, url):
    context = browser.new_context(color_scheme="light")
    context.add_init_script(IPC + STYLESHEET_IPC)
    settings, workspace = context.new_page(), context.new_page()
    settings.goto(url + "?view=settings")
    workspace.goto(url + "?view=main")
    selector = settings.locator("#theme-preset")
    install = settings.get_by_role("button", name="导入主题…")
    canvas = workspace.locator("html")

    def pick(background):
        settings.evaluate("""background => {
            window.themePick = {name:'蓝纸.css', css:':root[data-jot-scheme="light"] { --jot-color-bg: ' + background + '; }'};
        }""", background)
        install.click()

    pick("#abcdef")
    expect(selector.locator('option[value="蓝纸.css"]')).to_have_text("蓝纸")
    expect(selector).to_have_value("default")
    selector.select_option("蓝纸.css")
    expect(canvas).to_have_css("background-color", "rgb(171, 205, 239)")

    pick("#ccddee")
    conflict = settings.get_by_role("dialog", name="已安装同名主题「蓝纸」")
    conflict.get_by_role("button", name="取消").click()
    expect(canvas).to_have_css("background-color", "rgb(171, 205, 239)")
    pick("#ccddee")
    conflict.get_by_role("button", name="更新现有主题").click()
    expect(selector).to_have_value("蓝纸.css")
    expect(canvas).to_have_css("background-color", "rgb(204, 221, 238)")

    pick("#febad0")
    conflict.get_by_role("button", name="换个名字导入…").click()
    alternative = settings.get_by_role("dialog", name="换个名字导入…")
    alternative.get_by_role("textbox", name="名称").fill("粉纸")
    alternative.get_by_role("button", name="导入", exact=True).click()
    expect(selector.locator('option[value="粉纸.css"]')).to_have_text("粉纸")
    expect(canvas).to_have_css("background-color", "rgb(204, 221, 238)")

    settings.get_by_role("button", name="重命名主题「蓝纸」").click()
    rename = settings.get_by_role("dialog", name="重命名主题")
    rename.get_by_role("textbox", name="名称").fill("海蓝")
    rename.get_by_role("button", name="保存").click()
    expect(selector.locator('option[value="蓝纸.css"]')).to_have_count(0)
    expect(selector).to_have_value("海蓝.css")
    document = context.new_page()
    document.goto(url + "?view=document-1")
    expect(document.locator("html")).to_have_css("background-color", "rgb(204, 221, 238)")

    assert workspace.evaluate("!window.themeListRequests") and document.evaluate("!window.themeListRequests")
    assert settings.evaluate("window.themeListRequests > 0")
    assert not settings.evaluate("performance.getEntriesByType('resource').some(({name}) => name.includes('format-css-'))")
    input_css = settings.get_by_role("textbox", name="全局样式微调")
    format_css = settings.get_by_role("button", name="格式化", exact=True)
    css = '/* preserve */@media (min-width: 10px){:root{--jot-color-bg:#eeeeee;--sample:"{;}"}}'
    formatted = '/* preserve */\n@media (min-width: 10px) {\n  :root {\n    --jot-color-bg: #eeeeee;\n    --sample: "{;}";\n  }\n}\n'
    input_css.fill(css)
    format_css.click()
    expect(input_css).to_have_value(formatted)
    expect(canvas).to_have_css("background-color", "rgb(204, 221, 238)")
    settings.locator(".settings-global-css").get_by_role("button", name="保存", exact=True).click()
    for theme in ("blueprint", "粉纸.css", "海蓝.css"):
        selector.select_option(theme)
        expect(canvas).to_have_css("background-color", "rgb(238, 238, 238)")
        expect(document.locator("html")).to_have_css("background-color", "rgb(238, 238, 238)")
    settings.reload()
    expect(selector).to_have_value("海蓝.css")
    expect(input_css).to_have_value(formatted)
    draft = ":root {"
    input_css.fill(draft)
    selector.select_option("blueprint")
    expect(input_css).to_have_value(draft)
    format_css.click()
    expect(settings.get_by_role("alert")).to_have_text("无法格式化 CSS，请检查语法。")
    expect(input_css).to_have_value(draft)
    expect(canvas).to_have_css("background-color", "rgb(238, 238, 238)")
    selector.select_option("海蓝.css")
    save_global_css(settings, "")
    expect(canvas).to_have_css("background-color", "rgb(204, 221, 238)")
    context.close()
    print("Theme installation, conflict actions, live replacement, rename, and global overrides passed.")


def verify(browser, url):
    check_installed_themes(browser, url)
    check_diagram_presets(browser, url)
    check_diagram_palette(browser, url)
    check_task_paint(browser, url)
    context = browser.new_context(color_scheme="light")
    context.add_init_script(IPC + STYLESHEET_IPC)
    settings = context.new_page()
    settings.goto(url + "?view=settings")
    workspace = context.new_page()
    quick = context.new_page()
    for page, label in ((workspace, "main"), (quick, "quick")):
        page.goto(url + "?view=" + label)
        expect(page.locator(".cm-content")).to_be_visible()
    document = context.new_page()
    document.goto(url + "?view=document-1")
    expect(document.locator(".cm-content")).to_be_visible()
    for page in (settings, document):
        expect(page.locator(".secondary-titlebar")).to_be_visible()
        expect(page.locator(".caption-controls")).to_have_count(0)

    root = workspace.locator("html")
    baseline = root.evaluate("el => getComputedStyle(el).backgroundColor")
    check_dialog_backdrop(workspace, "rgba(0, 0, 0, 0.25)")
    settings.locator("#theme-mode").select_option("dark")
    expect(root).to_have_attribute("data-jot-scheme", "dark")
    check_dialog_backdrop(workspace, "rgba(0, 0, 0, 0.5)")
    settings.locator("#theme-mode").select_option("light")
    expect(root).to_have_attribute("data-jot-scheme", "light")

    workspace.evaluate("window.previousEditor = document.querySelector('.cm-content')")
    # The settings control, the editor's painted type and the quick-note metrics
    # must change together, including after a preset is reapplied.
    for preset, body_scale, quick_scale in (("default", 1.13, 1), ("material", 1.13, 1), ("blueprint", 1.13, 1), ("ink", 16/13, 15/13)):
        settings.locator("#theme-preset").select_option(preset)
        for base in (18, 13):
            if base == 13:
                settings.get_by_role("button", name="系统默认", exact=True).click()
            else:
                settings.locator("#base-size").fill(str(base))
                settings.get_by_role("button", name="应用", exact=True).click()
            for page, ratio in ((workspace, body_scale), (document, body_scale), (quick, quick_scale)):
                page.wait_for_function("""expected => Math.abs(parseFloat(getComputedStyle(
                    document.querySelector('.cm-content')).fontSize) - expected) < .05""", arg=base*ratio)
        chrome = workspace.locator(".workspace-titlebar").evaluate("el => getComputedStyle(el).backgroundColor")
        for page in (settings, document):
            expect(page.locator(".secondary-titlebar")).to_have_css("background-color", chrome)
    settings.locator("#theme-preset").select_option("ink")
    expect(root).to_have_css("background-color", "rgb(247, 244, 234)")
    settings.locator("#theme-preset").select_option("material")
    for page in (settings, workspace, quick):
        expect(page.locator("html")).to_have_css("background-color", "rgb(255, 251, 255)")
    assert workspace.evaluate("previousEditor === document.querySelector('.cm-content')")
    save_global_css(settings, THEME.read_text())
    document.reload()
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
    expect(workspace.locator(".cm-content")).to_have_css(
        "font-family", re.compile(r'^(?:Georgia|"Georgia"), (?:Songti SC|"Songti SC"), serif$')
    )

    settings.locator("#theme-mode").select_option("dark")
    for page in (settings, workspace, quick, document):
        expect(page.locator("html")).to_have_css("background-color", "rgb(35, 38, 43)")
    workspace.reload()
    expect(root).to_have_css("background-color", "rgb(35, 38, 43)")
    check_dialog_backdrop(workspace, "rgba(0, 0, 0, 0.45)")

    settings.locator("#theme-mode").select_option("light")
    save_global_css(settings, "")
    for page in (settings, workspace, quick, document):
        expect(page.locator("style[data-jot-custom-theme]")).to_have_count(0)
    expect(root).to_have_css("background-color", "rgb(255, 251, 255)")
    settings.locator("#theme-preset").select_option("default")
    expect(root).to_have_css("background-color", baseline)
    check_dialog_backdrop(workspace, "rgba(0, 0, 0, 0.25)")
    quick.evaluate("""async () => testEvent('desktop:quick-view', {
        epoch:20,kind:'summary',note:await __TAURI_INTERNALS__.invoke('read_card',{id:1}),docked:false,rail:null})""")
    folded = quick.locator(".quick-folded-bar")
    expect(folded).to_be_visible()
    settings.locator("#theme-preset").select_option("ink")
    expect(folded).to_have_css("background-color", "rgb(37, 42, 48)")
    expect(folded.locator(".quick-summary-title")).to_have_css("color", "rgb(255, 241, 206)")
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
