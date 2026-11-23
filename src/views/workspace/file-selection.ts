import { desktopPlatform, displayPath } from "../../desktop/platform";
import { deleteSelectionPressed } from "../../desktop/platform";
import { invoke } from "@tauri-apps/api/core";
import { copyText, popupMenu } from "../../ui/context-menu";
import { report } from "../../ui/dom";
import { fileName } from "../../ui/file-name";
import { fileDialog } from "../../ui/file-dialog";
import "./file-selection.css";

/** Selection follows visible file paths across row repaints; opening a file remains a separate action. */
export function fileSelection(panel: HTMLElement, rename: (row: HTMLElement) => void,
  relative: (path: string) => string, flush: (paths: string[]) => Promise<void>) {
  const selected = new Set<string>();
  let anchor: HTMLElement | undefined;
  let lastClick = -Infinity;
  const rows = () => Array.from(panel.querySelectorAll<HTMLElement>("button[data-file]"));
  const sync = (row: HTMLElement) => row.setAttribute("aria-pressed", String(selected.has(row.dataset.file!)));
  const paint = () => {
    panel.classList.toggle("has-file-selection", selected.size > 0);
    rows().forEach(sync);
  };
  function clear() { selected.clear(); anchor = undefined; lastClick = -Infinity; paint(); }
  function paths(row: HTMLElement) {
    const path = row.dataset.file;
    if (!path || !selected.has(path)) return path ? [path] : [];
    return [...new Set(rows().map(item => item.dataset.file!).filter(file => selected.has(file)))];
  }
  panel.addEventListener("click", event => {
    const row = (event.target as Element).closest<HTMLElement>("button[data-file]");
    if (!row) { if (!(event.target instanceof HTMLInputElement)) clear(); return; }
    const path = row.dataset.file!;
    const toggle = event.metaKey || event.ctrlKey;
    const wasSingle = selected.size === 1 && selected.has(path);
    if (event.shiftKey) {
      const items = rows();
      const start = anchor?.isConnected ? items.indexOf(anchor) : items.findIndex(item => item.dataset.file === anchor?.dataset.file);
      const end = items.indexOf(row);
      if (!toggle) selected.clear();
      if (start >= 0) items.slice(Math.min(start, end), Math.max(start, end) + 1).forEach(item => selected.add(item.dataset.file!));
      else { selected.add(path); anchor = row; }
    } else if (toggle) {
      if (selected.has(path)) selected.delete(path); else selected.add(path);
      anchor = row;
    } else {
      selected.clear(); selected.add(path); anchor = row;
    }
    paint();
    const slowRepeat = event.detail === 1 && wasSingle && Number.isFinite(lastClick) && performance.now() - lastClick >= 500;
    if (toggle || event.shiftKey || slowRepeat) {
      event.preventDefault(); event.stopImmediatePropagation();
      row.focus({ preventScroll: true });
      if (slowRepeat && !toggle && !event.shiftKey) rename(row);
      lastClick = -Infinity;
    } else { lastClick = event.detail === 1 ? performance.now() : -Infinity; }
  }, true);
  // Returning to this file after using the editor is an open, even though its row is still selected.
  document.addEventListener("pointerdown", event => {
    if (!panel.contains(event.target as Node)) lastClick = -Infinity;
  }, true);
  document.addEventListener("keydown", event => {
    if (!panel.contains(event.target as Node)) lastClick = -Infinity;
  }, true);
  async function mutate(files: string[], directory?: string) {
    if (!directory) {
      const sample = files.slice(0, 3).map(fileName).join("、") + (files.length > 3 ? " 等" : "");
      if (await fileDialog(`将选中的 ${files.length} 个文件移到${desktopPlatform.trash}？`, `移到${desktopPlatform.trash}`, undefined, sample) === undefined) return;
    }
    await flush(files);
    await invoke("file_selection_action", { paths: files, action: directory ? { kind: "move", directory } : { kind: "trash" } });
    clear();
  }
  panel.addEventListener("contextmenu", event => {
    const row = (event.target as Element).closest<HTMLElement>("button[data-file]");
    if (!row) return;
    const files = paths(row);
    lastClick = -Infinity;
    if (files.length <= 1) {
      selected.clear(); selected.add(row.dataset.file!); anchor = row; paint();
      return;
    }
    event.stopImmediatePropagation();
    void popupMenu(event, [
      { label: "复制绝对路径", run: () => copyText(files.map(displayPath).join("\n")) },
      { label: "复制相对路径", run: () => copyText(files.map(relative).join("\n")) }, null,
      { label: `移到${desktopPlatform.trash}…`, run: () => mutate(files) },
    ]);
  }, true);
  panel.addEventListener("keydown", event => {
    if (event.isComposing || event.target instanceof HTMLInputElement) return;
    const row = (event.target as Element).closest<HTMLElement>("button[data-file]");
    if (!row) return;
    if ((event.metaKey || event.ctrlKey) && event.code === "KeyA") {
      event.preventDefault(); rows().forEach(item => selected.add(item.dataset.file!)); paint();
    } else if (event.key === "Escape") { clear(); }
    else if (event.key === "F2" && paths(row).length === 1) { event.preventDefault(); rename(row); }
    else if (deleteSelectionPressed(event)) {
      const files = paths(row);
      if (files.length > 1) { event.preventDefault(); void mutate(files).catch(report); }
    }
  });
  function follow(path?: string) {
    if (path && selected.size === 1 && selected.has(path)) return;
    clear();
    const row = path && rows().find(item => item.dataset.file === path);
    if (row) {
      selected.add(row.dataset.file!);
      anchor = row;
      paint();
    }
  }
  return { sync, paths, move: mutate, clear, follow };
}
