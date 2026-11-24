import { t } from "../../i18n";
import { secondaryTitlebar } from "../../ui/titlebar";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { events, notes, type NoteId } from "../../desktop/ipc";
import { bindWindowShortcuts } from "../../desktop/shortcuts";
import { saveBeforeClose } from "../../desktop/window-actions";
import { NoteEditor } from "../../editor/note-editor";
import { iconButton } from "../../ui/chrome";
import { element, report, root } from "../../ui/dom";
import { documentStatus } from "../../ui/document-status";
import { documentPath } from "../workspace/document-file-menu";
import { folderEditor, moveDocument } from "../workspace/folder-dialog";
import "../workspace/workspace.css";

export async function mount(id: NoteId) {
  secondaryTitlebar(document.title);
  root.className = "document-pane jot-document-window";
  const paper = element("main", "editor-root");
  const { footer, metadata, counter, status } = documentStatus();
  let move: ReturnType<typeof moveDocument> | undefined;
  const filePath = documentPath({
    current: () => editor,
    openCopy: copy => invoke("open_document_window", { id: copy }),
    move: async (note, folder) => {
      move ??= moveDocument(result => {
        status.textContent = result.warning ?? t("已移动");
        void refresh().catch(report);
      }, () => {
        editFolder ??= folderEditor(newFolder => {
          void notes.card(id).then(card => move!(id, card.folder, newFolder.id)).catch(report);
        });
        editFolder();
      });
      await move(note, folder);
    },
  });
  let editFolder: ReturnType<typeof folderEditor> | undefined;
  const pin = iconButton("pin", t("置顶文档窗口"), async () => {
    const pinned = pin.getAttribute("aria-pressed") !== "true";
    pin.disabled = true;
    try {
      await getCurrentWindow().setAlwaysOnTop(pinned);
      pin.setAttribute("aria-pressed", String(pinned));
      pin.dataset.tip = pinned ? t("取消置顶") : t("置顶");
    } finally { pin.disabled = false; }
  });
  pin.dataset.tip = t("置顶");
  pin.setAttribute("aria-pressed", "false");
  metadata.append(pin, filePath.control);
  root.append(paper, footer);
  let editor: NoteEditor | undefined;
  let gone = false;
  let closing = false;
  async function refresh() {
    if (!editor || gone || closing) return;
    const card = await notes.card(id);
    if (gone || closing) return;
    editor.setMenuContext(true, card.attached, card.file ? "file" : card.folder === null ? "draft" : "archive");
    filePath.update(card);
    if (document.title !== card.title) {
      await getCurrentWindow().setTitle(card.title);
      document.title = card.title;
    }
    if (card.version > editor.savedVersion) await editor.synchronize(card.version);
  }
  const create = () => invoke("open_document_window", { id: null });
  const find = (tiled: boolean) => invoke("show_find", { tiled });
  const start = NoteEditor.open(paper, id, status, { counter }).then(opened => {
    editor = opened;
    if (!gone && !closing) { editor.setMenuContext(true); editor.focus(); }
  });
  await saveBeforeClose(async () => {
    closing = true;
    try {
      await start.catch(() => {});
      if (gone) await editor?.removed();
      await editor?.close();
      editor = undefined;
      await invoke("close_editor_window");
    } finally { closing = false; }
  });
  await listen<number[]>("desktop:notes-removed", ({ payload }) => {
    if (payload.includes(id)) {
      gone = true;
      void getCurrentWindow().close().catch(report);
    }
  });
  await listen<number | null>(events.notesChanged, ({ payload }) => {
    if (payload == null || payload === id) void refresh().catch(report);
  });
  bindWindowShortcuts({
    main: () => invoke("show_main"), pin: () => pin.click(), tile: () => editor?.command("tile"),
    newDocument: create, save: () => editor?.save(), search: () => find(false),
    overview: () => find(true), settings: () => invoke("show_settings"), quit: () => invoke("quit"),
  });
  await getCurrentWindow().listen<string>(events.editorAction, ({ payload }) => {
    const actions: Record<string, () => unknown> = {
      pin: () => pin.click(), new: create, save: () => editor?.save(), file: () => editor?.file(),
      find: () => find(false), overview: () => find(true),
    };
    void Promise.resolve().then(() => actions[payload] ? actions[payload]() : editor?.command(payload)).catch(report);
  });
  await start;
  await refresh();
}
