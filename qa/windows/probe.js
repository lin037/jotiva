// Runs inside the installed WebView2 runtime with an isolated profile shared by the test windows.
const editor = () => document.querySelector(".cm-content[contenteditable=true]");
const visible = el => el?.getBoundingClientRect().width > 0;
const flag = name => localStorage.getItem("qa-" + name);
const mark = (name, value = "1") => localStorage.setItem("qa-" + name, value);

async function input({run, report}, name, action) {
  await report("input " + JSON.stringify({name, title: await run("plugin:window|title"), ...action}));
}

function center(element) {
  const rect = element.getBoundingClientRect();
  if (!rect.width || !rect.height) throw Error("Drag target is hidden: " + element.className);
  return {x: Math.round((rect.left + rect.width / 2) * devicePixelRatio),
    y: Math.round((rect.top + rect.height / 2) * devicePixelRatio)};
}

function insert(value) {
  editor().focus();
  if (!document.execCommand("insertText", false, value)) throw Error("WebView edit failed");
}

export async function workspace(ctx) {
  const {id, run, waitFor, title} = ctx;
  if (document.documentElement.lang !== "en") throw Error("Expected the saved English interface");
  const settings = await run("read_settings");
  if (settings.shortcutError) throw Error(settings.shortcutError);
  const other = await run("new_note");
  await run("save_note", {id: other, version: 0, markdown: "Second tab"});
  await run("show_main", {id: other});
  await waitFor(() => document.querySelector('[role=tab][aria-selected=true]')?.dataset.note === String(other), "second tab");
  await waitFor(() => visible(editor()) && document.hasFocus(), "editor keyboard focus");
  editor().focus();
  await input(ctx, "Ctrl+Tab", {keys: ["ControlKey", "Tab"]});
  const tab = await waitFor(() => document.querySelector('[role=tab][aria-selected=true][data-note="' + id + '"]'), "previous document");
  await waitFor(async () => (await run("source_choices"))?.some(row => row.title.includes(title)), "real Edge host");
  await run("clear_source_choices");
  mark("mount-note", String(id));
  await input(ctx, "drag tab to Edge", {...center(tab), drop: true});
  await waitFor(async () => {
    const card = await run("read_card", {id});
    return card.attached && card.source.includes(title);
  }, "tab dropped onto Edge");
  await waitFor(() => flag("quick-edited"), "mounted window editing", 800);
  await run("show_main", {id});
  await waitFor(() => editor()?.textContent.includes("quick-sync"), "mounted edit in workspace");
  insert("main-sync");
  await waitFor(async () => (await run("read_note", {id})).markdown.includes("main-sync"), "workspace save");
  await run("float_note", {id, standalone: false});
  mark("main-edited");
  await waitFor(() => flag("quick-received"), "workspace edit in mounted window");
}

export async function mounted(ctx) {
  const {run, waitFor} = ctx;
  const id = Number(await waitFor(() => flag("mount-note"), "workspace drag"));
  await waitFor(async () => {
    const view = await run("quick_view");
    return view.kind === "edit" && view.id === id && visible(editor()) &&
      document.querySelector(".quick-view[data-mode=edit]:not([inert])") && await run("plugin:window|is_focused");
  }, "presented mounted editor");
  const header = document.querySelector(".quick-edit-pane .quick-titlebar");
  const close = header.querySelector('[data-caption="close"]');
  const title = header.querySelector(".quick-title");
  const controls = [...header.querySelectorAll(":scope > button, .caption-button")].filter(visible);
  const bounds = header.getBoundingClientRect();
  if (!visible(close) || !visible(title) || controls.some(button => {
    const rect = button.getBoundingClientRect();
    return rect.left < bounds.left || rect.right > bounds.right + 1 || document.elementFromPoint(
      rect.left + rect.width / 2, rect.top + rect.height / 2)?.closest("button") !== button;
  })) throw Error("Mounted caption controls overlap or extend outside the header");
  const pin = header.querySelector("[data-pin]");
  if (pin.getAttribute("aria-pressed") !== "true") pin.click();
  await waitFor(async () => (await run("quick_view")).pinned &&
    document.querySelector(".quick-edit-pane [data-pin]")?.getAttribute("aria-pressed") === "true", "mounted pin");
  const origin = await run("plugin:window|outer_position");
  await input(ctx, "drag mounted caption", center(title));
  await waitFor(async () => {
    const pos = await run("plugin:window|outer_position");
    return Math.abs(pos.x - origin.x) + Math.abs(pos.y - origin.y) > 15;
  }, "mounted caption moved");
  editor().focus();
  await input(ctx, "Ctrl+End", {keys: ["ControlKey", "End"]});
  await waitFor(() => editor().textContent.includes("LAST-PAGE") && document.querySelector(".cm-scroller").scrollTop > 0, "last page");
  await input(ctx, "Ctrl+Home", {keys: ["ControlKey", "Home"]});
  await waitFor(() => editor().textContent.includes("Desktop edit") && document.querySelector(".cm-scroller").scrollTop < 5, "first page");
  const beforeTask = (await run("read_note", {id})).markdown;
  await input(ctx, "Ctrl+Shift+9", {keys: ["ControlKey", "ShiftKey", "D9"]});
  await waitFor(async () => (await run("read_note", {id})).markdown === "- [ ] " + beforeTask, "task list shortcut");
  await input(ctx, "undo task list", {keys: ["ControlKey", "Z"]});
  await waitFor(async () => (await run("read_note", {id})).markdown === beforeTask, "task list undo");
  insert("quick-sync");
  await waitFor(async () => (await run("read_note", {id})).markdown.includes("quick-sync"), "mounted input on disk");
  await input(ctx, "Ctrl+Z", {keys: ["ControlKey", "Z"]});
  await waitFor(() => !editor().textContent.includes("quick-sync"), "undo");
  await input(ctx, "Ctrl+Y", {keys: ["ControlKey", "Y"]});
  await waitFor(() => editor().textContent.includes("quick-sync"), "Windows redo");
  let savePressed = false;
  const onKey = event => { if (event.ctrlKey && event.code === "KeyS") savePressed = true; };
  window.addEventListener("keydown", onKey, true);
  await input(ctx, "Ctrl+S", {keys: ["ControlKey", "S"]});
  await waitFor(() => savePressed, "Ctrl+S delivered to mounted window");
  window.removeEventListener("keydown", onKey, true);
  await waitFor(async () => (await run("read_note", {id})).markdown.includes("quick-sync"), "saved text");
  mark("quick-edited");
  await waitFor(() => flag("main-edited") && editor()?.textContent.includes("main-sync"), "pinned editor receives workspace edit");
  const collapse = await waitFor(() => document.querySelector('.quick-edit-pane button[aria-label="Collapse note"]'), "fold action");
  collapse.click();
  await waitFor(() => visible(document.querySelector(".quick-folded-bar [data-caption=close]")), "folded window controls");
  await input(ctx, "global quick capture from Edge", {external: true, keys: ["ControlKey", "ShiftKey", "F9"]});
  await waitFor(async () => document.querySelector(".quick-view[data-mode=edit]") && visible(editor()) &&
    editor().textContent.includes("main-sync") && await run("plugin:window|is_focused"), "global shortcut reopened mounted note");
  mark("quick-received");
}
