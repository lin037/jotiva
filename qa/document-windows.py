"""Exercise the built editor and Worker in WebKit with a versioned IPC fixture."""
import argparse
import functools
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
import os
from pathlib import Path
import shutil
import tempfile
import threading

from playwright.sync_api import expect, sync_playwright

PROJECT = Path(__file__).resolve().parents[1]
IPC = (PROJECT / "qa/macos/browser-env.js").read_text() + r"""
const label = new URL(location.href).searchParams.get("window");
// Hold one real engine request so local typing can happen while it is in flight.
window.workerMetrics = {live: 0, created: 0, peak: 0};
window.Worker = class extends Worker {
  constructor(...args) {
    super(...args); ++workerMetrics.live; ++workerMetrics.created;
    workerMetrics.peak = Math.max(workerMetrics.peak, workerMetrics.live);
  }
  terminate() { --workerMetrics.live; super.terminate(); }
  postMessage(message, ...args) {
    if (window.holdSync && message.body?.transaction?.origin === "system") {
      window.holdSync = false;
      window.releaseSync = () => super.postMessage(message, ...args);
      return;
    }
    super.postMessage(message, ...args);
  }
};
const subscribers = new Map();
const bus = new BroadcastChannel("jot-test");
window.notify = (name, payload) => {
  for (const subscription of subscribers.values()) {
    if (subscription.name === name) subscription.handler({event: name, payload});
  }
};
bus.onmessage = ({data}) => { if (!window.pauseEvents) notify("desktop:notes-changed", data); };
let sequence = 0;
window.fileMenuCalls = [];
window.filingRequests = [];
window.filingDecisions = [];
window.diskAnnouncements = 0;
window.presentedFrames = [];
window.__TAURI_INTERNALS__ = {
  metadata: {currentWindow:{label}}, transformCallback: handler => handler,
  invoke: async (key, args = {}) => {
    if (key === "context_menu" || key === "copy_text" || key.endsWith("_action")) fileMenuCalls.push({key, args});
    switch(key) {
      case "context_menu": {
        if (!window.pickFileAction) return null;
        const index = args.labels.indexOf(window.pickFileAction);
        if (index === -1) throw Error("Missing file action: " + window.pickFileAction);
        return index;
      }
      case "open_document_window": window.separateDocument = args.id; return;
      case "note_action": if (args.action.kind === "duplicate") return fixtureNote("duplicate", args);
        throw Error("Unexpected note action " + args.action.kind);
      case "copy_text": return;
      case "plugin:event|listen":
        subscribers.set(++sequence, {name:args.event,handler:args.handler}); return sequence;
      case "plugin:event|unlisten": return;
      case "plugin:window|set_title": return;
      case "plugin:window|close": notify("tauri://close-requested"); return;
      case "live_terminals": return 0;
      case "close_editor_window": window.wasClosed = true; return;
      case "document_menu": window.menu = args; return;
      case "workspace_tab_menu": window.multipleTabs = args.multiple; return;
      case "read_settings": return {preferences:{quick:"Control+Shift+Space",search:null,dock:null,local:{}}};
      case "read_appearance": return {baseSize:null, systemSize:13, mode:"light", accent:null,
        workspace:{sidebar:null,outline:null}, stylesheet:null,
        editing:{font:"system",wrapCode:true,foldHeadings:true,toolbar:true,headingScale:null,ruledPaper:false}};
      case "inbox_directory": return "/tmp/Jotiva/其他";
      case "list_folders": case "list_sources": return [];
      case "workspace_roots": return window.previewIds ? ["/work"] : [];
      case "workspace_children": return previewIds.map(id => ({name:id+".md",path:"/work/"+id+".md",directory:false}));
      case "open_library_file": return Number(args.path.split("/").pop().split(".")[0]);
      case "list_notes": return {items:[],more:false};
      case "search_document_files": return [];
      case "search_notes": return {current:[],attached:[],documents:[],more:false};
      case "take_dropped_tabs": return (window.droppedTabs ?? []).splice(0);
      case "read_tab_titles": return args.ids.map(id => ({id,title:window.tabNames?.[id] ?? "文档 "+id}));
      case "initial_note": return 1;
      case "quick_view": return {epoch:1,kind:"edit",id:2,pinned:false,standalone:true,source:null,
        cursor: JSON.parse(sessionStorage.getItem("fixture-bookmark"))?.state};
      case "quick_inactive": sessionStorage.setItem("fixture-bookmark", JSON.stringify(args.bookmark)); return;
      case "quick_presented":
        presentedFrames.push({epoch:args.epoch, mode:document.querySelector("#app").dataset.mode,
          text:document.querySelector(".cm-content")?.innerText, editable:document.activeElement?.isContentEditable});
        return;
      case "read_card": {
        const note = await fixtureNote("read", args);
        return {id:note.id, title:"文档 " + note.id, source:note.source, folder:null, attached:false,
          version:note.version, file:note.file, archivePath:note.archivePath};
      }
      case "open_editor_note": case "read_note": case "refresh_editor_note": {
        if (key === "open_editor_note" && args.id === window.holdOpening) {
          window.holdOpening = null;
          await new Promise(resolve => { window.continueOpening = resolve; });
        }
        const {note, changed} = await fixtureNote("open", args);
        if (changed) {
          ++diskAnnouncements;
          bus.postMessage(args.id);
          queueMicrotask(() => notify("desktop:notes-changed", args.id));
        }
        return key !== "refresh_editor_note" || note.version > args.version ? note : null;
      }
      case "save_note": {
        const version = await fixtureNote("save", args);
        bus.postMessage(args.id);
        queueMicrotask(() => notify("desktop:notes-changed", args.id));
        return version;
      }
      case "new_note": return fixtureNote("new", {});
      case "file_note": {
        const {id, version, manual} = args;
        window.filingRequests.push({id, version, manual});
        if (window.filingError) throw Error(window.filingError);
        const ticket = window.filingRequests.length;
        args.started.onmessage(ticket);
        if (window.holdFiling) await new Promise(resolve => { window.releaseFiling = resolve; });
        return window.filingReply?.kind === "suggestion" ? {...window.filingReply, ticket} : window.filingReply;
      }
      case "cancel_filing": case "confirm_filing":
        window.filingDecisions.push({command: key, ...args});
        return "已归档至「研发」";
      case "release_note": return;
      default: throw Error("Unexpected IPC " + key);
    }
  }
};
window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {unregisterListener: (_name,id) => subscribers.delete(id)};
"""

class Assets(SimpleHTTPRequestHandler):
    def log_message(self, *args):
        pass


# Just like the native note store, all windows read the same committed revision.
# Broadcasting from one WebKit page can outrun localStorage replication to a peer.
records = {}
external_edits = {}


def fixture_note(action, args):
    if action == "new":
        id = max(records) + 1
        records[id] = {"id": id, "source": "Jotiva", "version": 0, "markdown": ""}
        return id
    note = records[args["id"]]
    if action == "duplicate":
        duplicate = max(records) + 1
        records[duplicate] = {**note, "id": duplicate}
        return duplicate
    if action == "external":
        external_edits[note["id"]] = args["markdown"]
        return
    if action == "open":
        disk = external_edits.pop(note["id"], note["markdown"])
        changed = disk != note["markdown"]
        if changed:
            note["version"] += 1
            note["markdown"] = disk
        return {"note": note, "changed": changed}
    if action == "read":
        return note
    assert action == "save"
    if args["version"] != note["version"]:
        raise ValueError("revision conflict")
    note["version"] += 1
    note["markdown"] = args["markdown"]
    return note["version"]


def insert(page, text, at):
    page.bring_to_front()
    page.evaluate("""([text, at]) => {
      const input = document.querySelector('.cm-content');
      input.focus();
      const range = document.createRange();
      range.selectNodeContents(input);
      if (at !== "all") range.collapse(at === "start");
      const selection = getSelection();
      selection.removeAllRanges(); selection.addRange(range);
      if (!document.execCommand("insertText", false, text)) throw Error("WebKit rejected input");
    }""", [text, at])


def saved(page, text):
    try:
        page.wait_for_function("async text => (await fixtureNote('read', {id:1})).markdown === text", arg=text)
    except Exception:
        print("Expected", text, "but saved / visible / status are",
              records[1], page.evaluate("[document.querySelector('.cm-content').innerText,document.querySelector('[role=status]').innerText]"))
        raise


def visible(page, text):
    try:
        page.wait_for_function("text => document.querySelector('.cm-content').innerText.trim() === text", arg=text.strip())
        page.wait_for_function("count => document.querySelector('.workspace-count').textContent === count",
                               arg=f"{sum(not char.isspace() for char in text)} 字符")
    except Exception:
        print("Expected visible", repr(text), "in", page.url, "but visible / status / error are", page.evaluate(
            "[document.querySelector('.cm-content').innerText, document.querySelector('[role=status]').textContent, document.querySelector('.error').textContent]"))
        raise


def command(page, name):
    page.evaluate("name => notify('desktop:editor-action', name)", name)


def check_filename_menus(main, document):
    path = "/outside workspace/讨论 稿.md"
    records[1]["file"] = path
    for page in (main, document):
        page.bring_to_front()
        page.reload()
        filename = page.locator(f'.workspace-location[data-file-menu-path="{path}"]')
        filename.wait_for()
        assert filename.locator(".workspace-path-label").text_content() == path
        assert filename.get_attribute("title") == path
        box = filename.bounding_box()
        for index, pick in enumerate([None, "复制绝对路径", "移到废纸篓…"]):
            page.evaluate("pick => { window.pickFileAction = pick; }", pick)
            filename.click(button="right", position={"x": 4, "y": 4})
            page.wait_for_function("n => fileMenuCalls.filter(call => call.key === 'context_menu').length === n", arg=index + 1)
            args = page.evaluate("fileMenuCalls.filter(call => call.key === 'context_menu').at(-1).args")
            assert abs(args["x"] - box["x"] - 4) < 2 and abs(args["y"] - box["y"] - 4) < 2
            assert "复制绝对路径" in args["labels"] and "移到废纸篓…" in args["labels"]
            if pick == "复制绝对路径":
                page.wait_for_function("fileMenuCalls.some(call => call.key === 'copy_text')")
                assert page.evaluate("fileMenuCalls.find(call => call.key === 'copy_text').args") == {"text": path}
            if pick == "移到废纸篓…":
                confirm = page.get_by_role("dialog")
                assert "讨论 稿.md" in confirm.inner_text()
                confirm.get_by_role("button", name="取消").click()
                confirm.wait_for(state="hidden")
        assert page.evaluate("fileMenuCalls.map(call => call.key)") == ["context_menu", "context_menu", "copy_text", "context_menu"]
        assert page.locator(".error").is_hidden()
        assert records[1]["file"] == path
    text = records[1]["markdown"]
    for page, peer, suffix in [(main, document, " 主窗"), (document, main, " 独立窗")]:
        page.bring_to_front()
        page.locator(".cm-content").click()
        page.keyboard.press("Meta+ArrowDown")
        page.keyboard.insert_text(suffix)
        page.keyboard.press("Meta+s")
        text += suffix
        saved(page, text)
        peer.bring_to_front()
        visible(peer, text)
        assert records[1]["file"] == path
    del records[1]["file"]
    archive = "/tmp/Jotiva/其他/" + "长目录/" * 20 + "会议.md"
    records[1]["archivePath"] = archive
    for page in (main, document):
        page.bring_to_front()
        page.set_viewport_size({"width": 580, "height": 400})
        page.reload()
        path_button = page.get_by_role("button", name="文件操作，" + archive, exact=True)
        path_button.wait_for()
        assert path_button.locator(".workspace-path-label").text_content() == archive
        assert path_button.locator(".workspace-path-label").evaluate("""label => {
          const style = getComputedStyle(label);
          return label.scrollWidth > label.clientWidth && style.direction === "rtl" && style.textOverflow === "ellipsis";
        }""")
        assert page.locator(".workspace-stats").evaluate("""e => {
          const path = document.querySelector('.workspace-metadata').getBoundingClientRect();
          return path.right < e.getBoundingClientRect().left && e.getBoundingClientRect().right <= innerWidth
            && [...e.children].every(child => child.scrollWidth <= child.clientWidth + 1);
        }"""), "The pathname displaced or truncated the character count or save status"
        page.evaluate("window.pickFileAction = '移到归档目录…'")
        path_button.click()
        dialog = page.get_by_role("dialog")
        dialog.wait_for()
        assert dialog.get_by_text("移动便签", exact=True).is_visible()
        dialog.get_by_role("button", name="取消", exact=True).click()
    print("Both windows exposed the full managed inbox path, retained its filename under truncation and opened the move dialog.", flush=True)
    print("Both filename menus sent viewport coordinates and copied the external path. After cancelling Trash, both windows typed, saved and synchronized.", flush=True)


def check_tabs_and_edges(page):
    page.set_viewport_size({"width": 900, "height": 650})
    page.bring_to_front()
    long_text = "\n\n".join(f"# {i} 会议纪要\n\n" + "讨论项目安排和待办事项。" * (i % 18 + 1) + "\n\n- [ ] 跟进" for i in range(320))
    for id in range(30, 50):
        records[id] = {"id": id, "source": "Jotiva", "version": 1, "markdown": long_text if id == 30 else f"文档 {id}"}
    before = page.evaluate("workerMetrics.created")
    page.evaluate("() => { window.droppedTabs = Array.from({length:20}, (_,i) => 30+i).concat(30).map(id => ({id, title:'文档 '+id})); notify('desktop:dropped-tabs'); }")
    active = page.get_by_role("tab", name="文档 30", exact=True)
    page.wait_for_function("document.querySelector('[role=tab][aria-selected=true]')?.textContent === '文档 30'")
    assert page.get_by_role("tab").count() == 21
    assert page.evaluate("workerMetrics.live === 1 && workerMetrics.created === " + str(before + 1))
    page.wait_for_timeout(350)
    for end in (True, False, True, False):
        offsets = page.evaluate("""async end => {
          const view = document.querySelector('.cm-scroller');
          view.scrollTop = end ? view.scrollHeight : 0;
          const deltas = [];
          for (let i = 0; i < 26; ++i) {
            // Sample after WebKit has delivered the content ResizeObserver for this frame.
            await new Promise(done => requestAnimationFrame(() => setTimeout(done, 0)));
            deltas.push(end ? view.scrollHeight - view.clientHeight - view.scrollTop : view.scrollTop);
          }
          return deltas;
        }""", end)
        assert max(abs(delta) for delta in offsets) <= 1, (end, offsets)
    scroll = page.locator(".cm-scroller")
    scroll.hover()
    # WebKitGTK caps the distance delivered by one wheel event. Use real wheel
    # input until the same bottom-edge assertion holds on either WebKit port.
    for _ in range(80):
        if scroll.evaluate("e => e.scrollTop > 0 && e.scrollHeight - e.clientHeight - e.scrollTop <= 1"):
            break
        page.mouse.wheel(0, 300000)
        page.wait_for_timeout(100)
    page.wait_for_function("() => { const e=document.querySelector('.cm-scroller'); return e.scrollTop > 0 && e.scrollHeight - e.clientHeight - e.scrollTop <= 1; }")
    page.mouse.wheel(0, -600)
    page.wait_for_function("() => { const e=document.querySelector('.cm-scroller'); return e.scrollHeight - e.clientHeight - e.scrollTop > 100; }")
    command(page, "mode.source")
    page.wait_for_function("menu.mode === 'source'")
    page.locator(".cm-content").click()
    page.keyboard.press("Meta+ArrowDown")
    page.keyboard.press("Shift+ArrowLeft")
    page.wait_for_timeout(180)
    selection = page.evaluate("getSelection().toString()")
    assert selection
    page.keyboard.press("Control+Tab")
    page.wait_for_function("document.querySelector('[role=tab][aria-selected=true]')?.textContent === '文档 1'")
    page.keyboard.press("Control+Tab")
    page.wait_for_function("menu.mode === 'source' && document.querySelector('[role=tab][aria-selected=true]')?.textContent === '文档 30'")
    page.wait_for_function("text => getSelection().toString() === text", arg=selection)
    assert page.locator(".cm-scroller").evaluate("e => e.scrollTop > e.clientHeight")
    # Closing a background label does not create an editor or change the current selection.
    before = page.evaluate("workerMetrics.created")
    page.get_by_role("button", name="关闭标签，文档 32", exact=True).click()
    assert active.get_attribute("aria-selected") == "true" and page.evaluate("workerMetrics.created") == before
    assert page.evaluate("multipleTabs && !window.wasClosed")
    page.keyboard.press("Meta+w")
    page.wait_for_function("document.querySelector('[role=tab][aria-selected=true]')?.textContent === '文档 1'")
    assert page.evaluate("workerMetrics.live === 1")
    page.evaluate("() => { window.tabNames={31:'修改后的标题'}; notify('desktop:files-changed'); }")
    page.get_by_role("tab", name="修改后的标题").wait_for()
    # AppKit consumes Cmd+W as a menu equivalent. It sends this same action
    # only for a multi-tab main window; red close still asks to close the window.
    page.evaluate("notify('desktop:editor-action','close-tab')")
    page.wait_for_function("document.querySelector('[role=tab][aria-selected=true]')?.textContent === '文档 31'")
    assert not page.evaluate("!!window.wasClosed")
    page.reload()
    page.wait_for_function("window.menu?.exists")
    # A single tab leaves the equivalent unconsumed for the system Close Window command.
    assert page.evaluate("""window.dispatchEvent(new KeyboardEvent('keydown',
      {code:'KeyW', key:'w', metaKey:true, bubbles:true, cancelable:true}))""")
    page.evaluate("notify('tauri://close-requested')")
    page.wait_for_function("window.wasClosed")
    page.reload()
    page.wait_for_function("window.menu?.exists")
    page.evaluate("() => { window.droppedTabs = [{id:31,title:'文档 31'}]; notify('desktop:dropped-tabs'); }")
    page.wait_for_function("window.multipleTabs && document.querySelector('[role=tab][aria-selected=true]')?.textContent === '文档 31'")
    page.evaluate("notify('tauri://close-requested')")
    page.wait_for_function("window.wasClosed")
    print("20 dropped files used one editor and one worker. Ctrl+Tab restored the previous document, mode, selection and scroll. Four rapid boundary jumps stayed within one pixel.", flush=True)


def check_tab_history(page):
    records.update({id: {"id": id, "source": "Jotiva", "version": 1, "markdown": "相同的起点"} for id in [60, 61]})

    def active(id):
        page.wait_for_function("""id => document.querySelector('[role=tab][aria-selected=true]')?.textContent
            === '文档 '+id && document.activeElement?.isContentEditable""", arg=id)

    def select(id):
        page.get_by_role("tab", name=f"文档 {id}", exact=True).click()
        active(id)

    def drop(ids):
        page.evaluate("""ids => { window.droppedTabs = ids.map(id => ({id, title:'文档 '+id}));
            notify('desktop:dropped-tabs'); }""", ids)
        active(ids[0])

    drop([60, 61])
    insert(page, "甲", "end")
    select(61)  # Switch before the debounce has saved the change.
    assert records[60]["markdown"] == "相同的起点甲"
    insert(page, "乙", "end")
    select(60)
    page.keyboard.press("Meta+z")
    visible(page, "相同的起点")
    select(61)
    page.keyboard.press("Meta+z")
    visible(page, "相同的起点")
    select(60)
    page.keyboard.press("Meta+Shift+z")
    visible(page, "相同的起点甲")
    select(61)
    page.keyboard.press("Meta+Shift+z")
    visible(page, "相同的起点乙")

    # A parked snapshot is valid only for the source it was captured against.
    select(60)
    fixture_note("external", {"id": 61, "markdown": "磁盘上的新正文"})
    select(61)
    visible(page, "磁盘上的新正文")
    page.keyboard.press("Meta+z")
    insert(page, "！", "end")
    visible(page, "磁盘上的新正文！")

    # Closing a tab drops its history even if its source is unchanged when opened again.
    page.get_by_role("button", name="关闭标签，文档 60", exact=True).click()
    drop([60])
    page.keyboard.press("Meta+z")
    insert(page, "。", "end")
    visible(page, "相同的起点甲。")
    # A command can request a tab change before compositionend. Keep the
    # editor alive until the input method confirms the composition.
    page.evaluate("""() => {
      const input = document.querySelector('.cm-content');
      input.dispatchEvent(new CompositionEvent('compositionstart', {bubbles:true}));
      const other = [...document.querySelectorAll('[role=tab]')].find(tab => tab.textContent === '文档 61');
      other.click();
      window.endComposition = () => {
        if (!input.isConnected) throw Error('Editor was disposed before IME confirmation');
        input.dispatchEvent(new CompositionEvent('compositionend', {bubbles:true}));
      };
    }""")
    page.wait_for_function("!document.activeElement?.isContentEditable")
    page.evaluate("endComposition()")
    active(61)
    assert records[60]["markdown"] == "相同的起点甲。"
    assert page.evaluate("workerMetrics.live === 1 && workerMetrics.peak === 1")
    def menu(id, action):
        page.evaluate("action => window.pickFileAction = action", action)
        page.locator(f'[role=tab][data-note="{id}"]').click(button="right")

    insert(page, "!", "end")
    menu(61, "复制文档内容")
    page.wait_for_function("fileMenuCalls.some(call => call.key === 'copy_text' && call.args.text === '磁盘上的新正文！!')")
    menu(61, "制作副本")
    active(62)
    assert records[62]["markdown"] == records[61]["markdown"] == "磁盘上的新正文！!"
    menu(60, "在新窗口打开")
    page.wait_for_function("separateDocument === 60")
    menu(61, "关闭右侧标签")
    page.wait_for_function("document.querySelectorAll('[role=tab]').length === 2")
    menu(61, "关闭左侧标签")
    page.wait_for_function("document.querySelectorAll('[role=tab]').length === 1")
    drop([60, 1])
    menu(61, "关闭其他标签")
    page.wait_for_function("document.querySelectorAll('[role=tab]').length === 1")
    assert page.locator('[role=tab][aria-selected=true]').get_attribute("data-note") == "61"
    print("Two tabs restored independent Undo and Redo with one live Worker. Switching waited for compositionend. External changes and closed tabs discarded stale history.", flush=True)


def check_quick_resume(page):
    epoch = 1

    def show(kind, **options):
        nonlocal epoch
        epoch += 1
        page.evaluate("view => notify('desktop:quick-view', view)", {"kind": kind, "epoch": epoch, **options})
        page.wait_for_function("kind => document.querySelector('#app').dataset.mode === kind", arg=kind)
        if kind == "edit":
            page.wait_for_function("document.activeElement?.isContentEditable && menu.exists")
        if kind != "hidden":
            page.wait_for_function("epoch => presentedFrames.at(-1)?.epoch === epoch", arg=epoch)
        else:
            assert page.evaluate("epoch => !presentedFrames.some(frame => frame.epoch === epoch)", epoch)

    current = {"id": 2, "pinned": False, "standalone": False, "source": "会议"}
    card = {"id": 2, "title": "会议", "excerpt": "记录", "source": "测试", "folder": None, "attached": True}
    folded = {"note": card, "rail": None, "docked": False}
    peer = page.context.new_page()
    peer.goto(page.url.replace("window=quick", "window=document-2"))
    peer.wait_for_function("window.menu?.exists")
    page.bring_to_front()
    before = page.evaluate("workerMetrics.created")
    page.evaluate("window.originalInput = document.querySelector('.cm-content')")
    insert(page, "-quick", "end")
    show("summary", **folded)
    assert records[2]["markdown"] == "base text-quick"
    visible(peer, "base text-quick")
    show("edit", **current)
    assert page.evaluate("document.activeElement === window.originalInput && getSelection().focusOffset > 0")
    command(page, "history.undo")
    visible(page, "base text")
    show("hidden")
    assert records[2]["markdown"] == "base text"
    visible(peer, "base text")
    # A user saved externally while the host was hidden. There is no desktop event.
    fixture_note("external", {"id": 2, "markdown": "external edit"})
    show("edit", **current)
    visible(page, "external edit")
    visible(peer, "external edit")
    assert page.evaluate("diskAnnouncements") == 1 and peer.evaluate("diskAnnouncements") == 0
    assert page.evaluate("workerMetrics.created") == before
    insert(page, "-parked", "end")
    show("summary", **folded)
    # A superseded presentation must not cancel the parked editor's idle deadline.
    page.evaluate("views => views.forEach(view => notify('desktop:quick-view', view))", [
        {"kind": "edit", "epoch": epoch + 1, **current},
        {"kind": "search", "epoch": epoch + 2, "tiled": False, "host": None},
    ])
    epoch += 2
    page.wait_for_function("document.querySelector('#app').dataset.mode === 'search'")
    page.wait_for_function("epoch => presentedFrames.at(-1)?.epoch === epoch", arg=epoch)
    assert page.evaluate("epoch => !presentedFrames.some(frame => frame.epoch === epoch)", epoch-1)
    page.wait_for_function("workerMetrics.live === 0", timeout=21000)
    show("edit", **current)
    assert page.evaluate("workerMetrics.created") == before + 1
    visible(page, "external edit-parked")
    command(page, "history.undo")
    visible(page, "external edit")
    command(page, "history.redo")
    visible(page, "external edit-parked")
    show("summary", **folded)
    long_text = "\n".join(f"第 {index} 行 · 临时记录" for index in range(320))
    fixture_note("external", {"id": 2, "markdown": long_text})
    show("edit", **current)
    page.wait_for_function("count => document.querySelector('.workspace-count').textContent === count.toLocaleString()+' 字符'",
                           arg=sum(not char.isspace() for char in long_text))
    command(page, "mode.source")
    page.wait_for_function("menu.mode === 'source'")
    page.keyboard.press("Meta+ArrowDown")
    page.keyboard.press("Shift+ArrowLeft")
    page.wait_for_function("getSelection().toString() === '录' && document.querySelector('.cm-scroller').scrollTop > innerHeight")
    show("hidden")
    page.wait_for_function("JSON.parse(sessionStorage.getItem('fixture-bookmark'))?.state.mode === 'source'")
    position = page.evaluate("JSON.parse(sessionStorage.getItem('fixture-bookmark')).state")
    assert set(position) == {"anchor", "head", "top", "mode"}
    assert position["head"] == len(long_text) - 1 and position["top"] > page.viewport_size["height"]
    # Simulate the native WebView being reclaimed. Only its numeric bookmark/mode survives.
    page.reload()
    page.wait_for_function("window.menu?.mode === 'source' && document.activeElement?.isContentEditable")
    page.wait_for_function("getSelection().toString() === '录' && document.querySelector('.cm-scroller').scrollTop > innerHeight")
    def hold_open(id):
        nonlocal epoch
        epoch += 1
        page.evaluate("""view => {
          window.holdOpening = view.id;
          window.continueOpening = null;
          notify('desktop:quick-view', view);
        }""", {**current, "kind": "edit", "epoch": epoch, "id": id})
        page.wait_for_function("typeof continueOpening === 'function'")
        assert page.evaluate("epoch => !presentedFrames.some(frame => frame.epoch === epoch)", epoch)

    hold_open(1)
    page.evaluate("continueOpening()")
    page.wait_for_function("epoch => presentedFrames.at(-1)?.epoch === epoch", arg=epoch)
    frame = page.evaluate("presentedFrames.at(-1)")
    assert frame["mode"] == "edit" and frame["editable"] and frame["text"].strip() == records[1]["markdown"]
    page.wait_for_function("workerMetrics.created === 2 && menu.exists")
    hold_open(2)
    page.evaluate("""summary => {
      notify('desktop:quick-view', summary);
      continueOpening();
    }""", {**folded, "kind": "summary", "epoch": epoch+1})
    page.wait_for_function("epoch => presentedFrames.at(-1)?.epoch === epoch", arg=epoch+1)
    assert page.evaluate("epoch => !presentedFrames.some(frame => frame.epoch === epoch)", epoch)
    epoch += 1
    for mode, selector, destination in (("summary", ".quick-summary-title", 1), ("search", ".quick-query input", 2)):
        if mode == "search":
            show("search", tiled=False, host=None)
        source = page.locator(selector)
        assert source.is_visible()
        hold_open(destination)
        assert page.locator("#app").get_attribute("data-mode") == mode and source.is_visible()
        page.evaluate("continueOpening()")
        page.wait_for_function("epoch => presentedFrames.at(-1)?.epoch === epoch", arg=epoch)
        assert page.evaluate("presentedFrames.at(-1).mode === 'edit' && presentedFrames.at(-1).editable")
        assert not source.is_visible()
    assert page.evaluate("workerMetrics.peak") == 1
    print("Quick resume retained input and undo. An idle Worker was released, then rebuilt with working Undo and Redo. Reload restored the bookmark. Only ready, current frames were acknowledged.", flush=True)


def check_document_sync(context, url):
    pages = []
    for role in ("main", "document-1", "quick"):
        page = context.new_page()
        page.set_default_timeout(9000)
        page.goto(f"{url}?window={role}")
        page.wait_for_function("window.menu?.exists")
        for mode in ("source", "live-preview"):
            page.keyboard.press("Meta+Slash")
            page.wait_for_function("m => window.menu?.mode === m", arg=mode)
            assert page.evaluate("menu.editable")
        pages.append(page)
    main, document, _ = pages
    command(document, "mode.source")
    document.wait_for_function("menu.mode === 'source'")
    insert(main, "-main", "end")
    command(main, "save")
    saved(main, "base text-main")
    visible(document, "base text-main")
    document.wait_for_function("document.querySelector('.workspace-count').textContent === '13 字符' && menu.editable")
    command(document, "mode.live-preview")
    document.wait_for_function("menu.mode === 'live-preview'")
    visible(document, "base text-main")
    insert(document, "doc-", "start")
    command(document, "save")
    saved(main, "doc-base text-main")
    visible(main, "doc-base text-main")
    command(main, "history.undo")
    command(main, "save")
    saved(main, "doc-base text")
    visible(document, "doc-base text")
    # Both change a different range before the suspended window receives a notification.
    document.evaluate("window.pauseEvents = true")
    insert(main, "-again", "end")
    command(main, "save")
    saved(main, "doc-base text-again")
    insert(document, "start-", "start")
    command(document, "save")
    saved(main, "start-doc-base text-again")
    visible(main, "start-doc-base text-again")
    insert(main, "other version", "all")
    command(main, "save")
    saved(main, "other version")
    insert(document, "my version", "all")
    command(document, "save")
    document.get_by_role("button", name="另存编辑并重载").click()
    visible(document, "other version")
    assert records[3]["markdown"] == "my version"
    # Start a separate typing gesture after the released editor's 500 ms undo grouping interval.
    main.wait_for_timeout(550)
    document.evaluate("window.pauseEvents = false")
    main.evaluate("window.holdSync = true")
    insert(document, "-remote", "end")
    command(document, "save")
    main.wait_for_function("!!window.releaseSync")
    insert(main, "typed-", "start")
    main.evaluate("window.releaseSync()")
    command(main, "save")
    saved(main, "typed-other version-remote")
    visible(document, "typed-other version-remote")
    command(main, "history.undo")
    command(main, "save")
    saved(main, "other version-remote")
    visible(document, "other version-remote")
    # Removing the active document before autosave must recover pending text.
    insert(document, " while deleting", "end")
    document.evaluate("notify('desktop:notes-removed', [1])")
    document.wait_for_function("window.wasClosed")
    assert records[4]["markdown"] == "other version-remote while deleting"
    saved(main, "other version-remote")
    check_filename_menus(main, document)
    print("Document sync passed: mode changes, concurrent edits, undo isolation and conflict recovery.")


def check_filing_triggers(page):
    body = "本周研发会议的记录，包括代码审查结论和下周的测试安排。"
    insert(page, body, "all")
    saved(page, body)
    page.keyboard.press("Meta+s")
    command(page, "save")
    action = page.get_by_role("button", name="用 Jev 归档", exact=True)
    expect(action).to_be_enabled()
    assert page.evaluate("filingRequests.length") == 0
    page.evaluate("window.filingReply = {kind:'setup'}")
    action.click()
    expect(page.get_by_role("button", name="打开 Jev 归档设置")).to_be_visible()
    page.evaluate("window.filingError = '模拟网络失败'")
    command(page, "file")
    status = page.locator(".workspace-stats [role=status]")
    expect(status).to_contain_text("模拟网络失败")
    saved(page, body)
    page.evaluate("window.filingError = null; window.filingReply = {kind:'message',message:'保留在其他'}")
    action.click()
    expect(status).to_have_text("保留在其他")
    assert page.evaluate("filingDecisions") == []
    page.evaluate("window.filingReply = {kind:'suggestion',folder:'研发'}")
    action.click()
    prompt = page.get_by_role("dialog", name="Jev 建议归入「研发」")
    expect(prompt).to_be_visible()
    assert page.evaluate("filingDecisions") == []
    prompt.get_by_role("button", name="取消").click()
    expect(status).to_have_text("便签保留在「其他」")
    action.click()
    prompt.get_by_role("button", name="移到「研发」").click()
    expect(status).to_have_text("已归档至「研发」")
    assert page.evaluate("filingDecisions") == [
        {"command":"cancel_filing", "id":1, "version":2, "ticket":4},
        {"command":"confirm_filing", "id":1, "version":2, "ticket":5},
    ]
    page.evaluate("window.holdFiling = true")
    action.click()
    page.wait_for_function("typeof releaseFiling === 'function'")
    insert(page, "追加修改", "end")
    saved(page, body + "追加修改")
    page.evaluate("window.releaseFiling()")
    expect(action).to_be_enabled()
    expect(prompt).not_to_be_visible()
    assert page.evaluate("filingDecisions.at(-1)") == {"command":"cancel_filing", "id":1, "version":2, "ticket":6}
    assert page.evaluate("filingRequests") == [{"id":1, "version":2, "manual":True}] * 6
    page.evaluate("notify('tauri://close-requested')")
    page.wait_for_function("window.wasClosed")
    assert page.evaluate("filingRequests.length") == 6
    print("Filing IPC passed: saves and close stay local, a manual request offers a suggestion, and only confirmation moves the note. Editing cancels stale suggestions.")


def check_missing_history(page, url):
    records.update({id: {"id": id, "source": "Jotiva", "version": 1, "markdown": "相同的起点"} for id in [60, 61]})
    warnings = []
    page.on("console", lambda message: warnings.append(message.text) if message.type == "warning" else None)
    page.goto(url)
    page.wait_for_function("window.menu?.exists")
    page.evaluate("""() => { window.droppedTabs = [60,61].map(id => ({id,title:'文档 '+id}));
          notify('desktop:dropped-tabs'); }""")
    page.wait_for_function("document.querySelector('[role=tab][aria-selected=true]')?.textContent === '文档 60'")
    insert(page, "已保存", "end")
    for id in [61, 60, 61]:
        page.get_by_role("tab", name=f"文档 {id}", exact=True).click()
        page.wait_for_function("""id => document.querySelector('[role=tab][aria-selected=true]')?.textContent === '文档 '+id
            && document.activeElement?.isContentEditable""", arg=id)
    assert "已保存" in records[60]["markdown"]
    assert page.evaluate("window.qaMissingHistory === true")
    assert any("未保留本次撤销记录" in w for w in warnings), warnings
    assert page.evaluate("workerMetrics.peak === 1")
    print("Missing captureHistory reproduced. Markdown saved and three further switches remained editable.", flush=True)


def check_preview_tabs(page):
    for id in range(3, 7):
        records[id] = {"id": id, "source": "Jotiva", "version": 1, "markdown": f"document {id}"}
    page.evaluate("window.previewIds = [2,3,4,5,6]; notify('desktop:files-changed')")
    page.locator('[data-directory="/work"]').click()
    def row(id):
        return page.locator(f'[data-file="/work/{id}.md"]')
    def tab(id):
        return page.locator(f'[role=tab][data-note="{id}"]')
    def active(id):
        expect(tab(id)).to_have_attribute("aria-selected", "true")
    row(2).click()
    active(2)
    expect(page.locator("[data-preview] [role=tab]")).to_have_attribute("data-note", "2")
    row(3).click()
    active(3)
    expect(tab(2)).to_have_count(0)
    editor = page.locator(".cm-content")
    editor.press("End")
    page.keyboard.insert_text(" edited")
    expect(page.locator("[data-preview]")).to_have_count(0)
    row(4).click()
    active(4)
    row(4).dblclick()
    expect(page.locator("[data-preview]")).to_have_count(0)
    row(5).click()
    active(5)
    expect(tab(3)).to_be_visible()
    expect(tab(4)).to_be_visible()
    page.evaluate("""async () => {
      const value = await __TAURI_INTERNALS__.invoke('read_appearance');
      value.editing.previewTabs = false;
      notify('desktop:appearance-changed', value);
    }""")
    expect(page.locator("[data-preview]")).to_have_count(0)
    row(6).click()
    active(6)
    expect(tab(5)).to_be_visible()
    expect(page.locator("[data-preview]")).to_have_count(0)
    assert records[3]["markdown"] == "document 3 edited"
    expect(page.locator(".error")).to_be_hidden()
    print("Preview replacement, typing, double-click pinning and opt-out passed.")


def main():
    cases = {
        "sync": ("main", "Multi-window revision sync, conflicts, document actions and tabs"),
        "preview": ("main", "Preview tab replacement, pinning and opt-out"),
        "history": ("main", "Tab undo/redo and history reset after an external edit"),
        "resume": ("quick", "Quick note resume, Worker reclamation and stale frames"),
        "capacity": ("main", "Many tabs and scroll stability in a long document"),
        "filing": ("main", "Filing triggers and manual retry with mocked IPC"),
        "caret": ("main", "Native caret pixels after typing a Markdown heading"),
        "history-compat": ("main", "Diagnostic: replay a missing captureHistory method"),
    }
    parser = argparse.ArgumentParser(description=__doc__)
    for name, (_, description) in cases.items():
        parser.add_argument(f"--{name}", action="append_const", const=name, dest="cases", help=description)
    selected = parser.parse_args().cases or ["sync", "history", "resume", "preview"]
    dist = PROJECT / "dist"
    if not (dist / "index.html").is_file():
        parser.error("Run pnpm build first")
    if "history-compat" in selected and len(selected) != 1:
        parser.error("--history-compat requires its own invocation")
    if selected == ["history-compat"]:
        # Inject a fault into a copy. The default suite uses the shipped component unchanged.
        with tempfile.TemporaryDirectory(prefix="jot-history-compat-") as directory:
            copied = Path(directory) / "dist"
            shutil.copytree(dist, copied)
            component, = copied.glob("assets/component-*.js")
            source = component.read_text()
            assert "captureHistory(){" in source, "Update the fault injection for this editor bundle"
            component.write_text("globalThis.qaMissingHistory = true;\n" + source.replace("captureHistory(){", "missingHistory(){"))
            run(copied, selected, cases)
    else:
        run(dist, selected, cases)


def run(dist, selected, cases):
    with ThreadingHTTPServer(("127.0.0.1", 0), functools.partial(Assets, directory=dist)) as server:
        threading.Thread(target=server.serve_forever, daemon=True).start()
        try:
            with sync_playwright() as playwright:
                browser = playwright.webkit.launch(executable_path=os.getenv("WEBKIT_EXECUTABLE"))
                base = f"http://127.0.0.1:{server.server_port}/"
                for case in selected:
                    print(f"WebKit: {case}", flush=True)
                    records.clear()
                    records.update({id: {"id": id, "source": "Jotiva", "version": 1, "markdown": "base text"} for id in [1, 2]})
                    external_edits.clear()
                    context = browser.new_context()
                    context.expose_function("fixtureNote", fixture_note)
                    context.add_init_script(IPC)
                    if case == "sync":
                        check_document_sync(context, base)
                    else:
                        url = f"{base}?window={cases[case][0]}"
                        if case == "caret":
                            records[1]["markdown"] = ""
                        page = context.new_page()
                        page.set_default_timeout(9000)
                        if case == "history-compat":
                            check_missing_history(page, url)
                        else:
                            page.goto(url)
                            page.wait_for_function("window.menu?.exists")
                            if case == "caret":
                                from scenarios.caret import check_heading_caret
                                check_heading_caret(page)
                            elif case == "preview":
                                check_preview_tabs(page)
                            elif case == "resume":
                                check_quick_resume(page)
                            elif case == "history":
                                check_tab_history(page)
                            elif case == "capacity":
                                check_tabs_and_edges(page)
                            elif case == "filing":
                                check_filing_triggers(page)
                    context.close()
                browser.close()
        finally:
            server.shutdown()


if __name__ == "__main__":
    main()
