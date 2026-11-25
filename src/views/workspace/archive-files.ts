import type { OpenMode } from "./document-tabs";
import { localized } from "../../i18n";
import { afterNaming, fileName } from "../../ui/file-name";
import { invoke } from "@tauri-apps/api/core";
import { openLibraryFile, type NoteId } from "../../desktop/ipc";
import { button, element } from "../../ui/dom";

interface ArchiveEntry {
  path: string;
  id: NoteId | null;
  managed: boolean;
}

/** List filenames on expansion; a previously unseen file is opened only when chosen. */
export function archiveFiles(folder: number | null, open: (id: NoteId, mode?: OpenMode) => unknown, mark: (row: HTMLElement) => void) {
  const panel = element("div", "library-notes");
  const rows = element("div", "results");
  panel.append(rows);
  let active: NoteId | undefined;
  let activePath: string | undefined;
  let entries: ArchiveEntry[] = [];
  let visible = 80;
  let request = 0;
  let loaded = false;

  function draw() {
    if (afterNaming(rows, draw)) return;
    const shown = entries.slice(0, visible);
    const selected = entries.find(entry => entry.id === active || entry.path === activePath);
    if (selected && !shown.includes(selected)) shown[shown.length - 1] = selected;
    rows.replaceChildren(...shown.map(entry => {
      const name = fileName(entry.path);
      const row = button("", async event => open(entry.id ?? await openLibraryFile(entry.path), event.detail === 1 ? "preview" : "keep"), "note-row");
      row.append(element("strong", undefined, name));
      row.title = name;
      row.dataset.file = entry.path;
      if (entry.managed) row.dataset.archivePath = entry.path;
      if (entry.id !== null) row.dataset.note = String(entry.id);
      row.draggable = true;
      mark(row);
      if (selected === entry) row.setAttribute("aria-current", "true");
      return row;
    }));
    if (!shown.length) rows.append(element("p", "muted", localized("暂无 Markdown 文件")));
    if (shown.length < entries.length) rows.append(button(localized("显示更多"), () => { visible += 80; draw(); }, "library-node"));
  }
  return {
    panel,
    async refresh() {
      const turn = ++request;
      let incoming: ArchiveEntry[];
      try {
        incoming = await invoke<ArchiveEntry[]>("archive_entries", { folder });
      } catch (error) {
        if (turn !== request) return;
        loaded = false;
        entries = [];
        rows.replaceChildren(element("p", "muted", localized("无法读取目录")));
        throw error;
      }
      if (turn !== request) return;
      if (loaded && incoming.length === entries.length && incoming.every((e, i) =>
        e.path === entries[i].path && e.id === entries[i].id && e.managed === entries[i].managed)) return;
      loaded = true;
      entries = incoming;
      draw();
    },
    setActive(id: NoteId, path?: string) {
      if (id === active && path === activePath) return;
      active = id;
      activePath = path;
      const row = Array.from(rows.querySelectorAll<HTMLElement>("[data-file]")).find(row =>
        row.dataset.note === String(id) || row.dataset.file === path);
      rows.querySelector("[aria-current]")?.removeAttribute("aria-current");
      if (row) row.setAttribute("aria-current", "true");
      else if (entries.some(entry => entry.id === id || entry.path === path)) draw();
    },
  };
}
