import { t } from "../../i18n";
import { fileName } from "../../ui/file-name";
import { desktopPlatform, displayPath } from "../../desktop/platform";
import { invoke } from "@tauri-apps/api/core";
import { notes, type Card, type NoteId } from "../../desktop/ipc";
import type { NoteEditor } from "../../editor/note-editor";
import { copyText, popupMenu } from "../../ui/context-menu";
import { element } from "../../ui/dom";
import { fileDialog } from "../../ui/file-dialog";

export function noteActions(resolveId: () => NoteId | Promise<NoteId>, name: string, openCopy: (id: NoteId) => unknown, save: (id: NoteId) => Promise<void>) {
  return {
    rename: { label: t("重命名…"), run: async () => {
      const next = await fileDialog(t("重命名"), t("保存"), { initial: name });
      if (!next || next === name) return;
      const id = await resolveId();
      await save(id);
      await notes.rename(id, next);
    } },
    duplicate: { label: t("制作副本"), run: async () => {
      const id = await resolveId();
      await save(id);
      await openCopy(await invoke<NoteId>("note_action", { id, action: { kind: "duplicate" } }));
    } },
    trash: { label: t("移到{trash}…", { trash: desktopPlatform.trash }), run: async () => {
      if (await fileDialog(t("将“{name}”移到{trash}？", { name, trash: desktopPlatform.trash }), t("移到{trash}", { trash: desktopPlatform.trash })) === undefined) return;
      const id = await resolveId();
      await save(id);
      await invoke("note_action", { id, action: { kind: "trash" } });
    } },
  };
}

export function documentPath(manage?: {
  current: () => NoteEditor | undefined;
  openCopy: (id: NoteId) => unknown;
  move: (id: NoteId, folder: number | null) => Promise<void>;
}) {
  const control = element("button", "workspace-move workspace-location");
  control.type = "button";
  control.hidden = true;
  const label = element("span", "workspace-path-label");
  control.append(label);
  let card: Card | undefined;

  function show(event: MouseEvent) {
    const active = card;
    const path = active?.file ?? active?.archivePath;
    if (!active || !path) return;
    const location = [
      { label: desktopPlatform.reveal, run: () => invoke("reveal_file", { path }) },
      { label: t("复制绝对路径"), run: () => copyText(displayPath(path)) },
    ];
    if (!manage) {
      void popupMenu(event, location);
      return;
    }
    const editor = manage.current();
    if (editor?.id !== active.id) return;
    const name = active.file ? fileName(path) : active.title;
    const actions = noteActions(() => editor.id, name, manage.openCopy, () => editor.save());
    void popupMenu(event, [
      ...location,
      ...(!active.file ? [{ label: t("移到归档目录…"), run: async () => {
        await editor.save();
        await manage.move(active.id, active.folder);
      } }] : []),
      null, actions.rename, actions.duplicate, null, actions.trash,
    ]);
  }
  control.addEventListener("click", show);
  control.addEventListener("contextmenu", show);
  return {
    control,
    update(value?: Card) {
      card = value;
      const path = displayPath(value?.file ?? value?.archivePath ?? "");
      control.hidden = !path;
      if (control.dataset.fileMenuPath === path) return;
      label.textContent = control.dataset.fileMenuPath = control.title = path;
      control.setAttribute("aria-label", t("文件操作，") + path);
    },
  };
}
