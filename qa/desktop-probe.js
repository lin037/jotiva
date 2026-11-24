const api = window.__TAURI_INTERNALS__;
const label = api.metadata.currentWindow.label;
const os = document.documentElement.dataset.platform;
const text = "Desktop edit";
const scenario = new URL(import.meta.url).searchParams;
const run = (name, args) => api.invoke(name, args);
async function waitFor(check, description = "view", attempts = 220) {
  for (let attempt = 0; attempt < attempts; attempt++) {
    const result = await check();
    if (result) return result;
    await new Promise(resolve => setTimeout(resolve, 150));
  }
  throw Error("Timed out waiting for " + description + ": " + label + " " + document.body.innerText.slice(0, 400));
}
async function report(message) {
  const id = await run("new_note");
  await run("save_note", {id, version: 0, markdown: "# QA " + message});
}
try {
  if (label === "main") {
    window.addEventListener("jot:language", () => {
      const query = document.querySelector(".workspace-search input");
      if (query?.placeholder === (document.documentElement.lang === "en" ? "Search notes and documents" : "搜索便签和文档")) {
        localStorage.setItem("qa-workspace-language", document.documentElement.lang);
      }
    });
    if (!["windows", "linux"].includes(os) || location.protocol === "file:") throw Error("Wrong desktop origin or platform");
    const id = await waitFor(() => run("initial_note"));
    const editor = await waitFor(() => document.querySelector(".cm-content[contenteditable=true]"));
    editor.focus();
    if (!document.execCommand("insertText", false, text)) throw Error("Editor input failed");
    await waitFor(async () => (await run("read_note", {id})).markdown.includes(text));
    const historyKey = shift => editor.dispatchEvent(new KeyboardEvent("keydown", {
      key: shift ? "Z" : "z", code: "KeyZ", keyCode: 90, which: 90, ctrlKey: true, shiftKey: shift, bubbles: true, cancelable: true
    }));
    historyKey(false);
    await waitFor(() => !editor.textContent.includes(text), "Ctrl+Z");
    historyKey(true);
    await waitFor(() => editor.textContent.includes(text), "Ctrl+Shift+Z");
    await waitFor(async () => (await run("read_note", {id})).markdown.includes(text));
    const { checkTerminal } = await import("./terminal-probe.js");
    await checkTerminal(os, waitFor, run, report);
    const title = scenario.get("host");
    if (window.__jotPlatform.hostWindows !== Boolean(title)) throw Error("Incorrect host capability");
    if (scenario.has("desktop-input")) {
      const { workspace } = await import("./windows-probe.js");
      await workspace({id, run, report, waitFor, title});
      await report("attached");
      await waitFor(async () => !(await run("read_card", {id})).attached, "closed Edge host");
    } else if (title) {
      const host = await waitFor(async () => (await run("source_choices"))?.find(w => w.title === title));
      await run("attach_note", { id, choice: host.choice });
      if (!(await run("read_card", {id})).attached) throw Error("Window attachment missing");
      await run("clear_source_choices");
      await report("attached");
      await waitFor(async () => !(await run("read_card", {id})).attached);
    }
    if (scenario.has("wayland")) {
      await run("show_settings");
    } else {
      const menu = document.querySelector('.workspace-leading button[aria-haspopup="menu"]');
      if (!menu) throw Error("Missing application menu");
      await report("menu");
      menu.click();
      await waitFor(async () => (await run("plugin:window|get_all_windows")).includes("settings"), "Tools > Settings");
    }
    await waitFor(() => localStorage.getItem("qa-language-verified"), "the settings language round trip");
    if (scenario.has("desktop-input")) {
      await run("open_document_window", {id: null});
      await waitFor(() => localStorage.getItem("qa-new-window-name"), "new window named without a modal");
    }
    await run("open_document_window", {id});
    await waitFor(async () => !(await run("plugin:window|get_all_windows")).some(w => w.startsWith("document-")), "document close");
    const file = (await run("read_note", {id})).file;
    await run("reveal_file", {path: file});
    if (scenario.has("desktop-input")) {
      const folder = file.split(/[\\/]/).at(-2);
      const explorer = await waitFor(async () => (await run("source_choices"))?.find(
        row => row.app.toLowerCase() === "explorer" && row.title.includes(folder)), "test folder in Explorer");
      await run("attach_note", {id, choice: explorer.choice});
      const card = await run("read_card", {id});
      if (!card.attached || !card.source.includes(folder)) throw Error("Explorer attachment missing");
      await run("clear_source_choices");
    }
    document.querySelector('[aria-controls="workspace-terminal"]').click();
    await waitFor(async () => await run("live_terminals") === 1, "shell running before exit");
    await run("quit");
    const confirm = await waitFor(() => document.querySelector("dialog[open] .sheet-actions button[type=submit]"), "confirm exit with a running shell");
    await report("main");
    confirm.click();
  } else if (label === "quick" && scenario.has("desktop-input")) {
    const { mounted } = await import("./windows-probe.js");
    await mounted({run, report, waitFor});
  } else if (label.startsWith("document-") && window.__jotPlatform.nameOnOpen) {
    const name = await waitFor(() => document.querySelector(".tree-name-field input:focus"), "new document filename");
    if (document.querySelector("dialog[open]")) throw Error("Unexpected naming dialog");
    name.value = "QA-new-window.md";
    name.blur();
    const id = Number(label.slice("document-".length));
    await waitFor(async () => (await run("read_card", {id})).archivePath.endsWith("QA-new-window.md"), "name saved after blur");
    localStorage.setItem("qa-new-window-name", "1");
    await run("plugin:window|close");
  } else if (label.startsWith("document-")) {
    await waitFor(() => document.querySelector(".cm-content")?.textContent.includes(text));
    const controls = document.querySelector(".caption-controls");
    if (!controls) throw Error("Missing custom titlebar");
    const zoom = controls.querySelector('[data-caption="maximize"]');
    zoom.click();
    await waitFor(() => zoom.title === (os === "windows" ? "Restore window" : "还原窗口"), "maximize");
    zoom.click();
    await waitFor(() => zoom.title === (os === "windows" ? "Maximize" : "最大化"), "restore");
    if (!scenario.has("wayland")) {
      const title = document.querySelector(".caption-title");
      const rect = title.getBoundingClientRect();
      const left = await run("plugin:window|outer_position");
      await report((os === "windows" ? "input " : "drag ") + JSON.stringify({
        name: "drag document caption",
        title: await run("plugin:window|title"), x: Math.round((rect.x + rect.width / 2) * devicePixelRatio),
        y: Math.round((rect.y + rect.height / 2) * devicePixelRatio)
      }));
      await waitFor(async () => {
        const pos = await run("plugin:window|outer_position");
        return Math.abs(pos.x - left.x) > 12 || Math.abs(pos.y - left.y) > 12;
      }, "caption drag");
    }
    await report("document");
    controls.querySelector('[data-caption="close"]').click();
  } else if (label === "settings") {
    await waitFor(() => document.querySelector("button"));
    const language = await waitFor(() => document.querySelector("#interface-language"));
    const original = language.value;
    for (const value of [original === "en" ? "zh-CN" : "en", original]) {
      language.value = value;
      language.dispatchEvent(new Event("change", {bubbles: true}));
      await waitFor(() => document.documentElement.lang === value && localStorage.getItem("qa-workspace-language") === value,
        "live language in settings and workspace: " + value);
    }
    localStorage.setItem("qa-language-verified", "1");
    await report("settings");
  }
} catch (error) { await report("error " + label + " " + String(error)); }
