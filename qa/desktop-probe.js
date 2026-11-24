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
    await report("main");
    if (os === "windows") await run("quit");
  } else if (label === "quick" && scenario.has("desktop-input")) {
    const { mounted } = await import("./windows-probe.js");
    await mounted({run, report, waitFor});
  } else if (label.startsWith("document-")) {
    await waitFor(() => document.querySelector(".cm-content")?.textContent.includes(text));
    const controls = document.querySelector(".caption-controls");
    if (!controls) throw Error("Missing custom titlebar");
    const zoom = controls.querySelector('[data-caption="maximize"]');
    zoom.click();
    await waitFor(() => zoom.title === "还原窗口", "maximize");
    zoom.click();
    await waitFor(() => zoom.title === "最大化", "restore");
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
    await report("settings");
  }
} catch (error) { await report("error " + label + " " + String(error)); }
