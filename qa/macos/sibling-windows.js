const api = window.__TAURI_INTERNALS__;
const options = new URL(import.meta.url).searchParams;
const keepMain = options.get("main") === "open";
const label = api.metadata.currentWindow.label;
const run = (command, args = {}) => api.invoke(command, args);
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
const editor = () => document.querySelector("[data-jot-editor-ready] .cm-content[contenteditable=true]");
async function wait(read, description) {
  const end = Date.now() + 25000;
  do {
    const result = await read();
    if (result) return result;
    await sleep(55);
  } while (Date.now() < end);
  throw Error("Timeout: " + description);
}
async function record(kind, data) {
  const id = await run("new_note");
  await run("save_note", { id, version: 0, markdown: "#probe:" + kind + "\n" + JSON.stringify(data) });
}
async function type(text, id) {
  const input = await wait(editor, "editable body");
  input.focus();
  if (!document.execCommand("insertText", false, text)) throw Error("Could not enter " + text);
  await wait(async () => (await run("read_note", {id})).markdown.includes(text), "saved " + text);
}
async function checkRestart() {
  if (label !== "main") return;
  const previous = (await run("list_notes", {query:"#probe:pair", offset:0})).items[0];
  if (previous) {
    const marker = await run("read_note", {id:previous.id});
    const { id, second } = JSON.parse(marker.markdown.split("\n")[1]);
    await wait(async () => (await Promise.all([id, second].map(id => run("read_card", {id}))))
      .every(card => card.attached), "both attachments recovered");
    await record("bindings-restored", {});
    return;
  }
  if (!await run("accessibility_trusted")) throw Error("The native restart probe needs macOS Accessibility access");
  await record("main-ready", {});
  const host = await wait(async () => (await run("source_choices"))?.find(choice => choice.title === "Fixture A"),
    "selectable Fixture A");
  const source = await run("register_source", {choice:host.choice});
  const id = await run("create_from_text", {text:"original-note", source});
  const second = await run("create_from_text", {text:"second-note", source});
  await run("locate", {id});
  await wait(async () => (await run("quick_view")).id === id &&
    await run("plugin:window|is_visible", {label:"quick"}), "first note on the host");
  await run("locate", {id:second});
  await wait(async () => (await run("plugin:window|get_all_windows")).includes("quick-" + second) &&
    await run("plugin:window|is_visible", {label:"quick-" + second}), "second note on the host");
  const cards = await Promise.all([id, second].map(id => run("read_card", {id})));
  await record("pair", {id, second, paths:cards.map(card => card.archivePath), source:cards[0].source});
  await run("quit");
}

async function check() {
  if (options.has("restart")) return checkRestart();
  if (label === "main") {
    await wait(() => document.querySelector(".jot-workspace"), "main ready");
    await record("main-ready", {});
    if (!keepMain) {
      await run("close_editor_window");
      return;
    }
    await wait(() => document.hasFocus() && editor()?.innerText.includes("second-note"), "second document in main");
    const original = (await run("search_notes", {query:"original-note", host:null, page:0})).attached[0];
    const row = await wait(() => document.querySelector(`[data-source-state=live] [data-note="${original.id}"]`), "original in attached tree");
    row.click();
    return;
  }
  if (label === "quick") {
    // The prewarmed window can be idle until the driver activates its host.
    await wait(() => editor() && document.hasFocus(), "host capture");
    const { id } = await run("quick_view");
    await type("original-note", id);
    const original = editor();
    if (new URL(import.meta.url).searchParams.has("repeat")) {
      await record("repeat", {});
    } else {
      document.querySelector('button[aria-label="便签操作"]').click();
      const add = await wait(() => document.querySelector('#quick-actions:popover-open button[data-new-note]'), "new note menu");
      add.click();
    }
    await wait(async () => (await run("plugin:window|get_all_windows")).some(w => w.startsWith("quick-")), "second native window");
    const sibling = (await run("plugin:window|get_all_windows")).find(w => w.startsWith("quick-"));
    const second = Number(sibling.slice(6));
    await wait(async () => (await run("read_note", {id:second})).markdown.includes("second-note"), "separate editor saved");
    if (editor() !== original || (await run("quick_view")).id !== id) throw Error("Original editor was replaced");
    for (const win of [label, sibling]) {
      if (!await run("plugin:window|is_visible", {label:win})) throw Error(win + " was hidden");
    }
    const [a,b] = await Promise.all([label,sibling].map(label => run("plugin:window|outer_position", {label})));
    if (a.x === b.x && a.y === b.y) throw Error("New note covered the original at the same position");
    if ((await run("read_settings")).windows.main !== keepMain) throw Error("New note changed the main window lifetime");
    const cards = await Promise.all([id,second].map(id => run("read_card", {id})));
    if (!cards.every(c => c.attached) || cards[0].source !== cards[1].source || cards[0].archivePath === cards[1].archivePath) throw Error("Incorrect attachment or file " + JSON.stringify(cards));
    await record("pair", { id, second, paths:cards.map(c=>c.archivePath), positions:[a,b], source:cards[0].source });
    if (keepMain) {
      await wait(() => original === editor() && document.hasFocus(), "tree click refocused the original editor");
      await wait(() => run("plugin:window|is_visible", {label:sibling}), "other note also returned with its host");
      await type("+selected-while-both-open", id);
      await run("locate", {id:second});
    }
    await wait(async () => !(await run("plugin:window|get_all_windows")).includes(sibling), "close only the new note");
    if (!await run("plugin:window|is_visible", {label}) || editor() !== original || !(await run("read_card", {id})).attached || (await run("read_card", {id:second})).attached) throw Error("Closing one affected the other");
    await run("locate", {id});
    await wait(() => document.hasFocus(), "original refocused");
    await type("+still-editable", id);
    await record("success", {});
  } else if (label.startsWith("quick-")) {
    await wait(() => editor() && document.hasFocus(), "new native window focus");
    const { id } = await run("quick_view");
    if (id !== Number(label.slice(6))) throw Error("Wrong document in new window");
    const note = await run("read_note", {id});
    if (note.markdown !== "") throw Error("New document must start empty");
    await type("second-note", id);
    if (keepMain) {
      await wait(() => document.querySelector(".quick-query input"), "driver opened note search");
      await run("dismiss_quick");
      await wait(() => editor() && document.querySelector("#app").dataset.mode === "edit", "returned to second note");
      document.querySelector('button[aria-label="便签操作"]').click();
      document.querySelector('#quick-actions button[aria-label="在主窗口编辑"]').click();
      await wait(async () => {
        const matches = await run("search_notes", {query:"+selected-while-both-open", host:null, page:0});
        return matches.attached.length && document.hasFocus();
      }, "both documents selectable without closing either");
      await record("reselected", {});
    }
  }
}
check().catch(error => record("error", {label, error:String(error), body:document.body.innerText.slice(0,180)}));
