import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { openLibraryFile, sources, type NoteId } from "../../desktop/ipc";
import { allowWindowAccess } from "../../desktop/window-access";
import { button, element, report } from "../../ui/dom";

export const workspaceFileType = "application/x-jot-file";

type Payload = { kind: "note"; id: NoteId } | { kind: "file"; path: string } | { kind: "text"; text: string };
type Target = { source: string } | { choice: number };
const noteType = "application/x-jot-note";

function payload(data: DataTransfer): Payload | undefined {
  const id = data.getData(noteType);
  if (/^[1-9]\d*$/.test(id) && Number.isSafeInteger(Number(id))) return { kind: "note", id: Number(id) };
  const path = data.getData(workspaceFileType);
  if (path) return { kind: "file", path };
  const text = data.getData("text/plain");
  if (text.trim()) return { kind: "text", text };
}

export async function attachOnSwitch(id: NoteId) {
  if (!await allowWindowAccess("切换应用挂载")) return;
  const dialog = element("dialog", "sheet jot-dialog");
  dialog.setAttribute("aria-label", "切换应用挂载");
  const title = element("h2", undefined, "切换到要挂载的窗口");
  const message = element("p", "form-message", "按 ⌘Tab 或点选目标窗口。");
  message.setAttribute("role", "status");
  const actions = element("div", "sheet-actions");
  let waiting = true;
  const unlisten = await getCurrentWindow().listen<{ Ok: string } | { Err: string }>("desktop:attachment-finished", ({ payload }) => {
    waiting = false;
    if ("Ok" in payload) dialog.close();
    else message.textContent = payload.Err;
  });
  async function cancel() {
    if (waiting) await invoke("cancel_attachment");
    waiting = false;
  }
  actions.append(button("从列表选择…", async () => {
    await cancel();
    dialog.close();
    const { windowPicker } = await import("./source-dialog");
    windowPicker()("选择挂载窗口", "挂载", async choice => { await invoke("attach_note", { id, choice }); });
  }), button("取消", () => dialog.close()));
  dialog.addEventListener("close", () => {
    unlisten();
    void cancel().catch(report);
    dialog.remove();
  }, { once: true });
  dialog.append(title, message, actions);
  document.body.append(dialog);
  dialog.showModal();
  try { await invoke("attach_on_activation", { id }); }
  catch (error) { waiting = false; message.textContent = String(error); }
}

export function documentDrag(
  sidebar: HTMLElement,
  flush: (id: NoteId) => Promise<void>,
  pick: (apply: (choice: number) => Promise<void>) => void,
  reveal: (source: string) => Promise<void>,
) {
  let outgoing: Payload | undefined;
  let highlighted: Element | undefined;

  function clearHighlight() {
    highlighted?.classList.remove("drop-target");
    highlighted = undefined;
  }
  function destination(event: DragEvent) {
    const types = event.dataTransfer?.types;
    if (!types?.some(type => type === noteType || type === workspaceFileType || type === "text/plain")) return;
    const target = event.target instanceof Element ? event.target : null;
    const live = target?.closest<HTMLElement>('[data-source-state="live"]');
    if (live) return target?.closest<HTMLElement>("[data-source]") ?? live;
  }
  async function place(item: Payload, target?: Target) {
    if (item.kind === "text") {
      return invoke<NoteId>("create_from_text", { text: item.text, ...target });
    }
    const id = item.kind === "file" ? await openLibraryFile(item.path) : item.id;
    await flush(id);
    if (target) await invoke("attach_note", { id, ...target });
    return id;
  }
  document.addEventListener("dragstart", event => {
    outgoing = undefined;
    const data = event.dataTransfer;
    if (!data) return;
    if (!(event.target instanceof Element)) return;
    const row = event.target.closest<HTMLElement>("[data-note], [data-file]");
    if (row?.draggable) {
      if (row.dataset.note) data.setData(noteType, row.dataset.note);
      else if (row.dataset.file) data.setData(workspaceFileType, row.dataset.file);
      data.effectAllowed = row.dataset.file ? "copyMove" : "copy";
      outgoing = payload(data);
    } else if (event.target.closest(".jot-editor")) {
      const selected = document.getSelection()?.toString();
      if (selected?.trim()) outgoing = { kind: "text", text: selected };
    }
    if (outgoing) void invoke("observe_document_drag", { started: true }).catch(report);
  });
  document.addEventListener("keydown", event => {
    if (event.key === "Escape") { outgoing = undefined; clearHighlight(); }
  }, true);
  sidebar.addEventListener("dragover", event => {
    const target = destination(event);
    if (!target) { clearHighlight(); return; }
    event.preventDefault();
    event.dataTransfer!.dropEffect = "copy";
    if (highlighted !== target) {
      clearHighlight();
      highlighted = target;
      target.classList.add("drop-target");
    }
  });
  sidebar.addEventListener("dragleave", event => {
    if (!(event.relatedTarget instanceof Node) || !highlighted?.contains(event.relatedTarget)) clearHighlight();
  });
  sidebar.addEventListener("drop", event => {
    const target = destination(event);
    if (!target) return;
    event.preventDefault();
    const item = payload(event.dataTransfer!) ?? outgoing;
    outgoing = undefined;
    clearHighlight();
    if (!item) return;
    const key = target.dataset.source;
    if (key) {
      void place(item, { source: key }).then(() => reveal(key)).catch(report);
    } else {
      pick(async choice => {
        const key = await sources.add(choice);
        await place(item, { source: key });
        await reveal(key);
      });
    }
  });
  document.addEventListener("dragend", () => {
    clearHighlight();
    const item = outgoing;
    outgoing = undefined;
    if (!item) { void invoke("observe_document_drag", { started: false }).catch(report); return; }
    void (async () => {
      const hit = await invoke<{ outside: boolean; choice: number | null }>("source_at_drop").catch(() => {
        pick(async choice => {
          const id = await place(item, { choice });
          await invoke("float_note", { id, standalone: false });
        });
        return null;
      });
      if (!hit?.outside) return;
      const { choice } = hit;
      const id = await place(item, choice === null ? undefined : { choice });
      await invoke("float_note", { id, standalone: choice === null });
    })().catch(report);
  });
}
