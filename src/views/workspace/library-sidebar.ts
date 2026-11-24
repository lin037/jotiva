import { t } from "../../i18n";
import { desktopPlatform, displayPath } from "../../desktop/platform";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { copyText, popupMenu } from "../../ui/context-menu";
import { noteActions } from "./document-file-menu";
import { attachOnSwitch } from "./document-drag";
import { folders, notes, openLibraryFile, type Folder, type NoteId } from "../../desktop/ipc";
import { iconButton } from "../../ui/chrome";
import { button, element, report } from "../../ui/dom";
import { editFileName, nameNewFile, fileName } from "../../ui/file-name";
import { fileSelection } from "./file-selection";
import { archiveFiles } from "./archive-files";
import { sourceTree, treeRow } from "./source-tree";
import { workspaceFiles } from "./workspace-files";

export async function librarySidebar(
  open: (id: NoteId) => unknown,
  editFolder: (folder?: Folder) => void,
  addSource: () => void,
  moveArchive: (id: NoteId, folder: number | null) => unknown,
  saveOpenNote: (id: NoteId) => Promise<void>,
) {
  const sidebar = element("aside", "library-sidebar jot-sidebar");
  sidebar.id = "workspace-library";
  sidebar.setAttribute("aria-label", t("文档与窗口"));
  let current: NoteId | undefined;
  let currentPath: string | undefined;
  async function flush(paths: string[]) {
    if (current !== undefined && currentPath && paths.includes(currentPath)) await saveOpenNote(current);
  }
  const files = workspaceFiles(open, flush);
  const archive = element("div", "archive-tree");
  const tree = element("div", "library-tree");
  tree.setAttribute("role", "region");
  tree.setAttribute("aria-label", t("文件树"));
  const source = sourceTree(open, addSource);
  const bar = element("div", "library-bar");
  const tabs = element("div", "library-tabs");
  tabs.setAttribute("role", "group");
  tabs.setAttribute("aria-label", t("文档视图"));
  const workspaceTab = button(t("工作区"), () => switchTo(true));
  const archiveTab = button(t("归档"), () => switchTo(false));
  const add = iconButton("folderAdd", t("添加工作区文件夹"), () => inWorkspace ? files.add() : editFolder());
  tabs.append(workspaceTab, archiveTab);
  bar.append(tabs, add);
  tree.append(files.panel, archive);
  function focusRow(row: HTMLElement) {
    row.focus({ preventScroll: true });
    row.scrollIntoView({ block: "nearest", inline: "nearest" });
  }
  tree.addEventListener("keydown", event => {
    const row = event.target;
    if (!(row instanceof HTMLButtonElement) || event.metaKey || event.ctrlKey || event.altKey || event.shiftKey) return;
    const expanded = row.getAttribute("aria-expanded");
    if (event.key === "ArrowRight" && expanded === "false" || event.key === "ArrowLeft" && expanded === "true") {
      event.preventDefault();
      row.click();
    } else if (event.key === "ArrowUp" || event.key === "ArrowDown") {
      const rows = [...tree.querySelectorAll<HTMLButtonElement>("button:enabled")].filter(button => button.checkVisibility());
      const next = rows[rows.indexOf(row) + (event.key === "ArrowUp" ? -1 : 1)];
      event.preventDefault();
      if (next) focusRow(next);
    }
  });
  sidebar.append(bar, tree);
  if (desktopPlatform.hostWindows) sidebar.append(source.panel);
  const [initialFolders, inbox] = await Promise.all([folders.list(), folders.inbox()]);
  let items = initialFolders;
  let inWorkspace = true;
  const expandedFolders = new Set<number | null>([null]);
  const selection = fileSelection(archive, renameFile, fileName, flush);
  const lists = new Map<number | null, ReturnType<typeof archiveFiles>>();

  function drawFolders() {
    if (inWorkspace) {
      lists.clear();
      archive.replaceChildren();
      return;
    }
    const groups = [{ id: null, name: t("其他"), description: "", path: inbox }, ...items];
    const ids = new Set(groups.map(folder => folder.id));
    for (const id of expandedFolders) if (!ids.has(id)) expandedFolders.delete(id);
    for (const id of lists.keys()) {
      if (!expandedFolders.has(id)) lists.delete(id);
    }
    const branches = groups.map(folder => {
      const expanded = expandedFolders.has(folder.id);
      const branch = element("section");
      branch.dataset.archiveGroup = String(folder.id ?? "");
      const line = element("div", "library-line");
      const row = treeRow(folder.name, expanded, () => {
        if (expanded) expandedFolders.delete(folder.id);
        else expandedFolders.add(folder.id);
        drawFolders();
        archive.querySelector<HTMLButtonElement>(`[data-archive-folder="${folder.id ?? ""}"]`)?.focus({ preventScroll: true });
      });
      row.title = [folder.name, folder.description, folder.path].filter(Boolean).join("\n");
      row.dataset.archiveFolder = folder.id === null ? "" : String(folder.id);
      line.append(row);
      branch.append(line);
      if (expanded) {
        let list = lists.get(folder.id);
        if (!list) {
          list = archiveFiles(folder.id, open, selection.sync);
          void list.refresh().catch(report);
          if (current !== undefined) list.setActive(current, currentPath);
          lists.set(folder.id, list);
        }
        branch.append(list.panel);
      }
      return branch;
    });
    archive.replaceChildren(...branches);
  }

  function createInArchive(row: HTMLElement) {
    const folder = row.dataset.archiveFolder ? Number(row.dataset.archiveFolder) : null;
    if (!expandedFolders.has(folder)) {
      expandedFolders.add(folder);
      drawFolders();
      row = archive.querySelector<HTMLElement>(`[data-archive-folder="${folder ?? ""}"]`)!;
    }
    void nameNewFile(name => invoke<NoteId>("create_archive_note", { folder, name }), async id => {
      await lists.get(folder)?.refresh();
      await open(id);
    }).catch(report);
  }
  function renameFile(row: HTMLElement) {
    const path = row.dataset.file!;
    editFileName(row, fileName(path), async name => {
      await flush([path]);
      const id = row.dataset.note ? Number(row.dataset.note) : await openLibraryFile(path);
      await notes.rename(id, name);
    });
  }
  // A document mount has one target. A multi-selection offers its supported operations in the menu.
  archive.addEventListener("dragstart", event => {
    const row = (event.target as Element).closest<HTMLElement>("[data-file]");
    if (row && selection.paths(row).length > 1) { event.preventDefault(); event.stopPropagation(); }
  });

  archive.addEventListener("contextmenu", event => {
    const row = (event.target as Element).closest<HTMLElement>("[data-file], [data-archive-folder]");
    if (!row) return;
    const key = row.closest<HTMLElement>("[data-archive-group]")!.dataset.archiveGroup;
    const group = key ? items.find(item => item.id === Number(key)) : undefined;
    const directory = group?.path ?? inbox;
    if (!row.dataset.file) {
      void popupMenu(event, [
        { label: t("新建文档"), run: () => createInArchive(row) },
        ...(group ? [{ label: t("归档目录设置…"), run: () => editFolder(group) }] : []),
        { label: desktopPlatform.reveal, run: () => invoke("reveal_file", { path: directory }) },
        { label: t("复制绝对路径"), run: () => copyText(displayPath(directory)) },
      ]);
      return;
    }
    const path = row.dataset.file;
    const name = fileName(path);
    const resolveId = () => row.dataset.note ? Number(row.dataset.note) : openLibraryFile(path);
    const actions = noteActions(resolveId, name, open, saveOpenNote);
    void popupMenu(event, [
      { label: t("打开"), run: async () => open(await resolveId()) },
      { label: t("在新窗口打开"), run: async () => invoke("open_document_window", { id: await resolveId() }) },
      ...(desktopPlatform.hostWindows ? [{ label: t("挂载到窗口…"), run: async () => attachOnSwitch(await resolveId()) }] : []), null,
      { label: t("重命名…"), run: () => renameFile(row) },
      ...(row.dataset.archivePath ? [{ label: t("移到归档目录…"), run: async () => moveArchive(await resolveId(), group?.id ?? null) }] : []),
      actions.duplicate, null,
      { label: desktopPlatform.reveal, run: () => invoke("reveal_file", { path }) },
      { label: t("复制绝对路径"), run: () => copyText(displayPath(path)) },
      { label: t("复制相对路径"), run: () => copyText(name) }, null, actions.trash,
    ]);
  });

  async function switchTo(workspace: boolean) {
    if (workspace !== inWorkspace) { selection.clear(); files.clearSelection(); }
    inWorkspace = workspace;
    localStorage.setItem("jot-library-view", workspace ? "workspace" : "archive");
    files.panel.hidden = !workspace;
    archive.hidden = workspace;
    workspaceTab.setAttribute("aria-pressed", String(workspace));
    archiveTab.setAttribute("aria-pressed", String(!workspace));
    const label = workspace ? t("添加工作区文件夹") : t("导入归档目录");
    add.title = label;
    add.setAttribute("aria-label", label);
    add.dataset.tip = workspace ? t("添加目录") : t("导入目录");
    drawFolders();
    if (workspace) await files.refresh();
  }
  const refreshArchive = () => Promise.all(Array.from(lists.values(), list => list.refresh()));
  await listen("desktop:files-changed", () => {
    if (!inWorkspace && !sidebar.hidden) void refreshArchive().catch(report);
  });
  await switchTo(localStorage.getItem("jot-library-view") !== "archive");
  return {
    sidebar,
    setActive(id: NoteId, path?: string) {
      if (path !== currentPath) selection.follow(path);
      current = id;
      currentPath = path;
      files.setActive(path);
      for (const list of lists.values()) list.setActive(id, path);
      source.setActive(id);
    },
    async createInInbox() {
      expandedFolders.add(null);
      await switchTo(false);
      const row = archive.querySelector<HTMLElement>('[data-archive-folder=""]')!;
      row.scrollIntoView({ block: "nearest" });
      createInArchive(row);
    },
    async createAtFocus() {
      const row = document.activeElement instanceof HTMLElement ? document.activeElement : null;
      if (!row || !sidebar.contains(row)) return false;
      const directory = row.dataset.file
        ? row.closest("section")?.querySelector<HTMLElement>(":scope > .library-line > button") : row;
      if (directory?.dataset.directory) files.createFile(directory);
      else if (directory?.dataset.archiveFolder !== undefined) createInArchive(directory);
      else return false;
      return true;
    },
    async refresh(structure = true) {
      if (sidebar.hidden) return;
      await (structure ? source.refresh() : source.refreshDocuments());
    },
    refreshFiles() {
      if (sidebar.hidden) return;
      if (inWorkspace) return files.refresh(true).catch(report);
      return refreshArchive().catch(report);
    },
    async focusFiles(workspace: boolean) {
      if (inWorkspace !== workspace) await switchTo(workspace);
      const panel = workspace ? files.panel : archive;
      focusRow(panel.querySelector<HTMLElement>('[aria-current="true"], .selected') ??
        panel.querySelector<HTMLElement>('button:enabled') ?? (workspace ? workspaceTab : archiveTab));
    },
    showWorkspace: () => switchTo(true),
    async loadFolders(id?: number) {
      const next = await folders.list();
      for (const folder of next) {
        if (items.find(previous => previous.id === folder.id)?.path !== folder.path) lists.delete(folder.id);
      }
      items = next;
      if (id !== undefined) { expandedFolders.add(id); await switchTo(false); }
      else if (!inWorkspace) drawFolders();
    },
    showSource: (key: string) => source.refresh(key),
    async showFolder(id: number | null) { expandedFolders.add(id); await switchTo(false); },
  };
}
