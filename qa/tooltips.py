"""Run after pnpm build. Use a temporary test note and the actual bundled editor in WebKit."""
from functools import partial
import argparse
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
import os
from pathlib import Path
from threading import Thread

from playwright.sync_api import expect, sync_playwright

PROJECT = Path(__file__).resolve().parents[1]
IPC = (PROJECT / "qa/macos/browser-env.js").read_text() + r"""
const note = { id:1, source:'Jotiva', version:1, markdown:'# 项目记录\n\n正文\n\n## 会议纪要\n\n计划\n\n# 下一章\n\n尾段' };
const card = {...note, title:'项目记录', excerpt:'正文', folder:null, attached:false};
const appearance = { baseSize:null, systemSize:13, mode:'system', accent:null,
  workspace:{sidebar:null, outline:null}, stylesheet:null,
  editing:{font:'system', wrapCode:false, foldHeadings:true, toolbar:true, headingScale:null, ruledPaper:false} };
const label = new URL(location.href).searchParams.get('view') ?? 'main';
let serial = 0;
const callbacks = new Map(), events = new Map();
window.testEvent = (name, payload) => (events.get(name) ?? []).forEach(id => callbacks.get(id)({event:name,payload}));
window.testScale = size => window.testEvent('desktop:appearance-changed', {...appearance,baseSize:size});
window.__TAURI_INTERNALS__ = {metadata:{currentWindow:{label},currentWebview:{label}},
  transformCallback: fn => { callbacks.set(++serial,fn); return serial; },
  invoke: async (cmd, args) => {
    if (cmd==='plugin:event|listen') {
      const listeners = events.get(args.event) ?? [];
      listeners.push(args.handler); events.set(args.event,listeners); return args.handler;
    }
    if (cmd==='read_appearance') return appearance;
    if (cmd==='accessibility_trusted') return !window.permissionMissing;
    if (cmd==='quick_view') return {epoch:1, kind:'edit', id:1, standalone:false, pinned:false, source:'会议'};
    if (cmd==='read_settings') {
      const preferences = {quick:'Control+Alt+KeyJ', dock:null, search:null};
      return {preferences, defaults:preferences, trusted:false, shortcutError:null};
    }
    if (cmd==='record_shortcut' || cmd==='quick_presented') return;
    if (cmd==='new_quick') { window.createdQuick=true; return; }
    if (cmd==='drag_window') { window.windowDrags=(window.windowDrags || 0) + 1; return; }
    if (cmd==='note_action' && args.action.kind==='rename') {
      if (args.action.name==='重名.md') throw Error('该名称已存在');
      window.renamedNote=args;
      window.testEvent('desktop:quick-view',{epoch:10,kind:'summary',docked:false,rail:null,
        note:{...card,title:args.action.name,archivePath:'/tmp/'+args.action.name}});
      return;
    }
    if (cmd==='pin_quick') { window.savedPin=args; return; }
    if (cmd==='observe_document_drag') { window.dragTracking = args.started; return; }
    if (cmd==='source_at_drop') return {outside:true, choice:17};
    if (cmd==='attach_note') { window.attachedFromQuick = args; return "host"; }
    if (cmd==='take_dropped_tabs') return [];
    if (cmd==='initial_note') return 1;
    if (cmd==='open_editor_note' || cmd==='read_note') return note;
    if (cmd==='refresh_editor_note') return note.version > args.version ? note : null;
    if (cmd==='read_card') {
      if (window.failCardRead) throw Error('读取文档失败');
      return label==='quick' ? {...card, archivePath:window.quickFilePath} : card;
    }
    if (cmd==='inbox_directory') return '/tmp/Jotiva/其他';
    if (cmd==='archive_entries') return [{path:'/tmp/Jotiva/其他/1.md', id:null, managed:false}];
    if (cmd==='list_folders' || cmd==='list_sources' || cmd==='source_choices') return [];
    if (cmd==='workspace_roots') return ['/work'];
    if (cmd==='workspace_children') return [{name:'会议.md',path:'/work/会议.md',directory:false}];
    if (cmd==='context_menu') { window.focusOptions=args.labels; return args.labels.findIndex(v=>v.endsWith('文件树')); }
    if (cmd==='locate') { window.located=args.id; return; }
    if (cmd==='list_notes') return {items:[card], more:false};
    if (cmd==='search_notes') {
      const live={...card,id:7,title:'议程',source:'会议窗口',attached:true};
      if (args.query === '待办') return {current:[],attached:[{...live,id:8,title:'待办'}],documents:[],more:false};
      if (args.page) return {current:[],attached:[],documents:[{...card,id:9,title:'下一页'}],more:false};
      return {current:args.host===11?[{...live,id:6,title:'本窗口议程'}]:[], attached:[live],documents:[card],more:true};
    }
    if (cmd==='release_note' || cmd==='clear_source_choices' || cmd==='observe_document_drag' || cmd==='document_menu' || cmd==='plugin:window|set_title' || cmd==='plugin:window|set_always_on_top') return;
    throw Error('Unexpected IPC ' + cmd);
  }
};
"""

class Static(SimpleHTTPRequestHandler):
    def log_message(self, *_):
        pass


def expect_hover_appearance(row):
    expect(row).to_be_focused()
    page = row.page
    page.mouse.move(0, 0)
    assert row.evaluate("row => row.matches(':focus') && !row.matches(':hover')")
    paint = """row => {
        const s = getComputedStyle(row);
        return [s.backgroundColor, s.color, s.borderColor, s.outlineStyle, s.boxShadow];
    }"""
    focused = row.evaluate(paint)
    expect(page.locator(".jot-tooltip:popover-open")).to_have_count(0)
    row.evaluate("row => row.blur()")
    row.hover()
    assert row.evaluate("row => row.matches(':hover') && !row.matches(':focus')")
    hovered = row.evaluate(paint)
    assert focused == hovered, (focused, hovered)
    assert focused[0] != "rgba(0, 0, 0, 0)" and focused[-2:] == ["none", "none"]
    page.mouse.move(0, 0)
    row.focus()


def verify_tree_focus(browser, url):
    page = browser.new_page()
    page.add_init_script(IPC)
    page.goto(url)
    editor = page.locator(".cm-content")
    expect(page.locator('.workspace-footer [role="status"]')).to_have_text("已保存")
    editor.click()
    for theme in ("light", "dark"):
        page.emulate_media(color_scheme=theme)
        for key, root in [("2", '[data-directory="/work"]'), ("3", '[data-archive-folder=""]')]:
            page.keyboard.press("Meta+" + key)
            row = page.locator(root)
            expect_hover_appearance(row)
            if row.get_attribute("aria-expanded") != "true":
                row.press("ArrowRight")
            file = page.locator(".library-tree [data-file]:visible").first
            expect(file).to_be_visible()
            page.keyboard.press("ArrowDown")
            expect_hover_appearance(file)
            page.keyboard.press("Meta+1")
            expect(editor).to_be_focused()
        page.get_by_role("button", name="文档窗口操作").press("Enter")
        expect_hover_appearance(page.locator("#document-windows:popover-open button").first)
        page.keyboard.press("Escape")


def verify(browser, url):
    page = browser.new_page(viewport={"width": 1000, "height": 620})
    page.set_default_timeout(6000)
    page.add_init_script(IPC + r"""
      if (label === 'quick') {
        if (new URL(location.href).searchParams.has('draft')) {
          note.markdown = ''; note.version = card.version = 0;
        } else {
          note.markdown += '\n\n会议记录'.repeat(80);
        }
      }
    """)
    errors = []
    page.on("pageerror", lambda error: errors.append(str(error)))
    page.goto(url)
    tip = page.locator(".jot-tooltip:popover-open")
    page.wait_for_function('document.querySelector(".error:not([hidden])") || (document.querySelector(".cm-content") && document.querySelector(".workspace-footer [role=status]")?.textContent==="已保存")')
    expect(page.locator(".error")).to_be_hidden()

    page.locator(".workspace-search input").focus()
    expect(page.get_by_role("group", name="已挂载便签")).to_contain_text("会议窗口")
    expect(page.get_by_role("group", name="文档", exact=True)).to_contain_text("项目记录")
    page.locator(".workspace-search input").press("Enter")
    page.wait_for_function("window.located === 7")
    expect(page.locator(".workspace-search .note-browser")).to_be_hidden()

    def hover(control, text):
        page.mouse.move(0, 0)
        control.hover()
        expect(tip).to_have_text(text)

    page.locator(".cm-content").focus()
    page.keyboard.press("Alt+F6")
    expect(page.locator('[data-directory="/work"]')).to_be_focused()
    page.keyboard.press("F6")
    assert page.evaluate("Boolean(document.activeElement.closest('.source-panel'))")
    page.keyboard.press("Shift+F6")
    expect(page.locator('[data-directory="/work"]')).to_be_focused()
    side = page.get_by_role("button", name="显示或隐藏侧栏")
    side.hover()
    expect(tip).to_have_count(0)
    expect(tip).to_have_text("侧栏")
    expect(side).to_have_attribute("aria-keyshortcuts", "Shift+Meta+L")
    expect(side).to_have_attribute("aria-description", "快捷键 ⇧⌘L")
    page.keyboard.press("Escape")
    expect(tip).to_have_count(0)
    page.locator('[data-jot-toolbar-command="format.bold"]').focus()
    page.keyboard.press("ArrowRight")
    expect(tip).to_have_text("斜体", timeout=350)
    page.keyboard.press("Escape")
    expect(tip).to_have_count(0)

    toggle = page.get_by_role("button", name="文档窗口操作")
    toggle.click()
    menu = page.locator("#document-windows:popover-open")
    first = menu.get_by_role("button", name="挂载到窗口")
    first.hover()
    page.wait_for_timeout(500)
    expect(tip).to_have_count(0)
    assert toggle.evaluate("b => getComputedStyle(b).backgroundColor === 'rgba(0, 0, 0, 0)'")
    assert not first.evaluate("row => row.matches(':focus-visible')")
    page.keyboard.press("Escape")
    toggle.focus()
    page.keyboard.press("Enter")
    expect(first).to_be_focused()
    page.keyboard.press("ArrowDown")
    expect(menu.get_by_role("button", name="作为独立便签打开")).to_be_focused()
    page.keyboard.press("Escape")
    expect(toggle).to_be_focused()

    root = page.locator('[data-directory="/work"]')
    root.click()
    file = page.locator('[data-file="/work/会议.md"]')
    hover(file, "打开")
    expect(file).to_have_attribute("title", "")
    expect(file).to_have_attribute("aria-description", "/work/会议.md")
    file.dispatch_event("dragstart")
    expect(tip).to_have_count(0)

    def toolbar(command):
        return page.locator(f'[data-jot-toolbar-command="{command}"]')

    for command, label in [
        ("history.undo","撤销"), ("history.redo","重做"), ("toolbar.open-heading","标题"),
        ("format.bold","加粗"), ("format.italic","斜体"), ("format.code","行内代码"), ("format.link","链接"),
        ("format.bullet-list","无序列表"), ("format.ordered-list","有序列表"), ("format.quote","引用"),
        ("toolbar.open-table","表格"), ("toolbar.open-more","更多"), ("search.open","搜索"), ("command.palette","命令"),
    ]:
        hover(toolbar(command), label)

    print("Passed workspace controls, keyboard access and the 14 published toolbar buttons.", flush=True)
    bold = toolbar("format.bold")
    hover(bold, "加粗")
    expect(bold).to_have_attribute("title", "")
    assert "B" in bold.get_attribute("aria-description").upper()
    bold.evaluate("(b) => b.title = '加粗 · 新的快捷键'")
    expect(bold).to_have_attribute("title", "")
    expect(bold).to_have_attribute("aria-description", "加粗 · 新的快捷键")
    toolbar("toolbar.open-heading").click()
    level = page.locator('button[data-jot-heading-level="2"]')
    hover(level, "二级标题")
    assert tip.evaluate("el => !!el.parentElement.closest(':popover-open')")
    page.keyboard.press("Escape")
    expect(level).to_have_count(0)
    toolbar("toolbar.open-table").click()
    hover(page.locator('[data-rows="2"][data-columns="4"]'), "插表")
    page.keyboard.press("Escape")
    toolbar("toolbar.open-more").click()
    hover(page.locator('[data-jot-toolbar-choice="insert.code-block"]'), "代码块")
    page.keyboard.press("Escape")

    print("Passed tooltip restoration and the heading, table and extended format menus.", flush=True)
    outline = page.get_by_role("button", name="文档大纲")
    outline.click()
    branch = page.locator('.jot-outline__row[aria-expanded]').first
    caret = branch.locator(".jot-outline__toggle")
    hover(branch.locator("span").last, "定位")
    hover(caret, "折叠")
    caret.click()
    expect(branch).to_have_attribute("aria-expanded","false")
    hover(caret, "展开")
    outline.click()

    bar = page.get_by_role("toolbar", name="Markdown 工具栏")
    for width, scale, split in [(810, 13, True), (580, 13, False), (580, 18, False)]:
        page.set_viewport_size({"width": width, "height": 620})
        page.evaluate("size => testScale(size)", scale)
        if (outline.get_attribute("aria-expanded") == "true") != split:
            outline.click()
        page.wait_for_function("size => parseFloat(getComputedStyle(document.documentElement).fontSize) === size", arg=scale)
        positions = bar.locator("[data-jot-toolbar-command]").evaluate_all(
            "buttons => buttons.map(button => button.getBoundingClientRect().top)")
        assert len(positions) == 14 and max(positions) - min(positions) < 1, positions
        assert bar.evaluate("t => t.scrollWidth > t.clientWidth") == (scale == 18)
    # The component's roving focus must also expose the focused button on a narrow toolbar.
    toolbar("format.bold").focus()
    page.keyboard.press("End")
    last = toolbar("command.palette")
    expect(last).to_be_focused()
    assert bar.evaluate("t => t.scrollLeft > 0")
    def within_bar(button):
        return button.evaluate("""b => {
          const a = b.closest('[role=toolbar]').getBoundingClientRect(), r = b.getBoundingClientRect();
          return r.left >= a.left - 1 && r.right <= a.right + 1;
        }""")
    assert within_bar(last)
    page.keyboard.press("Home")
    assert within_bar(page.locator("[data-jot-toolbar-command]:focus"))
    toolbar("toolbar.open-more").click()
    choice = page.locator('[data-jot-toolbar-choice="insert.thematic-break"]')
    assert choice.evaluate("""b => {
      const r = b.getBoundingClientRect();
      return b.contains(document.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2));
    }""")
    page.keyboard.press("Escape")
    expect(toolbar("toolbar.open-more")).to_be_focused()
    page.set_viewport_size({"width": 1000, "height": 620})
    page.evaluate("testScale(13)")
    print("Passed single-row toolbar, offscreen keyboard controls and uncropped picker with large type.", flush=True)

    fold = page.locator('.jot-heading-fold-button[aria-expanded="true"]').first
    hover(fold, "折叠")
    fold.click()
    hover(page.locator('.jot-heading-fold-button[aria-expanded="false"]').first, "展开")
    print("Passed outline navigation and document fold controls.", flush=True)
    for mode, label in [("source", "源码"), ("live-preview", "即时预览")]:
        toolbar("command.palette").click()
        page.get_by_role("searchbox", name="搜索编辑命令").fill("mode." + mode)
        command = page.locator(f'[data-jot-command="mode.{mode}"]')
        hover(command, label)
        command.click()
        expect(page.locator(".editor-root")).to_have_attribute("data-jot-editor-mode", mode)
        hover(toolbar("toolbar.open-heading"), "标题")

    print("Passed command palette and both editable modes.", flush=True)
    page.evaluate("testScale(20)")
    hover(side, "侧栏")
    assert tip.evaluate("n => parseFloat(getComputedStyle(n).fontSize)") > 16
    page.locator(".workspace").dispatch_event("scroll")
    expect(tip).to_have_count(0)
    page.get_by_role("button", name="添加来源窗口").click()
    cancel = page.get_by_role("dialog").get_by_role("button", name="取消")
    hover(cancel, "取消")
    assert tip.evaluate("n => !!n.closest('dialog[open]')")
    cancel.click()
    expect(tip).to_have_count(0)

    print("Passed theme scale, scroll and modal close cleanup.", flush=True)
    page.goto(url + "?view=document-1")
    pin = page.get_by_role("button", name="置顶文档窗口")
    hover(pin, "置顶")
    pin.click()
    hover(pin, "取消置顶")
    box = tip.bounding_box()
    assert box["x"] >= 0 and box["x"]+box["width"] <= 1000 and box["y"]+box["height"] <= 620
    pin.evaluate("node => node.remove()")
    expect(tip).to_have_count(0)
    assert page.locator(".jot-tooltip").count()==1
    expect(page.locator(".error")).to_be_hidden()

    page.goto(url + "?view=quick")
    menu = page.get_by_role("button", name="便签操作")
    expect(menu).to_be_enabled()
    footer = page.locator(".quick-edit-pane .jot-statusbar")
    expect(footer.get_by_role("status")).to_have_text("已保存")
    expect(footer.locator(".workspace-count")).to_have_text("341 字符")
    rail = page.locator("body > .jot-direct-scrollbar:visible")
    expect(rail).to_have_count(1)
    path = footer.locator(".workspace-location")
    expect(path).to_be_hidden()
    page.evaluate("""() => {
      window.quickFilePath = '/Users/test/Documents/梧桐街/开放日/会议记录.md';
      testEvent('desktop:notes-changed', 1);
    }""")
    expect(path).to_have_text("/Users/test/Documents/梧桐街/开放日/会议记录.md")
    path.click()
    assert page.evaluate("window.focusOptions") == ["在 Finder 中显示", "复制绝对路径"]
    for width, height, size in ((384, 352, 13), (248, 145, 18)):
        page.set_viewport_size({"width": width, "height": height})
        page.evaluate("testScale", size)
        page.wait_for_function("size => parseFloat(getComputedStyle(document.documentElement).fontSize) === size", arg=size)
        title = page.locator(".quick-title").bounding_box()
        action = menu.bounding_box()
        count = footer.locator(".workspace-count").bounding_box()
        status = footer.get_by_role("status").bounding_box()
        assert abs(title["y"] + title["height"]/2 - action["y"] - action["height"]/2) < 1
        assert title["width"] >= size * 2
        assert footer.bounding_box()["y"] + footer.bounding_box()["height"] == height
        assert path.bounding_box()["x"] + path.bounding_box()["width"] <= count["x"] < status["x"]
        assert status["x"] + status["width"] <= width
    menu.focus()
    menu.press("ArrowUp")
    recall = page.get_by_role("button", name="解除挂载并收回")
    expect(recall).to_be_focused()
    bounds = recall.bounding_box()
    assert 0 <= bounds["y"] and bounds["y"] + bounds["height"] <= 145
    recall.press("Escape")
    page.set_viewport_size({"width": 1000, "height": 620})
    page.evaluate("testScale(13)")
    hover(menu, "更多")
    assert menu.inner_text() == "" and menu.locator("svg").count() == 1
    pin = page.get_by_role("button", name="置顶便签")
    pin.click()
    expect(pin).to_have_attribute("aria-pressed", "true")
    assert page.evaluate("savedPin.id === 1 && savedPin.pinned")
    pin.click()
    expect(pin).to_have_attribute("aria-pressed", "false")
    handle = page.get_by_role("button", name="拖动挂载或点选窗口")
    assert handle.evaluate("""node => {
      const dataTransfer = new DataTransfer();
      node.dispatchEvent(new DragEvent('dragstart', {dataTransfer, bubbles:true}));
      node.dispatchEvent(new DragEvent('dragend', {dataTransfer, bubbles:true}));
      return dataTransfer.getData('application/x-jot-note') === '1';
    }""")
    page.wait_for_function("window.attachedFromQuick?.id === 1 && attachedFromQuick.choice === 17 && window.dragTracking === false")
    menu.click()
    for name in ("在此窗口新建便签", "编辑工具栏", "查找便签", "在主窗口编辑", "解除挂载并收回"):
        page.get_by_role("button", name=name).hover()
        page.wait_for_timeout(460)
        expect(tip).to_have_count(0)
    page.get_by_role("button", name="在此窗口新建便签").click()
    page.wait_for_function("window.createdQuick")
    expect(page.locator("#quick-actions:popover-open")).to_have_count(0)
    menu.click()
    page.keyboard.press("Escape")
    expect(tip).to_have_count(0)
    expect(page.locator(".error")).to_be_hidden()

    page.evaluate("window.testEvent('desktop:quick-view',{epoch:6,kind:'search',tiled:false,host:11})")
    query = page.get_by_role("combobox", name="搜索便签和文档")
    expect(page.get_by_role("group", name="当前窗口")).to_contain_text("本窗口议程")
    expect(page.get_by_role("group", name="其他窗口便签")).to_contain_text("会议窗口")
    expect(page.get_by_role("group", name="文档", exact=True)).to_contain_text("项目记录")
    query.press("PageDown")
    expect(page.get_by_role("option", name="下一页")).to_be_visible()
    query.press("PageUp")
    expect(page.get_by_role("option")).to_have_count(3)
    query.press("ArrowUp")
    query.press("Enter")
    page.wait_for_function("window.located === 7")
    # Type and submit within the same turn, before the normal debounce can replace the old row.
    page.locator('.quick-query input').evaluate("""input => {
      input.value = '待办';
      input.dispatchEvent(new Event('input', {bubbles:true}));
      input.dispatchEvent(new KeyboardEvent('keydown', {key:'Enter', code:'Enter', bubbles:true}));
    }""")
    page.wait_for_function("window.located === 8")
    page.evaluate("window.testEvent('desktop:quick-view',{epoch:7,kind:'search',tiled:true,host:11})")
    expect(query).to_have_value("待办")
    expect(page.locator(".note-grid .note-row")).to_have_count(1)
    query.fill("")
    expect(page.locator(".note-grid .note-row")).to_have_count(3)
    query.press("ArrowDown")
    expect(query).to_have_attribute("aria-activedescendant", "jot-search-option-7")
    query.press("Meta+3")
    page.wait_for_function("window.located === 1")

    page.set_viewport_size({"width": 320, "height": 76})
    page.evaluate("""testEvent('desktop:quick-view', {epoch:8, kind:'summary', docked:false, rail:null,
      note:{id:1,title:'01-筹备周会.md',excerpt:'周五 17:00 前确认连廊照明和地面情况',attached:true,source:'会议'}})""")
    summary = page.locator(".quick-folded")
    title_button = summary.get_by_role("button", name="重命名，01-筹备周会.md")
    expand = summary.get_by_role("button", name="展开便签")
    expect(title_button).to_be_visible()
    expect(rail).to_have_count(0)
    title = title_button.bounding_box()
    excerpt = summary.locator(".quick-summary-excerpt").bounding_box()
    expander = expand.bounding_box()
    assert abs(title["y"] + title["height"]/2 - expander["y"] - expander["height"]/2) < 1
    assert excerpt["x"] == title["x"] and title["y"] + title["height"] <= excerpt["y"]
    assert excerpt["y"] + excerpt["height"] <= 76 and summary.bounding_box()["height"] == 76
    x, y = title["x"] + 12, title["y"] + title["height"]/2
    page.mouse.move(x, y)
    page.mouse.down()
    page.mouse.move(x+20, y+2, steps=5)
    page.mouse.up()
    page.wait_for_function("window.windowDrags === 1")
    field = summary.get_by_role("textbox", name="重命名")
    expect(field).to_have_count(0)
    title_button.click()
    expect(field).to_be_focused()
    assert field.evaluate("el => el.value.slice(el.selectionStart,el.selectionEnd)") == "01-筹备周会"
    field.press("Escape")
    expect(title_button).to_be_focused()
    title_button.press("F2")
    field.fill("重名.md")
    field.press("Enter")
    expect(summary.get_by_role("alert")).to_have_text("该名称已存在")
    expect(field).to_be_focused()
    field.fill("周会议程.md")
    page.evaluate("""testEvent('desktop:quick-view', {epoch:9, kind:'summary', docked:false, rail:null,
      note:{id:1,title:'01-筹备周会.md',excerpt:'另一窗口刚刚保存了正文',attached:true,source:'会议'}})""")
    expect(field).to_have_value("周会议程.md")
    expect(field).to_be_focused()
    field.press("Enter")
    expect(summary.get_by_role("button", name="重命名，周会议程.md")).to_be_visible()
    assert page.evaluate("renamedNote.id") == 1
    expand.press("Enter")
    page.wait_for_function("window.located === 1")
    page.set_viewport_size({"width": 248, "height": 145})
    page.goto(url + "?view=quick&draft")
    expect(footer.get_by_role("status")).to_have_text("新便签")
    expect(footer.locator(".workspace-count")).to_have_text("0 字符")
    expect(path).to_be_hidden()
    page.evaluate("testScale(18)")
    page.evaluate("window.failCardRead = true; testEvent('desktop:notes-changed', 1)")
    recover = footer.get_by_role("button", name="另存编辑并重载")
    expect(recover).to_be_visible()
    expect(footer.get_by_role("button", name="用 Jev 归档", include_hidden=True)).to_be_hidden()
    box = recover.bounding_box()
    assert box["x"] >= 0 and box["x"] + box["width"] <= 248
    expect(footer.locator(".workspace-count")).to_be_hidden()
    page.set_viewport_size({"width": 1000, "height": 620})
    page.goto(url + "?view=settings")
    hover(page.get_by_role("button", name="系统默认"), "默认")
    page.evaluate("window.permissionMissing=true")
    page.get_by_role("button", name="窗口关联", exact=True).click()
    expect(page.get_by_text("当前运行的 Jotiva 尚未获得授权")).to_be_visible()
    page.evaluate("window.permissionMissing=false")
    expect(page.get_by_text("已授权", exact=True)).to_be_visible()
    expect(page.get_by_role("button", name="授权辅助功能…")).to_be_hidden()
    page.get_by_role("button", name="快捷键", exact=True).click()
    record = page.locator("#key-quick")
    hover(record, "录入")
    record.click()
    hover(record, "取消")
    record.click()
    hover(page.get_by_role("button", name="保存快捷键"), "保存")
    expect(page.locator(".error")).to_be_hidden()
    assert not errors, errors


def verify_shortcut_settings(browser, url):
    context = browser.new_context()
    context.add_init_script(IPC + r"""
      const nativeInvoke = window.__TAURI_INTERNALS__.invoke;
      const defaults = {quick:'Control+Shift+Space',search:null,dock:null,local:{}};
      const saved = () => JSON.parse(localStorage.getItem('qa:keys')) ?? defaults;
      window.addEventListener('storage', e => {
        if(e.key==='qa:keys') window.testEvent('desktop:shortcuts-changed', saved().local);
      });
      window.__TAURI_INTERNALS__.invoke = async (cmd, args) => {
        if(cmd==='read_settings') return {preferences:saved(), defaults, trusted:true, shortcutError:null};
        if(cmd==='save_settings') {
          if(window.rejectKey) throw Error('该组合已被其他应用登记');
          localStorage.setItem('qa:keys', JSON.stringify(args.preferences));
          window.testEvent('desktop:shortcuts-changed', saved().local);
          return;
        }
        return nativeInvoke(cmd, args);
      };
    """)
    workspace = context.new_page()
    workspace.goto(url)
    side = workspace.get_by_role("button", name="显示或隐藏侧栏")
    expect(side).to_have_attribute("aria-keyshortcuts", "Shift+Meta+L")
    settings = context.new_page()
    settings.goto(url + "?view=settings")
    settings.get_by_role("button", name="快捷键", exact=True).click()
    find = settings.get_by_role("searchbox", name="搜索操作或快捷键")
    record = settings.locator("#key-local-sidebar")
    save = settings.get_by_role("button", name="保存快捷键", exact=True)
    find.fill("侧栏")
    expect(record).to_be_visible()
    expect(settings.locator("#key-local-openFile")).to_be_hidden()
    record.click()
    settings.keyboard.press("Escape")
    expect(record).to_have_text("⇧⌘L")
    record.click()
    # AppKit recorded a menu equivalent before it could reach the WebView.
    settings.evaluate("testEvent('desktop:shortcut-recorded', 'Super+Shift+KeyY')")
    expect(record).to_have_text("⇧⌘Y")
    settings.evaluate("window.rejectKey=true")
    save.click()
    expect(settings.locator(".shortcut-actions [role=status]")).to_contain_text("该组合已被其他应用登记")
    expect(side).to_have_attribute("aria-keyshortcuts", "Shift+Meta+L")
    settings.evaluate("window.rejectKey=false")
    save.click()
    expect(side).to_have_attribute("aria-keyshortcuts", "Shift+Meta+Y")
    find.fill("全局")
    find.fill("")
    quick = settings.locator("#key-quick")
    quick.click()
    settings.keyboard.press("Control+Shift+u")
    expect(quick).to_have_text("⌃⇧U")
    save.click()
    settings.reload()
    settings.get_by_role("button", name="快捷键", exact=True).click()
    expect(record).to_have_text("⇧⌘Y")
    expect(quick).to_have_text("⌃⇧U")
    settings.get_by_role("button", name="清除“显示或隐藏侧栏”的快捷键").click()
    save.click()
    expect(side).not_to_have_attribute("aria-keyshortcuts", "Shift+Meta+Y")
    workspace.reload()
    expect(side).to_be_visible()
    assert not side.get_attribute("aria-keyshortcuts")
    settings.get_by_role("button", name="恢复默认", exact=True).click()
    save.click()
    expect(side).to_have_attribute("aria-keyshortcuts", "Shift+Meta+L")
    expect(quick).to_have_text("⌃⇧空格")
    context.close()
    print("Recording, conflicts, filtering, saving, clearing, reopening and reset passed in WebKit.")


def verify_filing_settings(browser, url):
    page = browser.new_page()
    page.add_init_script(IPC + r"""
      const desktopInvoke = window.__TAURI_INTERNALS__.invoke;
      window.filingSaves = [];
      window.editKeys = [];
      const keys = new Set();
      let state = {enabled:false, endpoint:'', addressKind:'base', model:'jev-latest'};
      window.__TAURI_INTERNALS__.invoke = async (command, args) => {
        if (command === 'read_filing') return {...state};
        if (command === 'save_filing') {
          if (Object.keys(args).sort().join() !== 'addressKind,enabled,endpoint,model') throw Error('unexpected save fields');
          window.filingSaves.push(args);
          state = {...args};
          return state;
        }
        if (command !== 'filing_key') return desktopInvoke(command, args);
        if (Object.keys(args).sort().join() !== 'action,addressKind,endpoint') throw Error('unexpected key fields');
        const id = args.addressKind + ':' + args.endpoint;
        if (args.action === 'check') {
          if (window.locked) throw Error('钥匙串已锁定');
          if (args.endpoint === window.delayAddress) {
            return new Promise(resolve => {window.finishCheck = () => resolve(true)});
          }
        } else {
          window.editKeys.push(args);
          if (window.cancelKeyEntry) throw Error('已取消配置密钥');
          if (args.action === 'set') keys.add(id);
          else if (args.action === 'remove') keys.delete(id);
          else throw Error('unknown action');
        }
        return keys.has(id);
      };
    """)
    page.goto(url + "?view=settings")
    page.get_by_role("button", name="归档", exact=True).click()
    mode = page.locator("#filing-address-kind")
    address = page.locator("#filing-endpoint")
    model = page.locator("#filing-model")
    allow = page.locator("#filing-enabled")
    form = page.locator("form").filter(has=address)
    save = form.get_by_role("button", name="保存设置")
    key_status = page.locator("#filing-key-status")
    configure = page.get_by_role("button", name="配置密钥…")
    replace = page.get_by_role("button", name="更换密钥…")
    remove = page.get_by_role("button", name="移除密钥")
    expect(mode).to_have_value("base")
    expect(model).to_have_value("jev-latest")
    expect(configure).to_be_disabled()
    expect(key_status).to_contain_text("填写服务地址")
    expect(page.locator('input[type=password]')).to_have_count(0)

    base = "https://gateway.example.invalid/team"
    custom = "https://gateway.example.invalid/custom/classify"
    address.fill(base)
    model.fill("my-choice-model")
    allow.check()
    expect(key_status).to_have_text("此服务尚未配置密钥。")
    save.click()
    expect(form.get_by_role("status")).to_have_text("设置已保存。")
    settings = dict(enabled=True, addressKind="base", endpoint=base, model="my-choice-model")
    assert page.evaluate("filingSaves") == [settings]
    assert page.evaluate("editKeys") == []

    configure.click()
    expect(key_status).to_have_text("此服务已配置密钥。")
    page.evaluate("window.cancelKeyEntry = true")
    replace.click()
    expect(page.get_by_role("status").filter(has_text="已取消配置密钥")).to_be_visible()
    expect(key_status).to_have_text("此服务已配置密钥。")
    page.evaluate("window.cancelKeyEntry = false")
    mode.select_option("endpoint")
    address.fill(custom)
    model.fill("unsaved-model")
    expect(key_status).to_have_text("此服务尚未配置密钥。")
    expect(remove).to_have_count(0)
    configure.click()
    expect(replace).to_be_visible()
    assert page.evaluate("filingSaves") == [settings]
    assert page.evaluate("editKeys") == [
        dict(action="set", addressKind="base", endpoint=base),
        dict(action="set", addressKind="base", endpoint=base),
        dict(action="set", addressKind="endpoint", endpoint=custom),
    ]

    remove.click()
    expect(key_status).to_have_text("此服务尚未配置密钥。")
    expect(allow).to_be_checked()
    expect(model).to_have_value("unsaved-model")
    assert page.evaluate("editKeys.at(-1)") == dict(action="remove", addressKind="endpoint", endpoint=custom)
    assert page.evaluate("filingSaves") == [settings]
    mode.select_option("base")
    address.fill(base)
    expect(key_status).to_have_text("此服务已配置密钥。")
    page.evaluate("window.locked = true")
    mode.select_option("endpoint")
    expect(key_status).to_contain_text("无法确认此服务的密钥状态")
    expect(remove).to_have_count(0)
    page.evaluate("window.locked = false")

    delayed = "https://delayed.invalid/test"
    page.evaluate("(address) => {window.delayAddress = address}", delayed)
    address.fill(delayed)
    page.wait_for_function("typeof window.finishCheck === 'function'")
    address.fill(custom)
    expect(key_status).to_have_text("此服务尚未配置密钥。")
    page.evaluate("window.finishCheck()")
    expect(key_status).to_have_text("此服务尚未配置密钥。")
    expect(page.locator('input[type=password]')).to_have_count(0)
    page.close()
    print("Filing settings passed: independent settings and native key actions, per-address presence, cancellation and stale checks.")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--tree-focus", action="store_true")
    mode.add_argument("--shortcut-settings", action="store_true")
    mode.add_argument("--filing-settings", action="store_true")
    options = parser.parse_args()
    assert (PROJECT / "dist/index.html").is_file(), "Run pnpm build first"
    with ThreadingHTTPServer(("127.0.0.1", 0), partial(Static, directory=PROJECT / "dist")) as server:
        Thread(target=server.serve_forever, daemon=True).start()
        try:
            with sync_playwright() as playwright:
                browser = playwright.webkit.launch(executable_path=os.getenv("WEBKIT_EXECUTABLE"))
                url = f"http://127.0.0.1:{server.server_port}/"
                if options.shortcut_settings:
                    verify_shortcut_settings(browser, url)
                elif options.filing_settings:
                    verify_filing_settings(browser, url)
                else:
                    verify_tree_focus(browser, url)
                    if not options.tree_focus:
                        verify(browser, url)
                browser.close()
        finally:
            server.shutdown()
    print("WebKit checks passed.")
