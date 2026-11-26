import type { OpenMode } from "./document-tabs";
import { t, bindLabel, localized } from "../../i18n";
import { desktopPlatform, displayPath } from "../../desktop/platform";
import { invoke } from "@tauri-apps/api/core";
import { openLibraryFile, type NoteId } from "../../desktop/ipc";
import { listen } from "@tauri-apps/api/event";
import { popupMenu, copyText } from "../../ui/context-menu";
import { attachOnSwitch, workspaceFileType } from "./document-drag";
import { afterNaming, createInline, editFileName, nameCreatedDocument, fileName, isDescendant } from "../../ui/file-name";
import { fileSelection } from "./file-selection";
import { fileDialog } from "../../ui/file-dialog";
import { button, element, report } from "../../ui/dom";
import type { OpenTerminal } from "./terminal";
import { treeRow } from "./source-tree";
import { manageFolderImages } from "../../ui/image-dialog";

interface FileEntry {
  name: string;
  path: string;
  directory: boolean;
}

export function workspaceFiles(openNote: (id: NoteId, mode?: OpenMode) => unknown, flush: (paths: string[]) => Promise<void>, newTerminal: OpenTerminal) {
  const panel = element("div", "workspace-files");
  bindLabel(panel, "aria-label", localized("工作区文件"));
  const expanded = new Set<string>();
  const readers = new WeakMap<HTMLElement, () => Promise<void>>();
  let roots: string[] = [];
  let selected: string | undefined;
  let revision = 0;

  const selection = fileSelection(panel, renameFile, relativePath, flush);

  function renameFile(row: HTMLElement) {
    const path = row.dataset.file!;
    editFileName(row, fileName(path), async name => {
      await flush([path]);
      await change(path, { kind: "rename", name });
    });
  }
  function relativePath(path: string) {
    const root = owningRoot(path);
    return root ? path.slice(root.length).replace(/^\//, "") || "." : path;
  }
  async function add() {
    if (await invoke<boolean>("add_workspace_folder")) await refresh();
  }

  function fileRow(file: FileEntry) {
    const entry = button(file.name, async event => {
      await openNote(await openLibraryFile(file.path), event.detail === 1 ? "preview" : "keep");
    }, "library-node workspace-file");
    entry.title = file.path;
    entry.dataset.file = file.path;
    entry.setAttribute("aria-current", String(selected === file.path));
    entry.draggable = true;
    selection.sync(entry);
    return entry;
  }

  function directoryRow(file: FileEntry, root = false) {
    const branch = element("section");
    const line = element("div", "library-line");
    const name = root && roots.some(path => path !== file.path && fileName(path) === file.name)
      ? file.path : file.name;
    const entry = treeRow(name, expanded.has(file.path), async () => {
      if (expanded.has(file.path)) {
        expanded.delete(file.path);
        ++request;
        listed = undefined;
        children.replaceChildren();
        entry.setAttribute("aria-expanded", "false");
      } else {
        expanded.add(file.path);
        await read();
      }
    });
    entry.title = file.path;
    entry.dataset.directory = file.path;
    entry.draggable = true;
    line.append(entry);
    if (root) {
      entry.dataset.root = "true";
    }
    const children = element("div", "tree-children");
    branch.append(line, children);
    let request = 0;
    let listed: FileEntry[] | undefined;
    async function read() {
      const current = ++request;
      entry.setAttribute("aria-expanded", "true");
      let files: FileEntry[];
      try {
        files = await invoke<FileEntry[]>("workspace_children", { path: file.path });
      } catch (error) {
        if (request === current) {
          expanded.delete(file.path);
          entry.setAttribute("aria-expanded", "false");
          listed = undefined;
          children.replaceChildren();
          report(error);
        }
        return;
      }
      if (request !== current || !panel.contains(branch)) return;
      if (listed?.length === files.length && listed.every((previous, index) =>
        previous.path === files[index].path && previous.directory === files[index].directory)) return;
      if (afterNaming(children, () => void read().catch(report))) return;
      listed = files;
      children.replaceChildren();
      if (!files.length) children.append(element("p", "library-empty muted", localized("没有 Markdown 文件或子目录")));
      const pageSize = 80;
      const active = selected;
      if (active && files.length > pageSize) {
        // Keep the open document visible without building all preceding pages.
        const index = files.findIndex(child => child.path === active || (child.directory && isDescendant(active, child.path)));
        if (index >= pageSize) {
          files = files.slice();
          files.splice(pageSize - 1, 0, files.splice(index, 1)[0]);
        }
      }
      let offset = 0;
      const next = button(localized("显示更多"), appendPage, "library-node");
      function appendPage() {
        next.remove();
        const page = files.slice(offset, offset + pageSize);
        offset += page.length;
        const nodes = page.map(child => child.directory ? directoryRow(child) : fileRow(child));
        children.append(...nodes);
        if (offset < files.length) children.append(next);
      }
      appendPage();
    }
    readers.set(entry, read);
    if (expanded.has(file.path)) void read();
    return branch;
  }

  type Action = { kind: "createDirectory" | "rename"; name: string }
    | { kind: "move"; directory: string } | { kind: "trash" | "duplicate" };

  function owningRoot(path: string) {
    return roots.filter(root => path === root || isDescendant(path, root))
      .sort((a, b) => b.length - a.length)[0];
  }

  async function change(path: string, action: Action) {
    const result = await invoke<{path: string | null}>("workspace_action", { path, action });
    if ((action.kind === "rename" || action.kind === "move") && result.path) {
      for (const old of Array.from(expanded)) {
        if (old === path || isDescendant(old, path)) {
          expanded.delete(old);
          expanded.add(result.path + old.slice(path.length));
        }
      }
    }
    if (action.kind === "createDirectory") expanded.add(path);
    if (action.kind === "move") expanded.add(action.directory);
    await refresh();
    return result.path;
  }

  function createFile(row: HTMLElement) {
    const directory = row.dataset.directory!;
    return createInline(async () => {
      const { path } = await invoke<{ path: string }>("workspace_action", { path: directory, action: { kind: "createFile" } });
      selected = path;
      expanded.add(directory);
      const id = await openLibraryFile(path);
      await openNote(id);
      const folder = panel.querySelector<HTMLElement>(`[data-directory="${CSS.escape(directory)}"]`);
      if (folder) await readers.get(folder)?.();
      const created = panel.querySelector<HTMLElement>(`[data-file="${CSS.escape(path)}"]`);
      if (created) nameCreatedDocument(created, id);
    }).catch(report);
  }

  async function createDirectory(directory: string) {
    const name = await fileDialog(t("新建文件夹"), t("创建"), { initial: t("未命名文件夹") });
    if (name) await change(directory, { kind: "createDirectory", name });
  }

  panel.addEventListener("contextmenu", event => {
    const row = (event.target as Element).closest<HTMLElement>("[data-directory], [data-file]");
    if (!row) return;
    const path = row.dataset.directory ?? row.dataset.file!;
    const name = fileName(path) || path;
    const directory = !!row.dataset.directory;
    const menu = directory ? [
      { label: t("新建文档"), run: () => createFile(row) },
      { label: t("新建文件夹…"), run: () => createDirectory(path) },
      { label: t("图片管理…"), run: () => manageFolderImages(path, row) },
    ] : [
      { label: t("打开"), run: async () => openNote(await openLibraryFile(path)) },
      { label: t("在新窗口打开"), run: async () => invoke("open_document_window", { id: await openLibraryFile(path) }) },
      ...(desktopPlatform.hostWindows ? [{ label: t("挂载到窗口…"), run: async () => attachOnSwitch(await openLibraryFile(path)) }] : []),
    ];
    popupMenu(event, [
      ...menu, { label: t("在此处新建终端"), run: () => newTerminal({kind:"workspace", path}) }, null,
      { label: desktopPlatform.reveal, run: () => invoke("reveal_file", { path }) },
      { label: t("复制绝对路径"), run: () => copyText(displayPath(path)) },
      { label: t("复制相对路径"), run: () => copyText(relativePath(path)) }, null,
      { label: t("重命名…"), run: async () => {
        if (!directory) { renameFile(row); return; }
        const next = await fileDialog(t("重命名"), t("保存"), { initial: name });
        if (next && next !== name) await change(path, { kind: "rename", name: next });
      } },
      ...(!row.dataset.root ? [{ label: t("制作副本"), run: () => change(path, { kind: "duplicate" }) }] : []),
      ...(row.dataset.root ? [{ label: t("从工作区移除"), run: async () => {
        await invoke("remove_workspace_folder", { path });
        await refresh();
      } }] : []),
      { label: t("移到{trash}…", { trash: desktopPlatform.trash }), run: async () => {
        if (await fileDialog(t("将“{name}”移到{trash}？", { name, trash: desktopPlatform.trash }), t("移到{trash}", { trash: desktopPlatform.trash }), {
          detail: directory ? t("文件夹及其中所有内容都会移到{trash}。", { trash: desktopPlatform.trash }) : undefined }) !== undefined) await change(path, { kind: "trash" });
      } },
    ]);
  });

  const filesType = "application/x-jot-selected-files";
  const directoryType = "application/x-jot-directory";
  panel.addEventListener("dragstart", event => {
    const row = (event.target as Element).closest<HTMLElement>("[data-file]");
    if (row && event.dataTransfer) {
      const files = selection.paths(row);
      if (files.length > 1) {
        event.stopPropagation();
        event.dataTransfer.setData(filesType, JSON.stringify(files));
        event.dataTransfer.effectAllowed = "move";
        return;
      }
    }
    const directory = (event.target as Element).closest<HTMLElement>("[data-directory]")?.dataset.directory;
    if (directory && event.dataTransfer) {
      event.dataTransfer.setData(directoryType, directory);
      event.dataTransfer.effectAllowed = "move";
    }
  });
  let highlight: HTMLElement | undefined;
  function clearHighlight() {
    highlight?.classList.remove("drop-target");
    highlight = undefined;
  }
  function dropDirectory(event: DragEvent) {
    const transfer = event.dataTransfer;
    if (!transfer || !transfer.types.some(type => type === filesType || type === directoryType || type === workspaceFileType)) return;
    return (event.target as Element).closest<HTMLElement>("[data-directory]");
  }
  panel.addEventListener("dragover", event => {
    const target = dropDirectory(event);
    if (!target) { clearHighlight(); return; }
    event.preventDefault();
    event.dataTransfer!.dropEffect = "move";
    if (highlight !== target) {
      clearHighlight();
      highlight = target;
      target.classList.add("drop-target");
    }
  });
  panel.addEventListener("dragleave", event => {
    if (!(event.relatedTarget instanceof Node) || !highlight?.contains(event.relatedTarget)) clearHighlight();
  });
  panel.addEventListener("dragend", clearHighlight);
  panel.addEventListener("drop", event => {
    clearHighlight();
    const directory = dropDirectory(event)?.dataset.directory;
    if (!directory) return;
    event.preventDefault();
    const group = event.dataTransfer!.getData(filesType);
    if (group) {
      void selection.move(JSON.parse(group), directory).then(async () => {
        expanded.add(directory);
        await refresh();
      }).catch(report);
      return;
    }
    const path = event.dataTransfer!.getData(directoryType) || event.dataTransfer!.getData(workspaceFileType);
    if (path && path !== directory) void change(path, { kind: "move", directory }).catch(report);
  });
  void listen("desktop:files-changed", () => {
    if (panel.isConnected && !panel.closest("[hidden]")) void refresh(true).catch(report);
  }).catch(report);

  async function refresh(preserveRows = false) {
    const current = ++revision;
    const paths = await invoke<string[]>("workspace_roots");
    if (current !== revision) return;
    if (preserveRows && panel.childElementCount && paths.length === roots.length && paths.every((path, i) => path === roots[i])) {
      // Refocusing while the mouse is down must preserve WebKit's native drag source.
      const visible = panel.querySelectorAll<HTMLElement>('[data-directory][aria-expanded="true"]');
      await Promise.all(Array.from(visible, row => readers.get(row)!()));
      return;
    }
    if (afterNaming(panel, () => void refresh().catch(report))) return;
    roots = paths;
    panel.replaceChildren();
    if (!roots.length) {
      panel.append(button(localized("添加工作区文件夹…"), add, "library-node"));
      return;
    }
    for (const path of roots) {
      panel.append(directoryRow({ name: fileName(path) || path, path, directory: true }, true));
    }
  }

  return {
    panel, refresh, add, createFile, clearSelection: selection.clear,
    setActive(path?: string) {
      if (path !== selected) selection.follow(path);
      selected = path;
      for (const row of panel.querySelectorAll<HTMLElement>("[data-file]")) {
        row.setAttribute("aria-current", String(row.dataset.file === path));
      }
    },
  };
}
