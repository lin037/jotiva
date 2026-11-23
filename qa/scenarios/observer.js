// The native runner reads only state from its own temporary application.
const api = window.__TAURI_INTERNALS__;
const label = api.metadata.currentWindow.label;
let scheduled = false;
let previous = "";
let epoch = -1;
if (label.startsWith("quick")) {
  void api.invoke("plugin:event|listen", {
    event: "desktop:quick-view",
    target: { kind: "Any" },
    handler: api.transformCallback(event => { epoch = event.payload.epoch; queue(); }),
  });
}

function report() {
  scheduled = false;
  const input = document.querySelector(".cm-content");
  const active = document.activeElement;
  const tree = active?.closest(".library-tree");
  const frame = JSON.stringify({
    label,
    epoch,
    focused: document.hasFocus(),
    input: !!input && active === input,
    ready: !!document.querySelector("[data-jot-editor-ready]"),
    error: document.querySelector("[role=alert]")?.textContent,
    mode: document.querySelector("[data-jot-editor-mode]")?.dataset.jotEditorMode,
    view: document.querySelector("#app")?.dataset.mode,
    text: input?.innerText,
    pin: document.querySelector("[data-pin]")?.ariaPressed,
    search: active?.type === "search",
    tree: tree && (active.closest(".archive-tree") ? "archive" : "workspace"),
    childrenReady: !!document.querySelector(".workspace-files [data-file]"),
    row: active?.closest(".library-tree button")?.textContent,
    tab: document.querySelector('[role=tab][aria-selected=true]')?.textContent,
    results: [...document.querySelectorAll("[role=option]")].map(row => row.textContent),
  });
  if (frame === previous) return;
  previous = frame;
  void api.invoke("qa_frame", { frame });
}

function queue() {
  if (scheduled) return;
  scheduled = true;
  setTimeout(report, 0);
}

new MutationObserver(queue).observe(document.body, { subtree: true, attributes: true, childList: true, characterData: true });
for (const event of ["focus", "blur", "focusin", "input", "keyup"]) window.addEventListener(event, queue, true);
queue();
