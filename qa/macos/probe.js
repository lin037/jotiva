const api = window.__TAURI_INTERNALS__;
const label = api.metadata.currentWindow.label;
const sampleCount = Number(new URL(import.meta.url).searchParams.get("samples"));
const run = (command, args) => api.invoke(command, args);
const pause = ms => new Promise(resolve => setTimeout(resolve, ms));

async function record(kind, data) {
  const id = await run("new_note");
  await run("save_note", { id, version: 0, markdown: "#probe:" + kind + "\n" + JSON.stringify(data) });
}

async function waitFor(probe, description) {
  for (let tries = 0; tries < 160; tries += 1) {
    const result = await probe();
    if (result) return result;
    await pause(35);
  }
  throw new Error("Timed out waiting for " + (description || probe.name || probe.toString()));
}

function press(target, key, code, metaKey = false) {
  return target.dispatchEvent(new KeyboardEvent("keydown", {
    key, code, metaKey, bubbles: true, cancelable: true,
  }));
}

function focusedEditor() {
  const target = document.activeElement;
  return target?.matches(".cm-content[contenteditable=true]") &&
    target.closest("[data-jot-editor-ready]") ? target : undefined;
}

async function verifyNewlineOrigin() {
  const content = focusedEditor();
  const paragraph = content.querySelector(".jot-live-preview-native-block--paragraph");
  const text = document.createRange();
  text.selectNodeContents(paragraph);
  const start = text.getBoundingClientRect().left;
  const positions = [];
  const measure = () => {
    const br = content.querySelector(":scope > .cm-line:last-child > br:only-child");
    if (br) positions.push(br.getBoundingClientRect().left);
  };
  const observer = new MutationObserver(measure);
  observer.observe(content, { subtree: true, childList: true, attributes: true });
  try {
    press(content, "Enter", "Enter");
    await waitFor(() => content.querySelector(":scope > .jot-live-preview-blank-line:last-child"));
    measure();
    if (positions.length < 2 || positions.some(left => Math.abs(left - start) > .5)) {
      throw new Error(`Newline moved from the paragraph origin ${start}: ${positions}`);
    }
  } finally {
    observer.disconnect();
  }
}

async function verifyOutline() {
  const toggle = await waitFor(() => document.querySelector('button[aria-controls="document-outline"]'));
  if (!toggle.disabled || document.querySelector(".jot-outline-host")) throw new Error("Outline should start closed");
  document.querySelector('button[aria-label="新建文档"]').click();
  const name = await waitFor(() => document.querySelector('input[aria-label="新文档名称"]'));
  name.value = "测试大纲.md";
  press(name, "Enter", "Enter");
  await waitFor(focusedEditor, "main editor ready");
  const headings = Array.from({ length: 72 }, (_, index) => "## 记录 " + (index + 1));
  const source = ["# 开场", "## 计划", "# 正文", ...headings, "# 结束"].join("\n\n");
  document.execCommand("insertText", false, source);
  toggle.click();
  const pane = document.getElementById("document-outline");
  const nav = await waitFor(() => pane.querySelector(".jot-outline"));
  await record("outline-rendered", {});
  await waitFor(() => nav.getAttribute("aria-busy") === "false", "outline analysis complete");
  const row = title => [...pane.querySelectorAll(".jot-outline__row")].find(button => button.textContent.trim() === title);
  const first = await waitFor(() => row("开场"), "outline headings");
  await waitFor(() => row("计划"));
  if (pane.hidden || first.getAttribute("aria-level") !== "1") throw new Error("Outline did not render the heading hierarchy");
  const collapse = first.querySelector(".jot-outline__toggle");
  collapse.click();
  await waitFor(() => first.getAttribute("aria-expanded") === "false" && !row("计划"), "folded outline branch");
  collapse.click();
  const child = await waitFor(() => row("计划"), "expanded outline branch");
  child.click();
  await waitFor(() => child.hasAttribute("aria-current") && focusedEditor(), "outline click returned to the source heading");
  child.focus({ preventScroll: true });
  await pause(50);
  if (document.activeElement !== child) throw new Error("Could not focus the outline row for keyboard navigation");
  const handled = !press(child, "End", "End");
  const requestedTop = nav.scrollTop;
  const last = await waitFor(() => row("结束") === document.activeElement && row("结束"), "keyboard reached the end of a long outline")
    .catch(error => {
      const rows = pane.querySelectorAll(".jot-outline__row");
      throw new Error(`${error} (handled ${handled}, scroll ${requestedTop} → ${nav.scrollTop}/${nav.scrollHeight}, viewport ${nav.clientHeight}, ${rows.length} rows, last ${rows[rows.length - 1]?.textContent}, focus ${document.activeElement?.className})`);
    });
  if (pane.querySelectorAll(".jot-outline__row").length >= headings.length) throw new Error("Outline rendered every heading");
  last.click();
  await waitFor(() => last.hasAttribute("aria-current") && focusedEditor(), "end heading navigation");
  const thumb = pane.querySelector(".jot-direct-scrollbar__thumb");
  if (!thumb || !thumb.checkVisibility() || getComputedStyle(thumb).borderTopColor !== "rgba(0, 0, 0, 0)") {
    throw new Error("Long outline lost its themed scrollbar");
  }
  const sidebar = document.getElementById("workspace-library");
  const divider = sidebar.querySelector('[role="separator"]');
  const original = sidebar.offsetWidth;
  // Synthetic PointerEvents have no native pointer to capture in WKWebView.
  divider.setPointerCapture = () => {};
  const pointer = (type, clientX) => divider.dispatchEvent(new PointerEvent(type, {
    pointerId: 1, isPrimary: true, button: 0, clientX, bubbles: true, cancelable: true,
  }));
  pointer("pointerdown", 100);
  pointer("pointermove", 134);
  await waitFor(() => sidebar.offsetWidth === original + 34, "visible drag width");
  if ((await run("read_appearance")).workspace.sidebar !== null) throw new Error("Drag wrote before pointer up");
  pointer("pointerup", 134);
  await waitFor(async () => (await run("read_appearance")).workspace.sidebar === original + 34, "saved sidebar width");
  const outlineWidth = pane.offsetWidth;
  press(pane.querySelector('[role="separator"]'), "ArrowLeft", "ArrowLeft");
  await waitFor(async () => (await run("read_appearance")).workspace.outline === outlineWidth + 16, "saved outline width");

  await run("plugin:window|set_size", { label, value: { Logical: { width: 580, height: 410 } } });
  await waitFor(() => innerWidth === 580, "narrow window");
  if (getComputedStyle(pane).position !== "absolute") throw new Error("The narrow outline should overlay the document");
  const documentPane = document.querySelector(".document-pane");
  const narrowWidth = documentPane.getBoundingClientRect().width;
  toggle.click();
  await waitFor(() => pane.hidden && !pane.querySelector(".jot-outline-host"), "outline disposed on close");
  if (Math.abs(documentPane.getBoundingClientRect().width - narrowWidth) > 1) throw new Error("The outline changed the narrow editing width");
  if (document.scrollingElement.scrollHeight > innerHeight + 1) throw new Error("The window shell overflowed vertically");
}

async function workspaceFileRow(name = "meeting.md") {
  const tab = document.querySelector(".library-tabs button");
  if (tab.getAttribute("aria-pressed") !== "true") tab.click();
  const roots = await run("workspace_roots");
  const root = await waitFor(() => [...document.querySelectorAll(".workspace-files [data-directory]")]
    .find(row => row.dataset.directory === roots[0]));
  if (root.getAttribute("aria-expanded") !== "true") root.click();
  return waitFor(() => [...document.querySelectorAll("[data-file]")].find(row => row.dataset.file.endsWith("/" + name)));
}

async function verifyListGeometry() {
  const root = document.querySelector(".editor-root");
  const open = async markdown => {
    const id = await run("new_note");
    await run("save_note", { id, version: 0, markdown });
    await run("show_main", { id });
  };
  const textStart = label => {
    const line = [...root.querySelectorAll(".cm-line[data-jot-live-list-marker]")].find(row => row.textContent.trim() === label);
    if (!line) return undefined;
    const walker = document.createTreeWalker(line, NodeFilter.SHOW_TEXT);
    let node;
    while ((node = walker.nextNode())) {
      const index = node.data.indexOf(label);
      if (index < 0) continue;
      const range = document.createRange();
      range.setStart(node, index);
      range.setEnd(node, index + 1);
      return range.getBoundingClientRect().left;
    }
  };
  await open("- 普通\n- [ ] 待办\n  - 子项\n  - [x] 子任务\n\n9. 单位数\n10. 两位数\n123456789. [ ] 长编号");
  await waitFor(() => textStart("长编号") !== undefined, "ordered task geometry");
  const sameColumn = labels => Math.max(...labels.map(textStart)) - Math.min(...labels.map(textStart)) <= 1;
  if (!sameColumn(["普通", "待办", "单位数", "两位数", "长编号"]) ||
      !sameColumn(["子项", "子任务"]) || textStart("子项") <= textStart("普通")) {
    throw Error("List body columns do not match their indentation");
  }
  const wide = parseFloat(getComputedStyle(root.querySelector(".jot-live-preview-content")).getPropertyValue("--jot-list-indent"));
  await open("- 短列表");
  await waitFor(() => textStart("短列表") !== undefined, "reused editor root");
  const fresh = parseFloat(getComputedStyle(root.querySelector(".jot-live-preview-content")).getPropertyValue("--jot-list-indent"));
  if (fresh >= wide || root.style.getPropertyValue("--jot-ordered-task-width")) throw Error("Retained previous document's marker width");
}

async function verifyDiskEditing() {
  const row = await workspaceFileRow();
  row.click();
  await waitFor(() => focusedEditor()?.textContent.includes("original-file"), "workspace file in editor");
  document.execCommand("insertText", false, "edited-in-Jot ");
  const current = await waitFor(async () => {
    const { items } = await run("list_notes", { query: "edited-in-Jot", offset: 0 });
    return items.find(card => card.file === row.dataset.file);
  });
  await record("disk-saved", current);
  await waitFor(() => focusedEditor()?.textContent.includes("changed-by-another-editor"), "Open Documents reloaded active Markdown");
  await waitFor(async () => (await run("list_notes", { query: "close-saved", offset: 0 })).items
    .some(card => card.file?.endsWith("/finder-second.md")), "second Finder window flushed on close");
  await record("disk-reloaded", {});
  await verifyConflictCopy();
}

async function verifyConflictCopy() {
  const [root] = await run("workspace_roots");
  await waitFor(async () => (await run("workspace_children", { path: root })).some(child => child.name === "changed.md"));
  document.execCommand("insertText", false, "keep-this-local-edit ");
  const recover = await waitFor(() => [...document.querySelectorAll('button')].find(node => node.textContent === "另存编辑并重载"));
  recover.click();
  await waitFor(() => focusedEditor()?.textContent.includes("preserve-this-disk-version"), "conflict restored disk version");
  const copies = await run("list_notes", { query: "keep-this-local-edit", offset: 0, scope: {kind: "folder", id: null} });
  if (!copies.items.length) throw Error("Local edits did not survive as a separate note");
  return copies.items[0].id;
}

async function verifyMissingFileRecovery() {
  const row = await workspaceFileRow("removed.md");
  row.click();
  await waitFor(() => focusedEditor()?.textContent.includes("document-on-removable-volume"));
  await record("unlink-document", {});
  const [root] = await run("workspace_roots");
  await waitFor(async () => !(await run("workspace_children", { path: root })).some(child => child.name === "removed.md"));
  document.execCommand("insertText", false, "local-before-disconnect ");
  const recover = await waitFor(() => [...document.querySelectorAll("button")].find(node => node.textContent === "另存编辑并重载"));
  recover.click();
  recover.click();
  await waitFor(() => focusedEditor() && document.querySelector(".workspace-footer")?.textContent.includes("原路径不可用"));
  document.execCommand("insertText", false, "continuing-in-the-copy ");
  const saved = await waitFor(async () => (await run("list_notes", { query: "continuing-in-the-copy", offset: 0, scope: {kind: "folder", id: null} })).items[0]);
  const copies = (await run("list_notes", { query: "local-before-disconnect", offset: 0, scope: {kind: "folder", id: null} })).items;
  if (copies.length !== 1 || copies[0].id !== saved.id || (await run("read_note", {id: saved.id})).file) {
    throw Error("Deleted-file recovery did not continue in a single independent copy");
  }
}

async function verifyFolderCreation() {
  const [root] = await run("workspace_roots");
  const path = root.slice(0, root.lastIndexOf("/")) + "/会议归档";
  const folder = await run("save_folder", { input: { id: null, name: "会议归档", description: "", path, classify: false } });
  async function writeFromRow(selector, text) {
    const before = focusedEditor();
    const row = await waitFor(() => document.querySelector(selector));
    row.focus();
    press(row, "n", "KeyN", true);
    const input = await waitFor(() => document.activeElement?.getAttribute("aria-label") === "新文档名称" && document.activeElement);
    input.value = "已命名行内文档.md";
    press(input, "Enter", "Enter");
    await waitFor(() => focusedEditor() && focusedEditor() !== before && document.querySelector(".workspace-count").textContent === "0 字符", "new document ready for typing");
    if (document.querySelector("dialog[open]")) throw Error("New document asked for a name or directory");
    document.execCommand("insertText", false, text);
    await waitFor(() => document.querySelector(".workspace-count").textContent === "4 字符", "Unicode character count");
    const card = await waitFor(async () => (await run("list_notes", { query: text, offset: 0 })).items[0]);
    return run("read_note", { id: card.id });
  }
  const workspace = await writeFromRow('[data-directory][data-root="true"]', "目录 A📌");
  document.querySelectorAll(".library-tabs button")[1].click();
  const archive = await writeFromRow('[data-archive-folder="' + folder.id + '"]', "归档 B📎");
  const existing = await waitFor(() => document.querySelector('.archive-tree [data-file$="/已有议程.md"]'), "previously unopened imported Markdown");
  existing.click();
  await waitFor(() => focusedEditor()?.textContent.includes("已有的资料保持在原路径"), "imported Markdown read from disk");
  const draft = await writeFromRow('[data-archive-folder=""]', "草稿 C📃");
  if (archive.location?.folder !== folder.id || draft.location?.folder !== null || draft.file) throw Error("Unexpected new document location");
  const blankId = await run("new_note");
  const blankVersion = await run("save_note", { id: blankId, version: 0, markdown: "\n" });
  const blank = await run("read_card", { id: blankId });
  const rowForBlank = () => document.querySelector('.archive-tree [data-note="' + blankId + '"]');
  await waitFor(() => rowForBlank()?.dataset.file === blank.archivePath, "first save appears in inbox");
  await run("save_note", { id: blankId, version: blankVersion, markdown: "# 次日议程" });
  await waitFor(() => rowForBlank()?.dataset.file.endsWith("/次日议程.md"), "first title updates the visible path");
  const restored = await run("read_card", { id: 41 });
  await record("folder-create", { file: workspace.file, archive: archive.location.path, existing: existing.dataset.file,
    inbox: draft.location.path, migrated: restored.archivePath });
  document.querySelectorAll(".library-tabs button")[0].click();
}

function drag(source, target, text) {
  const dataTransfer = new DataTransfer();
  const event = (name, node) => node.dispatchEvent(new DragEvent(name, { dataTransfer, bubbles: true, cancelable: true }));
  if (source) event("dragstart", source);
  else dataTransfer.setData("text/plain", text);
  if (event("dragover", target)) throw Error("Source window rejected the drag");
  if (event("drop", target)) throw Error("No drop handler accepted the document");
  if (source) event("dragend", source);
}

async function verifyDrops(id, targetTitle) {
  const add = await waitFor(() => document.querySelector('button[aria-label="添加来源窗口"]'));
  add.click();
  const dialog = await waitFor(() => document.querySelector('#window-picker-heading')?.closest('dialog[open]'));
  const search = dialog.querySelector('[role="combobox"]');
  search.value = targetTitle;
  search.dispatchEvent(new Event("input", { bubbles: true }));
  const choice = await waitFor(() => [...dialog.querySelectorAll("[data-choice]")].find(row => row.firstElementChild.textContent === targetTitle),
    "fixture window in filtered source choices");
  choice.click();
  const target = () => [...document.querySelectorAll('[data-source-state="live"] [data-source]')].find(row => row.title === targetTitle || row.querySelector("button")?.title === targetTitle);
  await waitFor(() => !dialog.open && target());
  const row = await workspaceFileRow();
  drag(row, target());
  const file = await waitFor(async () => (await run("list_notes", { query: "meeting.md", offset: 0 })).items
    .find(card => card.file === row.dataset.file && card.attached));
  drag(null, target(), "plain text dropped onto the meeting");
  await waitFor(async () => (await run("list_notes", { query: "plain text dropped onto the meeting", offset: 0 })).items.some(card => card.attached));
  await run("show_main", { id: file.id });
  await waitFor(() => focusedEditor()?.textContent.includes("preserve-this-disk-version"));
  document.querySelector('button[aria-controls="document-windows"]').click();
  (await waitFor(() => [...document.querySelectorAll('#document-windows:popover-open button')]
    .find(button => button.textContent === "作为独立便签打开"))).click();
  await waitFor(async () => (await run("list_notes", { query: "#probe:disk-recalled", offset: 0 })).items.length);
  await waitFor(async () => !(await run("read_card", { id: file.id })).attached);
  if ((await run("read_note", { id: file.id })).file !== row.dataset.file) throw Error("Recall changed the path of the original file");
  await waitFor(() => focusedEditor()?.textContent.includes("preserve-this-disk-version"));
  const dragEvents = [];
  let downTarget;
  const observe = event => {
    const file = event.target.closest?.("[data-file]");
    if (event.type === "mousedown") downTarget = event.target;
    if (dragEvents.length < 60) dragEvents.push([event.type, file?.dataset.file?.split("/").pop() ?? event.target.className,
      event.clientX, event.clientY, event.buttons, downTarget?.isConnected, event.shiftKey, event.ctrlKey, event.detail]);
  };
  const events = new AbortController();
  for (const kind of ["mousedown", "mousemove", "mouseup", "dragstart", "dragend", "pointercancel"]) {
    document.addEventListener(kind, observe, { capture: true, signal: events.signal });
  }
  const outgoing = await workspaceFileRow();
  outgoing.scrollIntoView({ block: "nearest" });
  const rect = outgoing.getBoundingClientRect();
  const position = await run("plugin:window|inner_position", { label });
  const scale = await run("plugin:window|scale_factor", { label });
  await record("drag-screen", {
    x: position.x / scale + rect.x + rect.width / 2, y: position.y / scale + rect.y + rect.height / 2,
    target: targetTitle.slice(-1),
  });
  const secondRecall = await waitFor(async () => {
    const { items } = await run("list_notes", { query: "#probe:disk-recalled", offset: 0 });
    return items.length === 2 && items[0];
  }, "real mouse drag created quick editor over external window").catch(error => {
    throw Error(error.message + " · " + JSON.stringify({ dragEvents, focused: document.hasFocus(), same: downTarget === outgoing,
      draggable: downTarget?.draggable, dragStyle: downTarget && getComputedStyle(downTarget).webkitUserDrag, active: document.activeElement?.className,
      hit: document.elementFromPoint(rect.x + rect.width / 2, rect.y + rect.height / 2)?.outerHTML.slice(0, 220) }));
  });
  events.abort();
  const floated = JSON.parse((await run("read_note", { id: secondRecall.id })).markdown.split("\n")[1]);
  if (floated.id !== file.id || !floated.source.endsWith(targetTitle)) throw Error("Mouse drag selected another note or host");
  const switchRow = await workspaceFileRow();
  switchRow.scrollIntoView({ block: "nearest" });
  const after = switchRow.getBoundingClientRect();
  await record("switch-drag", { x: position.x / scale + after.x + after.width / 2, y: position.y / scale + after.y + after.height / 2,
    target: targetTitle.slice(-1) });
  const switched = await waitFor(async () => {
    const { items } = await run("list_notes", { query: "#probe:disk-recalled", offset: 0 });
    return items.length === 3 && items[0];
  }, "switching the active application while dragging followed its key window");
  const receipt = JSON.parse((await run("read_note", { id: switched.id })).markdown.split("\n")[1]);
  if (receipt.id !== file.id || !receipt.source.endsWith(targetTitle)) throw Error("Cmd+Tab drag attached to the wrong window");
  await run("show_main", { id });
  await waitFor(() => focusedEditor()?.textContent.includes("latency-probe"));
}

async function verifyReattach(id) {
  const note = await run("read_note", { id });
  const target = note.source.endsWith("Fixture A") ? "B" : "A";
  await verifyDrops(id, "Fixture " + target);
  const otherApp = (await run("source_choices")).find(candidate => candidate.title === "Focus-B");
  if (!otherApp) throw Error("The second AppKit process did not offer its document");
  const remote = await run("new_note");
  await run("save_note", { id: remote, version: 0, markdown: "other-host-search-target" });
  await run("attach_note", { id: remote, choice: otherApp.choice });
  await run("clear_source_choices");
  document.querySelector('button[aria-controls="document-windows"]').click();
  const button = await waitFor(() => document.querySelector('#document-windows:popover-open button[aria-label="更换宿主窗口"]'));
  button.click();
  const dialog = await waitFor(() => document.querySelector('dialog[aria-labelledby="window-picker-heading"][open]'));
  const search = dialog.querySelector('[role="combobox"]');
  search.value = "Fixture " + target;
  search.dispatchEvent(new Event("input", { bubbles: true }));
  await waitFor(() => dialog.querySelector('[role="option"][aria-selected="true"]')?.firstElementChild.textContent === search.value);
  press(search, "Enter", "Enter");
  await waitFor(() => !dialog.open, "attachment dialog submitted");
  const attached = await run("read_note", { id });
  if (!attached.source.endsWith("Fixture " + target) || attached.markdown !== note.markdown || attached.version !== note.version) {
    throw new Error("Reattaching changed the note or left it on the old window");
  }
  const { items } = await run("list_notes", { query: "plain text dropped onto the meeting", offset: 0 });
  for (const card of items) if (card.attached) await run("detach_note", { id: card.id });
  await record("reattached", { id, target, fileAndTextDropped: true, nativeDrag: true });
  await run("locate", { id });
}

if (label.startsWith("document-")) {
  try {
    const id = Number(label.slice("document-".length));
    const card = await run("read_card", { id });
    if (card.file?.endsWith("/finder-second.md")) {
      await waitFor(() => focusedEditor()?.textContent.includes("second-file-from-Finder"), "second Finder window");
      const { windows } = await run("read_settings");
      await run("open_document_window", { id });
      if ((await run("read_settings")).windows.webviews !== windows.webviews) throw Error("Duplicate document window");
      const pin = document.querySelector('[aria-label="置顶文档窗口"]');
      pin.click();
      await waitFor(() => pin.getAttribute("aria-pressed") === "true", "native document pin");
      const editor = document.querySelector(".jot-editor");
      const steps = [];
      const modes = new MutationObserver(records => steps.push(...records.map(record => record.oldValue)));
      modes.observe(editor, { attributes:true, attributeOldValue:true, attributeFilter:["data-jot-editor-mode"] });
      await record("document-mode-ready", {});
      for (const mode of ["source", "live-preview"]) {
        await waitFor(() => editor.dataset.jotEditorMode === mode, "native ⌘/ " + mode);
        await pause(90);
        if (editor.dataset.jotEditorMode !== mode) throw Error("One ⌘/ advanced more than one mode");
        await record("document-mode-" + mode, {});
      }
      modes.disconnect();
      if (steps.join() !== "live-preview,source") throw Error("Unexpected mode transitions: " + steps);
      await waitFor(() => focusedEditor(), "editable document after mode cycle");
      document.execCommand("insertText", false, "close-saved ");
      await run("plugin:window|close", {label});
    }
  } catch (error) {
    await record("error", { error: String(error) });
  }
}

async function verifyStorage() {
  await waitFor(() => document.querySelector(".library-tabs button"));
  const [root] = await run("workspace_roots");
  const legacy = await run("list_notes", {query:"旧版保留笔记", offset:0, scope:{kind:"folder", id:null}});
  if (legacy.items.length !== 1 || legacy.items[0].id !== 42 || legacy.items[0].version !== 5) {
    throw new Error("The archived note saved before renaming did not load");
  }
  const previous = (await run("list_notes", {query:"#probe:storage-saved", offset:0})).items[0];
  if (previous) {
    (await workspaceFileRow("images.md")).click();
    await waitFor(() => [...document.querySelectorAll(".jot-editor img")].some(img => img.naturalWidth === 3), "reopened native image");
    const marker = await run("read_note", {id: previous.id});
    const {missing, ...ids} = JSON.parse(marker.markdown.split("\n")[1]);
    const saved = {};
    for (const [name, id] of Object.entries(ids)) {
      const note = await run("read_note", {id});
      saved[name] = {path: note.file ?? note.location.path, markdown: note.markdown};
    }
    await run("show_main", {id:missing});
    await waitFor(() => focusedEditor()?.textContent.includes("offline snapshot"));
    const recover = await waitFor(() => [...document.querySelectorAll("button")]
      .find(button => button.textContent === "另存编辑并重载"));
    recover.click();
    const restored = await waitFor(async () => (await run("list_notes", {
      query:"offline snapshot", offset:0, scope:{kind:"folder", id:null}
    })).items.find(item => item.id !== missing));
    const offline = await run("read_note", {id:restored.id});
    saved.offline = {path:offline.location.path, markdown:offline.markdown};
    await record("storage-reopened", saved);
    return;
  }
  const blog = root + "/blog";
  const {revision, folder: imageFolder} = await run("read_image_settings", {folder:blog});
  if (imageFolder.effectiveRule.path !== root || imageFolder.effectiveRule.mode !== "relative") {
    throw Error("The folder's current image rule did not match its parent");
  }
  await run("save_image_folder", {
    folder:blog, revision, rule:{path:blog, mode:"fixed", directory:"${ruleDir}/../access"},
  });
  const monitor = await run("plugin:window|current_monitor", {label});
  const windowPosition = {x:monitor.workArea.position.x + 40, y:monitor.workArea.position.y + 40};
  await run("plugin:window|set_position", {label, value:{Physical:windowPosition}});
  await waitFor(async () => {
    const origin = await run("plugin:window|outer_position", {label});
    return Math.abs(origin.x - windowPosition.x) <= 2 && Math.abs(origin.y - windowPosition.y) <= 2;
  }, "workspace moved off the center of the display");
  const mainSize = await run("plugin:window|outer_size", {label});
  await run("show_settings", {section:"images"});
  const settingsMarker = await waitFor(async () => (await run("list_notes", {query:"#probe:settings-opened", offset:0})).items[0],
    "settings window opened");
  const settingsNote = await run("read_note", {id:settingsMarker.id});
  const {position, size} = JSON.parse(settingsNote.markdown.split("\n")[1]);
  const offset = Math.hypot(position.x + size.width / 2 - windowPosition.x - mainSize.width / 2,
    position.y + size.height / 2 - windowPosition.y - mainSize.height / 2);
  if (offset > 4) throw Error(`Settings window center differs from the main window by ${offset} pixels`);
  await waitFor(async () => !(await run("read_settings")).windows.settings);
  const imageRow = await workspaceFileRow("images.md");
  imageRow.click();
  await waitFor(() => focusedEditor()?.textContent.includes("Photo fixture"));
  const imageId = await run("open_library_file", {path:imageRow.dataset.file});
  const defaults = await run("image_options", {id:42});
  const workspace = await run("image_options", {id:imageId});
  if (defaults.mode !== "fixed" || workspace.mode !== "relative") throw Error("Incorrect image storage for inbox or workspace");
  // Paste a file through the mounted editor, then open the rendered asset through the native protocol.
  const canvas = document.createElement("canvas");
  canvas.width = 3;
  canvas.height = 2;
  const png = await new Promise(resolve => canvas.toBlob(resolve, "image/png"));
  const fixedLink = await api.invoke("store_note_image", await png.arrayBuffer(), {headers:{
    "x-jot-note":"42", "x-jot-name":"inbox.png", "x-jot-mode":defaults.mode, "x-jot-revision":String(defaults.revision)
  }});
  if (!fixedLink.startsWith("file:///")) throw Error("Inbox did not save to the fixed directory");
  const fixedPreview = new Image();
  fixedPreview.src = api.convertFileSrc("42/" + fixedLink);
  await fixedPreview.decode();
  if (fixedPreview.naturalWidth !== 3) throw Error("Fixed directory image preview failed");
  const transfer = new DataTransfer();
  transfer.items.add(new File([png], "粘贴 1.png", {type: "image/png"}));
  const pasteImage = () => {
    const paste = new Event("paste", {bubbles: true, cancelable: true});
    Object.defineProperty(paste, "clipboardData", {value: transfer});
    focusedEditor().dispatchEvent(paste);
  };
  pasteImage();
  await waitFor(async () => (await run("read_note", {id:imageId})).markdown.includes("notes/media/"), "saved image link");
  document.execCommand("insertParagraph");
  const image = await waitFor(() => [...document.querySelectorAll(".jot-editor img")].find(img => img.naturalWidth === 3), "native asset preview");
  if (image.naturalHeight !== 2) throw Error("Incorrect image dimensions");
  const blogId = await run("open_library_file", {path:root + "/blog/posts/entry.md"});
  await run("show_main", {id:blogId});
  await waitFor(() => focusedEditor()?.textContent.includes("Blog image rule fixture"));
  pasteImage();
  await waitFor(async () => (await run("read_note", {id:blogId})).markdown.includes("../../access/"), "blog image link");
  document.execCommand("insertParagraph");
  await waitFor(() => [...document.querySelectorAll(".jot-editor img")].some(img => img.naturalWidth === 3), "sibling directory image preview");
  const row = await workspaceFileRow();
  row.click();
  await waitFor(() => focusedEditor()?.textContent.includes("original-file"));
  document.execCommand("insertText", false, "edited-in-Jot ");
  const external = await waitFor(async () => (await run("list_notes", {query:"edited-in-Jot", offset:0})).items
    .find(card => card.file === row.dataset.file));
  await record("storage-open", {path: external.file});
  const copy = await verifyConflictCopy();
  const archivePath = root + "/archive";
  const folder = await run("save_folder", {input:{id:null, name:"会议", description:"", path:archivePath, classify:false}});
  const archiveImageId = await run("open_library_file", {path:archivePath + "/review.md"});
  if ((await run("image_options", {id:archiveImageId})).mode !== "http") throw Error("Archive override did not take precedence over workspace");
  await run("show_main", {id:archiveImageId});
  await waitFor(() => focusedEditor()?.textContent.includes("Archive image rule fixture"));
  pasteImage();
  await waitFor(async () => (await run("read_note", {id:archiveImageId})).markdown.includes("http://127.0.0.1:"), "archived note's uploaded image");
  document.execCommand("insertParagraph");
  await waitFor(() => [...document.querySelectorAll(".jot-editor img")].some(img => img.src.startsWith("http://127.0.0.1:") && img.naturalWidth === 3), "HTTP image preview");
  const original = await run("new_note");
  await run("save_note", {id:original, version:0, markdown:"归档失败后保留的正文"});
  const note = await run("read_note", {id:original});
  await record("storage-archive", {path:note.location.path});
  await waitFor(async () => !(await run("workspace_children", {path:root})).some(child => child.name === "archive"));
  let refused = false;
  try {
    await run("move_note", {id:original, folder:folder.id});
  } catch {
    refused = true;
  }
  if (!refused || (await run("read_note", {id:original})).location.path !== note.location.path) {
    throw Error("An unavailable archive changed the document's location");
  }
  const missing = await run("open_library_file", {path:root + "/removable.md"});
  await record("storage-saved", {external: external.id, copy, original, missing});
}

if (label === "main" && new URL(import.meta.url).searchParams.has("storage")) {
  try { await verifyStorage(); } catch (error) { await record("error", {error:String(error)}); }
} else if (label === "main") {
  try {
    const id = await run("initial_note");
    if (id === null) {
      await record("main-start", {});
      await verifyOutline();
      await record("outline-verified", {});
      await verifyListGeometry();
      await record("list-verified", {});
      await verifyDiskEditing();
      await verifyMissingFileRecovery();
      await verifyFolderCreation();
      const position = await run("plugin:window|inner_position", { label });
      const scale = await run("plugin:window|scale_factor", { label });
      const previousTab = document.querySelector('[role=tab][aria-selected=true]');
      if (!previousTab) throw Error("No active document before external drop");
      await record("receive-files", { x: position.x / scale + innerWidth * .76, y: position.y / scale + innerHeight * .5 });
      await waitFor(() => [...document.querySelectorAll('.document-tab-name')].filter(
        tab => tab.textContent.includes("外部拖入")).length === 2, "two real external files dropped into tabs");
      await waitFor(() => document.querySelector(".editor-root")?.textContent.includes("native file drag"), "dropped Markdown opened in editor");
      const droppedTab = await waitFor(() => {
        const active = document.querySelector('[role=tab][aria-selected=true]');
        return active?.textContent.includes("外部拖入") && active;
      }, "dropped file selected once its editor finishes opening");
      const beforeClose = document.querySelectorAll('[role=tab]').length;
      await record("close-active-tab", {});
      await waitFor(() => !droppedTab.isConnected && previousTab.getAttribute("aria-selected") === "true" &&
        document.querySelectorAll('[role=tab]').length === beforeClose - 1, "native Cmd+W closed only the active tab and restored the previous document");
      const settings = await run("read_settings");
      document.querySelector('button[aria-label="设置"]').click();
      await waitFor(async () => (await run("list_notes", { query: "#probe:settings-opened", offset: 0 })).items.length);
      await waitFor(async () => !(await run("read_settings")).windows.settings);
      await record("ready", { ...settings, outlinePassed: true, listGeometryPassed: true, missingFileRecovered: true, paneWidthsPassed: true, settingsPassed: true });
      await run("close_editor_window");
    } else {
      await verifyReattach(id);
    }
  } catch (error) {
    await record("error", { error: String(error) });
  }
}

if (label === "settings") {
  try {
    await waitFor(() => document.querySelector(".settings-nav"), "settings mount");
    const position = await run("plugin:window|outer_position", {label});
    const size = await run("plugin:window|outer_size", {label});
    await record("settings-opened", {position, size});
    await run("plugin:window|close", { label });
  } catch (error) {
    await record("error", {error:String(error)});
  }
}

if (label === "quick") {
  const page = Math.random();
  let previous;
  let sending = false;
  let recheck = false;
  const shell = document.querySelector("#app");
  async function ready() {
    if (sending) { recheck = true; return; }
    const editor = document.querySelector("[data-jot-editor-ready]");
    if (shell.dataset.mode !== "edit" || !editor || editor === previous || !document.hasFocus() ||
        !editor.contains(document.activeElement) || document.activeElement.contentEditable !== "true") return;
    const measured = Date.now();
    sending = true;
    try {
      const state = await run("quick_view");
      if (state.kind !== "edit" || shell.dataset.mode !== "edit" || !document.hasFocus() || !editor.contains(document.activeElement)) return;
      previous = editor;
      const note = await run("read_note", { id: state.id });
      if (note.file?.endsWith("/meeting.md")) {
        document.querySelector('button[aria-label="便签操作"]').click();
        const recall = await waitFor(() => document.querySelector('button[aria-label="解除挂载并收回"]'));
        recall.click();
        await record("disk-recalled", { id: note.id, source: note.source });
        return;
      }
      const samples = await run("list_notes", { query: "#probe:sample", offset: 0 });
      if (samples.items.length === sampleCount) {
        const reassigned = await run("list_notes", { query: "#probe:reattached", offset: 0 });
        if (!reassigned.items.length) {
          // Timing samples can leave an earlier note on the host used later by the drag fixture.
          const warmed = await run("list_notes", {query:"latency-probe first key", offset:0});
          for (const card of warmed.items) {
            if (card.attached && card.id !== state.id) await run("detach_note", {id:card.id});
          }
          await run("show_main", { id: state.id });
          return;
        }
        const keyboard = await run("list_notes", { query: "#probe:key-close", offset: 0 });
        const closed = await run("list_notes", { query: "#probe:native-close", offset: 0 });
        if (keyboard.items.length) {
          document.execCommand("insertText", false, "after-reopen-marker");
          await waitFor(async () => (await run("read_note", { id: state.id })).markdown.includes("after-reopen-marker"));
          await record("reopened-after-close", { id: state.id, page, source: note.source });
          await exerciseSelection(focusedEditor(), state);
          return;
        }
        if (closed.items.length) {
          document.execCommand("insertText", false, " save-through-key-close");
          await record("key-close", { id: state.id, page, typedAt: Date.now() });
          return;
        }
        if (!(await run("list_notes", { query: "#probe:layout-ready", offset: 0 })).items.length) {
          const pin = document.querySelector('button[aria-label="置顶便签"]');
          pin.click();
          await waitFor(async () => (await run("quick_view")).pinned && pin.getAttribute("aria-pressed") === "true");
          pin.click();
          await waitFor(async () => !(await run("quick_view")).pinned && pin.getAttribute("aria-pressed") === "false");
          const handle = document.querySelector('button[draggable=true][aria-label="拖动挂载或点选窗口"]');
          const rect = handle.getBoundingClientRect();
          const pos = await run("plugin:window|inner_position", { label });
          const scale = await run("plugin:window|scale_factor", { label });
          const target = note.source.endsWith("Fixture A") ? "B" : "A";
          await record("quick-drag-ready", { target, x: pos.x / scale + rect.x + rect.width / 2, y: pos.y / scale + rect.y + rect.height / 2 });
          await waitFor(async () => (await run("read_card", { id: state.id })).source.endsWith("Fixture " + target), "native drag from floating note changed its host");
          await waitFor(async () => {
            const view = await run("quick_view");
            return view.kind === "edit" && view.id === state.id && !view.standalone;
          }, "drag preserved floating editor");
          await record("layout-ready", { id: state.id });
          return;
        }
        const active = document.activeElement;
        const more = document.querySelector('button[aria-label="便签操作"]');
        const metadata = await run("read_card", { id: state.id });
        const filename = (metadata.file ?? metadata.archivePath ?? metadata.title).split("/").at(-1);
        await waitFor(() => document.querySelector(".quick-title")?.textContent === filename, "quick title shows the document filename");
        more.click();
        const menu = await waitFor(() => document.querySelector("#quick-actions:popover-open"));
        press(menu.querySelector("button"), "Escape", "Escape");
        if (menu.matches(":popover-open") || focusedEditor() !== active) throw new Error("Escape did not resume typing");
        more.click();
        await waitFor(() => menu.matches(":popover-open"));
        const toolbar = document.querySelector('button[aria-label="编辑工具栏"]');
        if (!toolbar || document.querySelector(".jot-editor-toolbar")) throw new Error("Quick toolbar should start collapsed");
        toolbar.click();
        await waitFor(() => document.querySelector('[data-jot-toolbar-command="toolbar.open-table"]'));
        document.execCommand("insertText", false, " undo-across-search");
        press(active, "k", "KeyK", true);
        const query = await waitFor(() => document.querySelector(".note-list input[type=search]"));
        if (!(await run("read_note", { id: state.id })).markdown.includes("undo-across-search")) {
          throw new Error("Search opened before the latest input was saved");
        }
        press(query, "Escape", "Escape");
        const restored = await waitFor(focusedEditor, "editor focus after canceling search");
        if ((await run("quick_view")).id !== state.id) throw new Error("Search cancel selected a different note");
        if (restored !== active) throw new Error("Search cancel replaced the editor");
        press(restored, "z", "KeyZ", true);
        await waitFor(async () => (await run("read_note", { id: state.id })).markdown === note.markdown);
        document.execCommand("insertText", false, " save-through-close");
        await record("native-close", { id: state.id, page, typedAt: Date.now() });
        return;
      }
      if (note.markdown === "") {
        const content = focusedEditor();
        if (!content || !document.execCommand("insertText", false, "latency-probe first key")) {
          throw Error("Focus or text input lost after waking the quick editor");
        }
        await waitFor(async () => (await run("read_note", { id: state.id })).markdown.includes("latency-probe first key"),
          "typed text persisted after debounce in note " + state.id).catch(error => {
          throw Error(error + " (visible text: " + content.textContent + ", status: " +
            document.querySelector(".quick-status")?.textContent + ")");
        });
        await verifyNewlineOrigin();
      }
      await run("dismiss_quick");
      const expand = await waitFor(() => shell.dataset.mode === "summary" &&
        document.querySelector('button[aria-label="展开便签"]')?.getBoundingClientRect(), "folded note ready to reopen");
      await record("sample", { measured, page, id: state.id, source: note.source,
        expand: [innerWidth - expand.right + expand.width / 2, expand.y + expand.height / 2] });
    } catch (error) {
      await record("error", { error: String(error), mode: (await run("quick_view")).kind, focused: document.hasFocus(), active: document.activeElement?.className });
    } finally {
      sending = false;
      if (recheck) {
        recheck = false;
        void ready();
      }
    }
  }

  async function exerciseSelection(reopened, state) {
    document.querySelector('button[aria-label="便签操作"]').click();
    const newNote = await waitFor(() => document.querySelector('#quick-actions:popover-open button[aria-label="新建便签"]'));
    newNote.click();
    const original = await run("read_card", { id: state.id });
    const second = await waitFor(async () => (await run("search_notes", {
      query:"second note on the same window", host:null, page:0
    })).attached.find(card => card.id !== state.id && card.source === original.source), "a separate note in a second native window");
    if (reopened !== document.querySelector(".cm-content[contenteditable=true]") || second.archivePath === original.archivePath) {
      throw Error("New note replaced the original editor or file");
    }
    const observed = [];
    await waitFor(async () => {
      const actual = await run("quick_view");
      const snapshot = [actual.kind, actual.id, actual.epoch, document.hasFocus(), document.querySelector("#app").dataset.mode];
      if (JSON.stringify(observed.at(-1)) !== JSON.stringify(snapshot)) observed.push(snapshot);
      return actual.kind === "edit" && actual.id === state.id && document.hasFocus()
        && focusedEditor()?.textContent.includes("after-reopen-marker");
    }, "edit earlier note chosen by immediate Enter").catch(async error => {
      const matches = (await run("search_notes", { query: "after-reopen-marker", host: null, page: 0 })).attached
        .map(card => [card.id,card.source]);
      throw new Error(error + " (wanted " + state.id + ", matches " + JSON.stringify(matches)
        + ", view transitions " + JSON.stringify(observed) + ", main document " + await run("initial_note") + ")");
    });

    press(focusedEditor(), "k", "KeyK", true);
    const remoteQuery = await waitFor(() => document.querySelector(".quick-query input"));
    remoteQuery.value = "other-host-search-target";
    remoteQuery.dispatchEvent(new Event("input", { bubbles:true }));
    const remote = await waitFor(() => {
      const row = document.querySelector('[role="group"][aria-label="其他窗口便签"] [aria-selected="true"]');
      return row?.textContent.includes(remoteQuery.value) && row;
    }, "the other application's note is selected in 其他窗口便签");
    const remoteId = Number(remote.dataset.note);
    const remoteCard = await run("read_card", { id: remoteId });
    if (remoteId === state.id || remoteCard.source === original.source ||
        (await run("read_note", { id: remoteId })).markdown !== "other-host-search-target") {
      throw Error("Cross-host result matched the wrong note");
    }
    // The driver submits a physical Return and inspects the native window ordering.
    await record("cross-host-query", { from: original.source, to: remoteCard.source, id: remoteId });
    await waitFor(async () => (await run("quick_view")).id === remoteId && document.hasFocus() &&
      focusedEditor()?.textContent.includes("other-host-search-target"), "keyboard selected the other host's editor")
      .catch(async error => { throw Error(error + ": wanted " + remoteId + ", view " + JSON.stringify(await run("quick_view"))); });
    await record("cross-host-opened", { id: remoteId });
    // Physical Cmd+K, sent after checking WindowServer, starts the return journey.
    const back = await waitFor(() => document.querySelector(".quick-query input"));
    back.value = "after-reopen-marker";
    back.dispatchEvent(new Event("input", { bubbles:true }));
    press(back, "Enter", "Enter");
    await waitFor(async () => (await run("quick_view")).id === state.id && document.hasFocus() &&
      focusedEditor()?.textContent.includes("after-reopen-marker"), "returned to original host through search");
    press(focusedEditor(), "k", "KeyK", true);
    await waitFor(() => document.activeElement?.matches(".quick-query input"));
    await record("close-search", { id: state.id });
    await waitFor(async () => (await run("quick_view")).id === state.id && document.hasFocus() &&
      focusedEditor()?.textContent.includes("after-reopen-marker"), "closing search restored the original editor");
    const afterClose = await run("read_card", { id: state.id });
    if (!afterClose.attached || afterClose.source !== original.source) throw Error("Closing search detached the note");
    document.execCommand("insertText", false, " search-close-kept-editing");
    await waitFor(async () => (await run("read_note", { id: state.id })).markdown.includes("search-close-kept-editing"));
    await record("search-close-kept-note", { id: state.id });
    document.querySelector('button[aria-label="折叠便签"]').click();
    const title = await waitFor(() => document.querySelector(".quick-summary-title"));
    if ((await run("quick_view")).note.id !== state.id) throw new Error("Summary switched to a more recently written note");
    const at = await run("plugin:window|inner_position", { label });
    const scale = await run("plugin:window|scale_factor", { label });
    const rect = title.getBoundingClientRect();
    await record("folded-drag", { x: at.x / scale + rect.x + rect.width / 2, y: at.y / scale + rect.y + rect.height / 2 });
    await waitFor(async () => {
      const moved = await run("plugin:window|inner_position", { label });
      return Math.abs(at.x - moved.x) > 20 * scale;
    }, "dragging the folded filename moves the native window");
    await waitFor(() => title.matches(":not(:active)"), "release folded title after drag");
    if ((await run("quick_view")).kind !== "summary" || document.querySelector(".quick-folded input")) throw new Error("Dragging the folded title activated it");
    title.click();
    const name = await waitFor(() => document.querySelector(".quick-folded input"));
    name.value = "召回校对.md";
    press(name, "Enter", "Enter");
    await waitFor(async () => {
      const c = await run("read_card", { id: state.id });
      return c.archivePath?.endsWith("/召回校对.md") && document.querySelector(".quick-summary-title")?.textContent === "召回校对.md";
    }, "rename saved to disk and visible on folded note");
    document.querySelector('button[aria-label="展开便签"]').click();
    const resumed = await waitFor(focusedEditor, "editor focus after clicking the summary");
    if ((await run("quick_view")).id !== state.id) throw new Error("Clicking the summary reopened another note");
    if (!(await run("read_note", { id: second.id })).markdown.includes("second note")) throw new Error("Search lost the other draft");
    press(resumed, "n", "KeyN", true);

  }
  new MutationObserver(changes => {
    const leftNote = value => value === "hidden" || value === "summary";
    if (leftNote(shell.dataset.mode) || changes.some(change => change.target === shell &&
        change.attributeName === "data-mode" && leftNote(change.oldValue))) previous = undefined;
    void ready();
  }).observe(document.body, { subtree: true, childList: true, attributeOldValue: true, attributeFilter: ["data-mode", "data-jot-editor-ready"] });
  document.addEventListener("focusin", ready);
  window.addEventListener("blur", () => { previous = undefined; });
  window.addEventListener("focus", ready);
  await ready();
}

if (label.startsWith("quick-")) {
  (async () => {
    const input = await waitFor(() => document.hasFocus() && focusedEditor(), "focused sibling editor");
    const view = await run("quick_view");
    const existing = await run("list_notes", {query:"#probe:folded-drag", offset:0, scope:null});
    if (existing.items.length) {
      document.execCommand("insertText", false, " save-through-quit");
      await record("quit", {id:view.id, typedAt:Date.now(), newShortcutWorked:true});
      return;
    }
    document.execCommand("insertText", false, "second note on the same window");
    press(input, "k", "KeyK", true);
    const search = await waitFor(() => document.querySelector(".quick-query input"));
    search.value = "after-reopen-marker";
    search.dispatchEvent(new Event("input", {bubbles:true}));
    press(search, "Enter", "Enter");
  })().catch(error => record("error", {label, error:String(error)}));
}
