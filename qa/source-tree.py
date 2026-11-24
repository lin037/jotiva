"""Exercise the sidebar in WebKit with isolated IPC fixtures."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile

from playwright.sync_api import expect, sync_playwright

PROJECT = Path(__file__).resolve().parents[1]
BUILD = """
import {build} from 'vite';
await build({configFile: false, logLevel: 'error', build: {
  outDir: process.argv[2], lib: {entry: process.argv[1], name:'SidebarTest', formats:['iife'], fileName:()=>'sidebar.js'},
}});
"""


def check_capacity(page):
    missing_toggle = page.get_by_role("button", name="待重新挂载", exact=True)
    page.wait_for_function("document.querySelectorAll('.source-window').length === 40 && window.noteReads() === 1")
    assert missing_toggle.get_attribute("aria-expanded") == "false"
    assert page.locator('[data-source-state="missing"] .source-window').count() == 0
    section = page.locator(".source-sections").bounding_box()
    bottom = missing_toggle.bounding_box()
    assert 0 <= section["y"] + section["height"] - bottom["y"] - bottom["height"] <= 18
    missing_toggle.click()
    page.wait_for_function("document.querySelectorAll('.source-window').length === 80 && window.noteReads() === 2")
    before_sources = page.evaluate("window.sourceReads()")
    page.evaluate("window.editDocuments()")
    assert page.evaluate("window.sourceReads()") == before_sources
    assert page.evaluate("window.noteReads()") == 4
    assert page.locator(".source-window [data-note] strong").all_text_contents() == ["文档 0 v2", "文档 1 v2"]
    lanes = page.locator(".source-lane")
    for width, height in [(940, 640), (580, 400)]:
        page.set_viewport_size({"width": width, "height": height})
        live, missing = [lanes.nth(i).bounding_box() for i in (0, 1)]
        assert live["height"] > 45 and missing["height"] > 45
        assert live["y"] + live["height"] <= missing["y"] + 1
        assert missing["y"] + missing["height"] <= height + 1
        for lane in lanes.all():
            assert lane.locator(".source-lane-tree").evaluate("e=>e.clientHeight") >= int(lane.locator(".source-window > button").first.bounding_box()["height"]) - 1
            levels = [lane.locator(selector).first.bounding_box()["x"] for selector in (
                ".source-lane-heading .node-title", ".source-lane-tree > button .node-title",
                ".source-window .node-title", ".source-window [data-note] strong")]
            assert all(parent < child for parent, child in zip(levels, levels[1:])), levels
    page.evaluate("window.testSidebar.showSource('window/9999')")
    page.wait_for_selector('[data-source="window/9999"] [data-note="9999"]')
    print("10,000 window records. Initially 40 online rows and one list read. Opening offline makes 80 rows and two list reads. Both lanes visible at 940×640 and 580×400. Reveal reached window 9999.")
    page.evaluate("populate('/work/专题', 2000)")
    page.locator('[data-directory="/work"]').click()
    page.locator('[data-directory="/work/专题"]').click()
    rows = page.locator('[data-file^="/work/专题/"]')
    expect(rows).to_have_count(80)
    for count in range(160, 2001, 80):
        page.locator(".workspace-files").get_by_role("button", name="显示更多", exact=True).click()
        expect(rows).to_have_count(count)
    assert not page.evaluate("fileCalls.some(([cmd]) => cmd === 'open_library_file')")
    page.locator('[data-file="/work/专题/长文件名-1999.md"]').click()
    page.wait_for_function("fileCalls.some(([cmd,args]) => cmd === 'open_library_file' && args.path === '/work/专题/长文件名-1999.md')")
    print("2,000 file rows opened in batches of 80. The final file opened at the expected path.")


parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--capacity", action="store_true", help="Check large window and file lists")
options = parser.parse_args()
with tempfile.TemporaryDirectory(prefix="jot-source-tree-") as directory:
    work = Path(directory)
    entry = work / "entry.ts"
    entry.write_text(f"""
import {{ windowPicker }} from {json.dumps(str(PROJECT / "src/views/workspace/source-dialog.ts"))};
import {{ attachOnSwitch, documentDrag }} from {json.dumps(str(PROJECT / "src/views/workspace/document-drag.ts"))};
import {{ librarySidebar }} from {json.dumps(str(PROJECT / "src/views/workspace/library-sidebar.ts"))};
import {json.dumps(str(PROJECT / "src/styles/index.css"))};
import {json.dumps(str(PROJECT / "src/views/workspace/workspace.css"))};
let groups = Array.from({{length:{100 if options.capacity else 2}}}, (_, app) => ({{
  app:'应用 ' + app,
  windows: Array.from({{length:{100 if options.capacity else 2}}}, (_, i) => ({{key:'window/' + (app*100+i), title:'窗口 '+i, attached:i%2===0}}))
}}));
let reads = 0;
let sourceReads = 0;
let revision = 1;
window.fileCalls = [];
const tree = new Map([['/work', ['会议.md', '小组', '专题']], ['/work/小组', []], ['/work/专题', []]]);
window.workspaceRoots = ['/work'];
window.pickTestWindow = (title,action) => windowPicker()(title,action,async choice => {{ window.chosenWindow = choice; }});
window.switchTestWindow = id => {{ void attachOnSwitch(id); }};
window.permissionRequests = 0;
window.choiceReads = 0;
window.populate = (path, count = 85) => tree.get(path).push(...Array.from({{length:count}}, (_, i) => '长文件名-'+i+'.md'));
let draft = '归档草稿';
const createdArchive = new Map();
let openedFile;
const actions = [];
window.archiveActions = actions;
window.archiveReads = [];
window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {{ unregisterListener() {{}} }};
window.__TAURI_INTERNALS__ = {{ metadata:{{currentWindow:{{label:'main'}}}}, transformCallback: () => 1, async invoke(cmd, args) {{
  if(cmd==='plugin:event|listen') return 1;
  if(cmd==='plugin:event|unlisten' || cmd==='cancel_attachment') return;
  if(cmd==='attach_on_activation') {{ window.pendingAttach = args.id; return; }}
  if(cmd==='accessibility_trusted') return !window.permissionMissing;
  if(cmd==='running_app_path') return '/Users/qa/Applications/Jotiva.app';
  if(cmd==='reveal_file') {{ window.revealed=args.path; return; }}
  if(cmd==='plugin:window|is_focused') return window.nativeFocused;
  if(cmd==='request_accessibility') {{ window.permissionRequests++; return !window.permissionMissing; }}
  if(cmd==='finish_accessibility_request') {{ window.permissionFinished=(window.permissionFinished || 0)+1; return; }}
  if(cmd==='list_sources') {{ sourceReads++; return groups; }}
  if(cmd==='inbox_directory') return '/notes/其他';
  if(cmd==='archive_entries') {{
    archiveReads.push(args.folder);
    return [...(createdArchive.has(args.folder) ? [createdArchive.get(args.folder)] : []), ...(args.folder === null ? [] : [
      {{id:73,path:'/archive/'+draft+'.md',managed:true}},
      {{id:null,path:'/archive/导入前的材料.markdown',managed:false}},
    ])];
  }}
  if(cmd==='source_choices') {{
    window.choiceReads++;
    if(window.permissionMissing) return null;
    if(window.choiceError) throw Error('读取窗口列表失败');
    if(window.manyChoices) return Array.from({{length:300}}, (_,i)=>({{choice:i+1,app:'编辑器',title:'文件'+String(i).padStart(3,'0')}}));
    return [{{choice:5,app:'会议',title:'每周例会'}},{{choice:6,app:'会议',title:'设计评审'}},{{choice:7,app:'备忘录',title:''}}];
  }}
  if(cmd==='clear_source_choices') return;
  if(cmd==='list_folders') return [{{id:9, name:'会议归档', path:'/archive', description:''}}];
  if(cmd==='workspace_roots') return window.workspaceRoots;
  if(cmd==='workspace_children') return tree.get(args.path).map(name=>({{name, path:args.path+'/'+name, directory:tree.has(args.path+'/'+name)}}));
  if(cmd==='source_at_drop') return {{outside:false, choice:null}};
  if(cmd==='workspace_action') {{
    window.fileCalls.push([cmd,args]);
    const action = args.action;
    if(action.kind==='createFile') {{
      const children = tree.get(args.path);
      if (action.name && children.includes(action.name)) throw Error('文件已存在');
      let number = 1;
      let name = action.name ?? '未命名.md';
      while(children.includes(name)) name = '未命名 (' + (++number) + ').md';
      children.push(name);
      return {{path:args.path+'/'+name}};
    }}
    if(action.kind==='rename') {{
      const parent = args.path.slice(0,args.path.lastIndexOf('/')), old = args.path.split('/').pop();
      const name = action.name.toLowerCase().endsWith('.md') ? action.name : action.name+'.md';
      if(tree.get(parent).includes(name)) throw Error('文件已存在');
      tree.get(parent)[tree.get(parent).indexOf(old)] = name;
      return {{path:parent+'/'+name}};
    }}
    if(action.kind!=='move') throw Error('Unexpected action '+action.kind);
    const name = args.path.split('/').pop();
    const parent = args.path.slice(0,args.path.lastIndexOf('/'));
    const next = action.directory+'/'+name;
    tree.set(parent, tree.get(parent).filter(n=>n!==name));
    tree.get(action.directory).push(name);
    if(tree.has(args.path)) {{ tree.set(next, tree.get(args.path)); tree.delete(args.path); }}
    return {{path:next, removed:[]}};
  }}
  if(cmd==='file_selection_action') {{
    fileCalls.push([cmd,args]);
    for(const path of args.paths) {{
      const parent=path.slice(0,path.lastIndexOf('/')), name=path.split('/').pop();
      tree.set(parent,tree.get(parent).filter(n=>n!==name));
      if(args.action.kind==='move') tree.get(args.action.directory).push(name);
    }}
    return;
  }}
  if(cmd==='note_action') {{
    actions.push(args);
    if(args.action.kind==='rename') draft = args.action.name;
    return args.action.kind==='duplicate' ? 74 : args.id;
  }}
  if(cmd==='create_archive_note' || cmd==='new_note') {{
    window.fileCalls.push([cmd,args]);
    const id = args.folder === 9 ? 80 : 81;
    createdArchive.set(args.folder, {{id, path:(args.folder===9?'/archive/':'/notes/其他/')+'未命名.md', managed:true}});
    return id;
  }}
  if(cmd==='name_created_document') {{
    fileCalls.push([cmd,args]);
    if (args.id !== 42) {{
      const entry = [...createdArchive.values()].find(row => row.id === args.id);
      entry.path = entry.path.slice(0,entry.path.lastIndexOf('/')+1)+args.name;
      return null;
    }}
    const parent = openedFile.slice(0,openedFile.lastIndexOf('/'));
    const names = tree.get(parent);
    const index = names.indexOf(openedFile.split('/').pop());
    let actual = args.name;
    for(let n=2; names.includes(actual) && names[index]!==actual; n++) actual=args.name.replace(/[.]md$/, ' ('+n+').md');
    names[index]=actual;
    queueMicrotask(() => testSidebar.refreshFiles());
    return actual===args.name ? null : actual;
  }}
  if(cmd==='context_menu') {{
    window.menuLabels = args.labels;
    const index = args.labels.indexOf(window.pick);
    return index < 0 ? null : index;
  }}
  if(['open_library_file','open_document_window','copy_text'].includes(cmd)) {{
    window.fileCalls.push([cmd,args]);
    if(cmd==='open_library_file') openedFile = args.path;
    return cmd==='open_library_file' ? 42 : undefined;
  }}
  if(cmd==='list_notes') {{
    reads++;
    const id = Number(args.scope.key.split('/')[1]);
    return {{more:false, items:[{{id, title:'文档 ' + id + ' v' + revision, version:revision}}]}};
  }}
  throw Error(cmd);
}} }};
librarySidebar(id=>window.opened=id, ()=>{{}}, ()=>{{}}, (id,folder)=>window.movedArchive=[id,folder], async()=>{{}}).then(sidebar => {{
  documentDrag(sidebar.sidebar, async()=>{{}}, ()=>{{}}, async()=>{{}});
  document.querySelector('.workspace').prepend(sidebar.sidebar);
  window.testSidebar = sidebar; window.noteReads = () => reads; window.sourceReads = () => sourceReads;
  window.editDocuments = () => {{ revision++; return sidebar.refresh(false); }};
  window.untitledWindows = async n => {{
    groups = [{{app:'录音', windows:Array.from({{length:n}}, (_, i)=>({{
      key:'window/'+(20001+i), title:'未命名窗口', attached:true
    }}))}}];
    if(n===1) await sidebar.showSource('window/20001');
    else await sidebar.refresh();
  }};
}});
""")
    subprocess.run(["node", "--input-type=module", "-e", BUILD, str(entry), str(work / "dist")], check=True, cwd=PROJECT)
    with sync_playwright() as playwright:
        browser = playwright.webkit.launch(executable_path=os.getenv("WEBKIT_EXECUTABLE"))
        page = browser.new_page(viewport={"width": 940, "height": 640})
        page.route("https://sidebar.test/", lambda route: route.fulfill(body='<div id="app" class="main-view"><header class="workspace-titlebar"></header><div class="workspace"><main></main></div></div>', content_type="text/html"))
        page.goto("https://sidebar.test/")
        page.add_style_tag(path=next((work / "dist").glob("*.css")))
        page.add_script_tag(path=work / "dist/sidebar.js")
        if options.capacity:
            check_capacity(page)
            browser.close()
            raise SystemExit(0)
        page.evaluate("async () => { window.workspaceRoots=[]; await window.testSidebar.showWorkspace(); }")
        empty = page.get_by_role("button", name="添加工作区文件夹…", exact=True)
        for width in (580, 940):
            page.set_viewport_size({"width": width, "height": 640})
            assert abs(empty.bounding_box()["width"] - page.locator(".workspace-files").bounding_box()["width"]) < 1
        page.evaluate("async () => { window.workspaceRoots=['/work']; await window.testSidebar.showWorkspace(); }")
        page.locator('[data-directory="/work"]').click()
        row = page.locator('[data-file="/work/会议.md"]')
        for action, value in [("复制绝对路径", "/work/会议.md"), ("复制相对路径", "会议.md")]:
            page.evaluate("action => window.pick = action", action)
            row.click(button="right")
            page.wait_for_function("v => window.fileCalls.some(([cmd,args]) => cmd === 'copy_text' && args.text === v)", arg=value)
        page.evaluate("window.pick = '在新窗口打开'")
        row.click(button="right")
        page.wait_for_function("window.fileCalls.some(([cmd,args]) => cmd === 'open_document_window' && args.id === 42)")
        page.evaluate("window.pick = '移到废纸篓…'")
        row.click(button="right")
        dialog = page.get_by_role("dialog")
        dialog.get_by_role("button", name="取消").click()
        assert not page.evaluate("window.fileCalls.some(([cmd]) => cmd === 'workspace_action')")
        print("File menu dispatched both paths and opened the selected document ID. Cancelling Trash did not move the file.")
        assert row.evaluate("""async row => {
          await testSidebar.refreshFiles();
          return row === document.querySelector('[data-file="/work/会议.md"]');
        }"""), "Refocusing replaced the active drag source"
        page.set_viewport_size({"width": 940, "height": 640})
        row.drag_to(page.locator('[data-directory="/work/小组"]'))
        page.wait_for_selector('[data-file="/work/小组/会议.md"]')
        assert page.locator('[data-file="/work/会议.md"]').count() == 0
        page.locator('[data-directory="/work/小组"]').drag_to(page.locator('[data-directory="/work/专题"]'))
        page.wait_for_selector('[data-file="/work/专题/小组/会议.md"]')
        assert page.locator('[data-directory="/work/小组"]').count() == 0
        def begin_create(selector, repeat=False):
            page.wait_for_timeout(310)
            page.locator(selector).focus()
            if repeat:
                assert all(page.evaluate("Promise.all([testSidebar.createAtFocus(), testSidebar.createAtFocus()])"))
            else:
                assert page.evaluate("testSidebar.createAtFocus()")
            field = page.get_by_role("textbox", name="新文档名称")
            expect(field).to_be_focused()
            assert field.evaluate("el => el.selectionStart===0 && el.selectionEnd===el.value.lastIndexOf('.')")
            expect(page.locator("dialog[open]")).to_have_count(0)
            return field

        def create_at(selector, name=None):
            field = begin_create(selector)
            if name:
                field.fill(name)
            page.locator(selector).focus()
            expect(field).to_have_count(0)

        root_dir = '[data-directory="/work"]'
        before = page.evaluate("fileCalls.filter(([cmd,a])=>cmd==='workspace_action' && a.action.kind==='createFile').length")
        field = begin_create(root_dir, repeat=True)
        assert page.evaluate("fileCalls.filter(([cmd,a])=>cmd==='workspace_action' && a.action.kind==='createFile').length") == before + 1
        expect(page.locator('[data-file="/work/未命名.md"]')).to_have_count(1)
        field.fill("取消.md")
        field.press("Escape")
        expect(page.locator('[data-file="/work/未命名.md"]')).to_be_visible()
        create_at(root_dir, "重名.md")
        field = begin_create(root_dir)
        field.evaluate("""input => {
          input.dispatchEvent(new CompositionEvent('compositionstart', {bubbles:true}));
          input.value = '重名.md';
          input.dispatchEvent(new KeyboardEvent('keydown', {key:'Enter',isComposing:true,bubbles:true}));
          input.dispatchEvent(new CompositionEvent('compositionend', {bubbles:true}));
          input.dispatchEvent(new KeyboardEvent('keydown', {key:'Enter',bubbles:true}));
        }""")
        page.evaluate("testSidebar.refreshFiles()")
        expect(field).to_be_focused()
        page.locator(root_dir).focus()
        expect(page.locator('[data-file="/work/重名 (2).md"]')).to_be_visible()
        expect(page.get_by_role("status").filter(has_text="重名 (2).md")).to_be_visible()
        assert page.evaluate("fileCalls.filter(([cmd,a])=>cmd==='name_created_document' && a.name==='重名.md').length") == 2
        field = begin_create(root_dir)
        field.fill("失焦保留.md")
        field.dispatch_event("compositionstart")
        field.blur()
        expect(page.locator('[data-file="/work/失焦保留.md"]')).to_be_visible()
        nested = "/work/专题/小组"
        page.evaluate("path => window.populate(path)", nested)
        for title in ("未命名.md", "未命名 (2).md"):
            create_at(f'[data-directory="{nested}"]')
            file = nested + "/" + title
            page.wait_for_selector(f'[data-file="{file}"][aria-current=true]')
            page.wait_for_function("path => window.fileCalls.some(([cmd,args])=>cmd==='open_library_file' && args.path===path)", arg=file)
        assert page.locator(f'[data-file^="{nested}/"]').count() == 80
        first = page.locator(f'[data-file="{nested}/长文件名-0.md"]')
        second = page.locator(f'[data-file="{nested}/长文件名-1.md"]')
        third = page.locator(f'[data-file="{nested}/长文件名-2.md"]')
        first.click()
        opens = page.evaluate("fileCalls.filter(([cmd]) => cmd==='open_library_file').length")
        second.click(modifiers=["Meta"])
        third.click(modifiers=["Meta"])
        assert page.evaluate("fileCalls.filter(([cmd]) => cmd==='open_library_file').length") == opens
        assert page.locator('.workspace-files [data-file][aria-pressed=true]').count() == 3
        page.evaluate("window.pick = '复制绝对路径'")
        second.click(button="right")
        copied = page.evaluate("fileCalls.filter(([cmd])=>cmd==='copy_text').at(-1)[1].text.split('\\n')")
        assert len(copied) == 3 and copied[0].endswith("/长文件名-0.md")
        first.click()
        third.click(modifiers=["Shift"])
        assert page.locator('.workspace-files [data-file][aria-pressed=true]').count() == 3
        page.evaluate("window.pick = '移到废纸篓…'")
        third.click(button="right")
        page.get_by_role("dialog").get_by_role("button", name="取消").click()
        assert not page.evaluate("fileCalls.some(([cmd])=>cmd==='file_selection_action')")
        first.drag_to(page.locator('[data-directory="/work"]'))
        page.wait_for_selector('[data-file="/work/长文件名-2.md"]')
        assert page.evaluate("fileCalls.filter(([cmd])=>cmd==='file_selection_action').at(-1)[1].paths.length") == 3
        renamed = page.locator('[data-file="/work/长文件名-0.md"]')
        renamed.click()
        page.wait_for_timeout(550)
        renamed.click()
        name_input = page.get_by_role("textbox",name="重命名")
        name_input.fill("长文件名-1.md")
        name_input.press("Enter")
        page.wait_for_selector('.tree-name-field [aria-invalid=true]')
        name_input.fill("名单.md")
        name_input.press("Enter")
        page.wait_for_selector('[data-file="/work/名单.md"]')
        renamed = page.locator('[data-file="/work/名单.md"]')
        renamed.click()
        page.locator('[data-file="/work/长文件名-1.md"]').click(modifiers=["Meta"])
        page.evaluate("window.pick = '移到废纸篓…'")
        renamed.click(button="right")
        page.get_by_role("dialog").get_by_role("button", name="移到废纸篓").click()
        page.wait_for_function("fileCalls.filter(([cmd])=>cmd==='file_selection_action').at(-1)[1].action.kind === 'trash'")
        deletion = page.evaluate("fileCalls.filter(([cmd])=>cmd==='file_selection_action').at(-1)[1].paths")
        assert set(deletion) == {"/work/名单.md","/work/长文件名-1.md"}
        print("Modified clicks selected a range without opening editors. Bulk copy, cancel trash, drag move and retrying an inline rename all passed.")
        print("Empty-state row filled the panel at both widths. Directory creation targeted in root and nested directories; the active file remained visible beyond the first 80 entries.")
        page.get_by_role("button", name="归档", exact=True).click()
        create_at('[data-archive-folder=""]')
        page.wait_for_function("window.opened === 81")
        create_at('[data-archive-folder="9"]', '会议提纲.md')
        page.wait_for_function("window.opened === 80")
        assert page.locator('[data-archive-folder="9"]').get_attribute("aria-expanded") == "true"
        assert page.locator('[data-archive-folder=""]').get_attribute("aria-expanded") == "true"
        assert page.evaluate("window.fileCalls.some(([cmd,args])=>cmd==='create_archive_note' && args.folder===9 && args.name==='未命名')")
        assert page.locator("dialog[open]").count() == 0
        page.evaluate("async () => { archiveReads.length=0; await testSidebar.refresh(false); }")
        assert page.evaluate("archiveReads") == []
        page.evaluate("async () => { await testSidebar.refreshFiles(); }")
        assert page.evaluate("archiveReads") == [None, 9]
        imported = page.locator('[data-file="/archive/导入前的材料.markdown"]')
        assert imported.is_visible()
        assert not page.evaluate("fileCalls.some(([cmd,args])=>cmd==='open_library_file' && args.path.startsWith('/archive/'))")
        imported.click()
        assert page.evaluate("fileCalls.some(([cmd,args])=>cmd==='open_library_file' && args.path==='/archive/导入前的材料.markdown')")
        archived = page.locator('.archive-tree [data-note="73"]')
        page.evaluate("window.pick = '重命名…'")
        archived.click(button="right")
        name_input = page.get_by_role("textbox", name="重命名")
        name_input.fill("会议结论")
        name_input.press("Enter")
        page.wait_for_function("window.archiveActions.some(a=>a.id===73 && a.action.name==='会议结论')")
        page.evaluate("window.testSidebar.refreshFiles()")
        page.wait_for_selector('.archive-tree [data-note="73"][title="会议结论.md"]')
        page.evaluate("window.pick = '制作副本'")
        archived.click(button="right")
        page.wait_for_function("window.opened===74")
        page.evaluate("window.pick = '移到归档目录…'")
        archived.click(button="right")
        page.wait_for_function("window.movedArchive?.[0]===73 && window.movedArchive[1]===9")
        print("WebKit pointer drags moved a file into a folder, then moved its expanded parent and retained the nested file. Archive rename, copy and move targeted the selected record.")
        page.evaluate("window.untitledWindows(1)")
        first = page.locator('[data-source="window/20001"]')
        expect(first.locator(":scope > button .node-title")).to_have_text("录音")
        page.wait_for_selector('[data-source="window/20001"] [data-note="20001"]')
        page.evaluate("window.testSidebar.setActive(20001)")
        page.evaluate("window.untitledWindows(2)")
        expect(first.locator(":scope > button .node-title")).to_have_text("窗口 1")
        expect(page.locator('[data-source="window/20002"] > button .node-title')).to_have_text("窗口 2")
        assert first.locator('[data-note="20001"][aria-current=true]').count() == 1
        assert page.locator('[data-source="window/20002"] [data-note="20001"]').count() == 0
        print("A second untitled window kept the first document under its original key.")
        page.evaluate("pickTestWindow('添加来源窗口','登记')")
        picker = page.get_by_role("dialog",name="添加来源窗口")
        search = picker.get_by_role("combobox")
        expect(search).to_be_focused()
        options = picker.get_by_role("option")
        page.wait_for_function("document.querySelectorAll('.window-picker-option').length === 3")
        search.fill("无匹配的文稿")
        assert options.count() == 0 and picker.get_by_text("没有匹配的窗口").is_visible()
        search.fill("会议")
        assert options.count() == 2
        second = int(options.nth(1).get_attribute("data-choice"))
        search.press("ArrowDown")
        search.press("Enter")
        page.wait_for_function("id=>window.chosenWindow===id",arg=second)
        page.evaluate("pickTestWindow('将会议纪要挂载到窗口','挂载')")
        picker = page.get_by_role("dialog",name="将会议纪要挂载到窗口")
        picker.get_by_role("combobox").fill("设计评审")
        picker.get_by_role("option").click()
        page.wait_for_function("window.chosenWindow===6")
        reads = page.evaluate("window.choiceReads")
        page.evaluate("window.permissionMissing=true; pickTestWindow('添加来源窗口','登记')")
        picker = page.get_by_role("dialog",name="添加来源窗口")
        allow = picker.get_by_role("button",name="打开辅助功能设置…")
        expect(allow).to_be_focused()
        expect(picker.get_by_role("combobox")).to_have_count(0)
        expect(picker.get_by_role("button",name="刷新",exact=True)).to_have_count(0)
        picker.get_by_text("已打开开关，仍提示未授权？").click()
        expect(picker.get_by_text("当前运行位置：/Users/qa/Applications/Jotiva.app")).to_be_visible()
        picker.get_by_role("button", name="在 Finder 中显示这份 Jotiva").click()
        assert page.evaluate("window.revealed") == "/Users/qa/Applications/Jotiva.app"
        allow.click()
        assert page.evaluate("window.permissionRequests===1")
        page.evaluate("window.dispatchEvent(new Event('focus'))")
        expect(allow).to_be_visible()
        assert page.evaluate("window.choiceReads") == reads
        picker.get_by_role("button",name="取消").click()
        page.evaluate("window.permissionMissing=false; window.dispatchEvent(new Event('focus'))")
        expect(picker).not_to_be_visible()
        page.wait_for_function("window.permissionFinished===1")
        assert page.evaluate("window.choiceReads") == reads
        assert page.evaluate("window.chosenWindow===6")

        page.evaluate("window.permissionMissing=true; pickTestWindow('将会议纪要挂载到窗口','挂载')")
        picker = page.get_by_role("dialog",name="将会议纪要挂载到窗口")
        picker.get_by_role("button",name="打开辅助功能设置…").click()
        # A non-activating panel can return without any DOM focus event. While
        # in System Settings, detect the grant but do not start choosing a window yet.
        page.evaluate("document.hasFocus = () => false; window.nativeFocused=false; window.permissionMissing=false")
        expect(picker.get_by_role("status")).to_have_text("已授权，切回 Jotiva 即可继续。")
        expect(picker.get_by_role("combobox")).to_have_count(0)
        page.evaluate("window.nativeFocused=true")
        expect(picker.get_by_role("combobox")).to_be_focused()
        page.evaluate("delete document.hasFocus")
        # Revocation after opening the picker leads back to the same pending action.
        page.evaluate("window.permissionMissing=true")
        picker.get_by_role("button",name="刷新",exact=True).click()
        expect(picker.get_by_role("button",name="打开辅助功能设置…")).to_be_visible()
        page.evaluate("window.permissionMissing=false; window.dispatchEvent(new Event('focus'))")
        picker.get_by_role("option").filter(has_text="每周例会").click()
        page.wait_for_function("window.chosenWindow===5")

        page.evaluate("window.choiceError=true; pickTestWindow('添加来源窗口','登记')")
        picker = page.get_by_role("dialog",name="添加来源窗口")
        expect(picker.get_by_role("status")).to_contain_text("读取窗口列表失败")
        expect(picker.get_by_role("button",name="打开辅助功能设置…")).to_have_count(0)
        page.evaluate("window.choiceError=false")
        picker.get_by_role("button",name="刷新",exact=True).click()
        expect(picker.get_by_role("option")).to_have_count(3)
        picker.get_by_role("button",name="取消").click()

        page.evaluate("window.permissionMissing=true; switchTestWindow(37)")
        switcher = page.get_by_role("dialog",name="切换应用挂载")
        switcher.get_by_role("button",name="打开辅助功能设置…").click()
        assert page.evaluate("window.pendingAttach===undefined")
        page.evaluate("window.permissionMissing=false; window.dispatchEvent(new Event('focus'))")
        page.wait_for_function("window.pendingAttach===37")
        expect(switcher.get_by_role("heading")).to_have_text("切换到要挂载的窗口")
        switcher.get_by_role("button",name="取消").click()
        page.evaluate("window.manyChoices=true; pickTestWindow('添加来源窗口','登记')")
        picker = page.get_by_role("dialog",name="添加来源窗口")
        page.wait_for_function("document.querySelectorAll('.window-picker-option').length===60")
        picker.get_by_role("button",name="显示更多").click()
        assert picker.get_by_role("option").count() == 120
        picker.get_by_role("combobox").fill("文件299")
        picker.get_by_role("combobox").press("Enter")
        page.wait_for_function("window.chosenWindow===300")
        print("Window access handled denial, cancellation, later grant and revocation. Both window selection and application switching resumed the original action. Window-list errors retried independently.")
        browser.close()
