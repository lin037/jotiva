import { desktopPlatform, displayPath } from "../../desktop/platform";
import { invoke } from "@tauri-apps/api/core";
import { openLibraryFile, type NoteId } from "../../desktop/ipc";
import { listen } from "@tauri-apps/api/event";
import { popupMenu, copyText } from "../../ui/context-menu";
import { attachOnSwitch, workspaceFileType } from "./document-drag";
import { editFileName, fileName, isDescendant } from "../../ui/file-name";
import { fileSelection } from "./file-selection";
import { fileDialog } from "../../ui/file-dialog";
import { button, element, report } from "../../ui/dom";
import { treeRow } from "./source-tree";

interface FileEntry {
  name: string;
  path: string;
  directory: boolean;
}

export function workspaceFiles(openNote: (id: NoteId) => unknown, flush: (paths: string[]) => Promise<void>) {
  const panel = element("div", "workspace-files");
  panel.setAttribute("aria-label", "工作区文件");
  const expanded = new Set<string>();
  const readers = new WeakMap<HTMLElement, () => Promise<void>>();
  let roots: string[] = [];
  let selected: string | undefined;
  let revision = 0;

  const selection = fileSelection(panel, renameFile, relativePath, flush);

  function renameFile(row: HTMLElement) {
    const path = row.dataset.file!;
    editFileName(row, fileName(path), false, async name => {
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
    const entry = button(file.name, async () => {
      await openNote(await openLibraryFile(file.path));
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
      listed = files;
      children.replaceChildren();
      if (!files.length) children.append(element("p", "library-empty muted", "没有 Markdown 文件或子目录"));
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
      const next = button("显示更多", appendPage, "library-node");
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

  type Action = { kind: "createFile"; name?: string } | { kind: "createDirectory" | "rename"; name: string }
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
    if (action.kind === "createFile" || action.kind === "createDirectory") expanded.add(path);
    if (action.kind === "createFile" && result.path) selected = result.path;
    if (action.kind === "move") expanded.add(action.directory);
    await refresh();
    return result.path;
  }

  function createFile(row: HTMLElement) {
    const directory = row.dataset.directory!;
    editFileName(row.closest<HTMLElement>(".library-line")!, "未命名.md", true, async name => {
      const action: Action = name === "未命名.md" ? { kind: "createFile" } : { kind: "createFile", name };
      const path = await change(directory, action);
      if (path) await openNote(await openLibraryFile(path));
    });
  }

  async function createDirectory(directory: string) {
    const name = await fileDialog("新建文件夹", "创建", "未命名文件夹");
    if (name) await change(directory, { kind: "createDirectory", name });
  }

  panel.addEventListener("contextmenu", event => {
    const row = (event.target as Element).closest<HTMLElement>("[data-directory], [data-file]");
    if (!row) return;
    const path = row.dataset.directory ?? row.dataset.file!;
    const name = fileName(path) || path;
    const directory = !!row.dataset.directory;
    const menu = directory ? [
      { label: "新建文档", run: () => createFile(row) },
      { label: "新建文件夹…", run: () => createDirectory(path) },
    ] : [
      { label: "打开", run: async () => openNote(await openLibraryFile(path)) },
      { label: "在新窗口打开", run: async () => invoke("open_document_window", { id: await openLibraryFile(path) }) },
      ...(desktopPlatform.hostWindows ? [{ label: "挂载到窗口…", run: async () => attachOnSwitch(await openLibraryFile(path)) }] : []),
    ];
    popupMenu(event, [
      ...menu, null,
      { label: desktopPlatform.reveal, run: () => invoke("reveal_file", { path }) },
      { label: "复制绝对路径", run: () => copyText(displayPath(path)) },
      { label: "复制相对路径", run: () => copyText(relativePath(path)) }, null,
      { label: "重命名…", run: async () => {
        if (!directory) { renameFile(row); return; }
        const next = await fileDialog("重命名", "保存", name);
        if (next && next !== name) await change(path, { kind: "rename", name: next });
      } },
      ...(!row.dataset.root ? [{ label: "制作副本", run: () => change(path, { kind: "duplicate" }) }] : []),
      ...(row.dataset.root ? [{ label: "从工作区移除", run: async () => {
        await invoke("remove_workspace_folder", { path });
        await refresh();
      } }] : []),
      { label: `移到${desktopPlatform.trash}…`, run: async () => {
        if (await fileDialog(`将“${name}”移到${desktopPlatform.trash}？`, `移到${desktopPlatform.trash}`, undefined,
          directory ? `文件夹及其中所有内容都会移到${desktopPlatform.trash}。` : undefined) !== undefined) await change(path, { kind: "trash" });
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
    roots = paths;
    panel.replaceChildren();
    if (!roots.length) {
      panel.append(button("添加工作区文件夹…", add, "library-node"));
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
