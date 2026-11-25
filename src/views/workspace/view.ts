import { t, bindLabel, localized } from "../../i18n";
import { titlebar as configureTitlebar } from "../../ui/titlebar";
import { desktopPlatform } from "../../desktop/platform";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { events, notes, sources, type Card, type NoteId } from "../../desktop/ipc";
import { bindWindowShortcuts, shortcutHint } from "../../desktop/shortcuts";
import { watchUpdates } from "../../desktop/updates";
import { saveBeforeClose } from "../../desktop/window-actions";
import { NoteEditor } from "../../editor/note-editor";
import { bindWindowMenu, icon, iconButton } from "../../ui/chrome";
import { button, element, report, root } from "../../ui/dom";
import { documentStatus } from "../../ui/document-status";
import { noteList } from "../../ui/note-list";
import { currentAppearance, onAppearanceChange } from "../../appearance/preferences";
import { documentTabs, type OpenMode } from "./document-tabs";
import { documentDrag } from "./document-drag";
import { documentPath } from "./document-file-menu";
import { folderEditor, moveDocument } from "./folder-dialog";
import { librarySidebar } from "./library-sidebar";
import type { TerminalLocation } from "./terminal";
import { fileDialog } from "../../ui/file-dialog";
import { workspaceDividers } from "./pane-divider";
import { windowPicker } from "./source-dialog";
import "./workspace.css";

export async function mount() {
  root.className = "main-view jot-workspace";
  const titlebar = element("header", "workspace-titlebar jot-titlebar");
  const workspace = element("div", "workspace");
  const documentPane = element("main", "document-pane");
  const tabs = documentTabs(open, closeTabs, saveOpenNote, openTerminal);
  onAppearanceChange(({ editing }) => { if (editing.previewTabs === false) tabs.pin(); });
  const editorRoot = element("div", "editor-root");
  editorRoot.id = "workspace-editor-panel";
  const outline = element("aside", "workspace-outline jot-document-outline");
  outline.id = "document-outline";
  outline.hidden = true;
  const outlineHeading = element("h2", "outline-heading", localized("文档大纲"));
  outlineHeading.id = "document-outline-heading";
  outline.setAttribute("aria-labelledby", outlineHeading.id);
  outline.append(outlineHeading);
  const empty = element("div", "document-empty");
  const startActions = element("div", "workspace-start-actions");
  startActions.append(button(localized("打开文档…"), async () => {
    const id = await invoke<NoteId | null>("choose_markdown_file");
    if (id !== null) await open(id);
  }), button(localized("新建文档"), create));
  empty.append(startActions);
  const { footer, metadata, counter, status } = documentStatus();
  const windowMenu = element("div", "jot-window-menu");
  windowMenu.id = "document-windows";
  windowMenu.popover = "auto";
  windowMenu.dataset.noDrag = "";
  windowMenu.setAttribute("role", "group");
  bindLabel(windowMenu, "aria-label", localized("当前文档的窗口操作"));
  const hostLabel = element("div", "workspace-host-label");
  editorRoot.hidden = footer.hidden = true;
  documentPane.append(tabs.bar, editorRoot, empty, footer);
  let editor: NoteEditor | undefined;
  let card: Card | undefined;
  let tail = Promise.resolve();
  let dirty = false;
  let cardRead = 0;
  let resumeMove = false;

  const editFolder = folderEditor(folder => {
    const resume = resumeMove;
    resumeMove = false;
    void library.loadFolders(folder.id).then(refreshCard).then(() => {
      if (resume && editor && card) return move(editor.id, card.folder, folder.id);
    }).catch(report);
  });
  const pickWindow = windowPicker();
  const addSource = () => pickWindow(t("添加来源窗口"), t("添加"), async choice => {
    const key = await sources.add(choice);
    void library.showSource(key).catch(report);
  });
  async function saveOpenNote(id: NoteId) {
    if (editor?.id === id) await editor.save();
  }
  const library = await librarySidebar(open, folder => {
    resumeMove = false;
    editFolder(folder);
  }, addSource, (id, folder) => move(id, folder), saveOpenNote, openTerminal);
  const move = moveDocument((result, folder) => {
    bindLabel(status, "text", () => result.warning ?? t("已移动"));
    void Promise.all([library.showFolder(folder), refreshCard()]).catch(report);
  }, () => {
    resumeMove = true;
    editFolder();
  });

  function toggleSidebar() {
    library.sidebar.hidden = !library.sidebar.hidden;
    workspace.classList.toggle("sidebar-hidden", library.sidebar.hidden);
    sidebar.setAttribute("aria-pressed", String(!library.sidebar.hidden));
    if (!library.sidebar.hidden) {
      void library.refresh().catch(report);
      library.refreshFiles();
    }
    void editor?.remeasure().catch(report);
  }

  async function toggleOutline() {
    if (!editor) return;
    const current = editor;
    outline.hidden = !outline.hidden;
    outlineButton.setAttribute("aria-expanded", String(!outline.hidden));
    if (outline.hidden) {
      current.hideOutline();
    } else {
      await current.showOutline(outline);
    }
    if (editor === current) await current.remeasure();
  }

  async function refreshCard() {
    const id = editor?.id;
    if (id === undefined) return;
    const ticket = ++cardRead;
    const incoming = await notes.card(id);
    if (editor?.id !== id || ticket !== cardRead) return;
    card = incoming;
    tabs.rename(id, card.title);
    editor.setMenuContext(true, card.attached, card.file ? "file" : card.folder === null ? "draft" : "archive");
    library.setActive(id, card.file ?? card.archivePath);
    filePath.update(card);
    const action = () => incoming.attached ? t("更换宿主窗口") : t("挂载到窗口");
    bindLabel(sourceButton, "text", () => action() + "…");
    bindLabel(sourceButton, "aria-label", action);
    hostLabel.hidden = !card.attached;
    hostLabel.textContent = card.attached ? card.source : "";
    hostLabel.title = hostLabel.textContent;
    if (incoming.version > editor.savedVersion) await editor.synchronize(incoming.version);
  }

  async function create() {
    await editor?.save();
    if (library.sidebar.hidden) toggleSidebar();
    await library.createInInbox();
  }

  async function newDocument() {
    if (!await library.createAtFocus()) await create();
  }

  async function attach() {
    const current = editor;
    if (!current) return;
    await current.save();
    if (current !== editor) return;
    if (current.savedVersion === 0) throw new Error(t("请先输入并保存正文"));
    pickWindow(card?.attached ? t("更换便签的宿主窗口") : t("选择便签的宿主窗口"), t("挂载"), async choice => {
      const key = await notes.attach(current.id, choice);
      void Promise.all([library.showSource(key), refreshCard()]).catch(report);
    });
  }

  function enqueue(action: () => Promise<void>) {
    tail = tail.catch(() => {}).then(action);
    return tail;
  }

  function open(id: NoteId, mode: OpenMode = "keep") {
    return enqueue(() => present(id, false, mode));
  }

  function openTabs(cards: Pick<Card, "id" | "title">[]) {
    return enqueue(async () => {
      for (const card of cards) {
        tabs.pin(card.id);
        tabs.add(card.id);
        tabs.rename(card.id, card.title);
      }
      if (cards.length) await present(cards[0].id);
    });
  }

  function previousDocument() {
    return enqueue(async () => {
      const id = tabs.previous();
      if (id !== undefined) await present(id);
    });
  }

  function closeActive() {
    if (editor) return closeTabs([editor.id]);
  }

  function closeTabs(ids: NoteId[]) {
    return enqueue(async () => {
      if (editor && ids.includes(editor.id)) {
        const remaining = tabs.previous(id => !ids.includes(id));
        await present(remaining, true);
      }
      for (const id of ids) tabs.remove(id);
    });
  }

  async function present(id?: NoteId, dismiss = false, mode: OpenMode = "select") {
    if (editor && editor.id === id) {
      if (mode === "keep") tabs.pin(id);
      await editor.synchronize();
      editor.focus();
      return;
    }
    if (editor) {
      const session = await editor.close(!dismiss);
      if (session) tabs.remember(editor.id, session);
      else tabs.remove(editor.id);
      tabs.deactivate();
    }
    editor = undefined;
    card = undefined;
    filePath.update();
    windowButton.disabled = outlineButton.disabled = true;
    windowMenu.hidePopover();
    editorRoot.replaceChildren();
    bindLabel(status, "text", localized("正在打开…"));
    editorRoot.hidden = footer.hidden = id === undefined;
    empty.hidden = id !== undefined;
    if (id === undefined) {
      outline.hidden = true;
      outlineButton.setAttribute("aria-expanded", "false");
      return;
    }
    const session = tabs.takeSession(id);
    try {
      editor = await NoteEditor.open(editorRoot, id, status, { counter, mode: session?.mode, history: session?.history, onEdit: () => tabs.pin(id) });
    } catch (error) {
      tabs.remove(id);
      editorRoot.hidden = footer.hidden = outline.hidden = true;
      outlineButton.setAttribute("aria-expanded", "false");
      empty.hidden = false;
      throw error;
    }
    tabs.select(id, currentAppearance().editing.previewTabs === false ? "keep" : mode);
    windowButton.disabled = outlineButton.disabled = false;
    if (!outline.hidden) void editor.showOutline(outline).catch(report);
    editor.focus(session);
    await refreshCard();
  }

  const sidebar = shortcutHint(iconButton("sidebar", localized("显示或隐藏侧栏"), localized("侧栏"), toggleSidebar), "sidebar");
  sidebar.setAttribute("aria-pressed", "true");
  const add = shortcutHint(iconButton("compose", localized("新建文档"), localized("新建文档"), create), "newDocument");
  const outlineButton = shortcutHint(iconButton("outline", localized("文档大纲"), localized("大纲"), toggleOutline), "outline");
  outlineButton.disabled = true;
  outlineButton.setAttribute("aria-controls", outline.id);
  outlineButton.setAttribute("aria-expanded", "false");
  const finder = noteList(async (id, card) => {
    if (!card.attached) return open(id, "preview");
    await editor?.save();
    await invoke("locate", { id });
  }, { onDismiss: () => editor?.focus() });
  finder.panel.classList.add("workspace-search", "jot-search");
  finder.panel.setAttribute("role", "search");
  bindLabel(finder.panel, "aria-label", localized("搜索便签和文档"));
  const searchField = element("label", "workspace-search-field");
  searchField.append(icon("search"), finder.input, shortcutHint(element("kbd", "workspace-search-key"), "search"));
  finder.panel.prepend(searchField);
  finder.panel.dataset.noDrag = "";
  const showOverview = () => invoke("show_find", { tiled: true });
  let terminal: Promise<ReturnType<typeof import("./terminal").terminalPanel>> | undefined;
  let mountedTerminal: Awaited<NonNullable<typeof terminal>> | undefined;
  function currentLocation(): TerminalLocation {
    const row = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    if (row && library.sidebar.contains(row)) {
      if (row.dataset.directory || row.dataset.file && row.closest(".workspace-files"))
        return {kind:"workspace", path:(row.dataset.directory ?? row.dataset.file)!};
      const group = row.closest<HTMLElement>("[data-archive-group]");
      if (group) return {kind:"archive", folder:Number(group.dataset.archiveGroup) || null};
    }
    return { kind: "document", id: Number(row?.closest<HTMLElement>('[role=tab][data-note]')?.dataset.note) || editor?.id || null };
  }
  function panel() {
    return terminal ??= import("./terminal").then(({terminalPanel}) => mountedTerminal = terminalPanel(documentPane, terminalButton, currentLocation, () => editor?.focus()));
  }
  async function openTerminal(location = currentLocation(), mode: "new" | "reveal" = "new") {
    if (location.kind === "document" && location.id) await saveOpenNote(location.id);
    await (await panel()).start(location, mode);
  }
  const terminalButton = iconButton("terminal", localized("显示或隐藏终端"), localized("终端"), async () => (await panel()).toggle());
  terminalButton.setAttribute("aria-expanded", "false");
  terminalButton.setAttribute("aria-controls", "workspace-terminal");
  const showSettings = () => invoke("show_settings");
  const windowButton = element("button", "icon-button");
  windowButton.type = "button";
  bindLabel(windowButton, "aria-label", localized("文档窗口操作"));
  bindLabel(windowButton, "data-tip", localized("窗口操作"));
  bindWindowMenu(windowButton, windowMenu);
  windowButton.disabled = true;
  windowButton.append(icon("window"));
  const sourceButton = button(localized("挂载到窗口…"), () => { windowMenu.hidePopover(); return attach(); }, "jot-window-menu-item");
  const filePath = documentPath({ current: () => editor, openCopy: open, move });
  const floatButton = button(localized("作为独立便签打开"), async () => {
    windowMenu.hidePopover();
    const current = editor;
    if (!current) return;
    await current.save();
    if (current !== editor) return;
    if (!current.savedVersion) throw new Error(t("请先输入速记的正文"));
    await invoke("float_note", { id: current.id, standalone: true });
  }, "jot-window-menu-item", localized("独立便签"));
  sourceButton.hidden = !desktopPlatform.hostWindows;
  windowMenu.append(hostLabel, sourceButton, floatButton);
  metadata.append(filePath.control);
  const leading = element("div", "workspace-actions workspace-leading");
  const trailing = element("div", "workspace-actions workspace-trailing");
  leading.append(sidebar);
  const update = button("", () => invoke("show_settings", { section: "updates" }));
  update.hidden = true;
  trailing.append(update, terminalButton);
  void watchUpdates(status => {
    update.hidden = status.kind !== "available";
    if (status.kind === "available") bindLabel(update, "text", () => t("更新到 {version}", { version: status.version }));
  }).catch(report);
  if (desktopPlatform.windowsChrome) {
    const documentTools = element("div", "document-tools");
    add.replaceChildren(icon("add"));
    documentTools.append(tabs.bar, add, outlineButton, windowButton);
    documentPane.prepend(documentTools);
    windowMenu.addEventListener("beforetoggle", () => {
      const rect = windowButton.getBoundingClientRect();
      windowMenu.style.top = (rect.bottom + 4) + "px";
      windowMenu.style.right = Math.max(0, innerWidth - rect.right) + "px";
    });
  } else {
    leading.append(add);
    trailing.append(outlineButton, windowButton, shortcutHint(iconButton("settings", localized("设置"), localized("设置"), showSettings), "settings"));
  }
  titlebar.append(leading, finder.panel, trailing, windowMenu);
  configureTitlebar(titlebar);
  workspace.append(library.sidebar, documentPane, outline);
  root.append(titlebar, workspace);
  workspaceDividers(workspace, library.sidebar, outline, () => { void editor?.remeasure().catch(report); });

  documentDrag(library.sidebar, saveOpenNote,
    apply => pickWindow(t("挂载到窗口"), t("挂载"), apply), library.showSource);

  async function focusFiles(workspace: boolean) {
    if (library.sidebar.hidden) toggleSidebar();
    await library.focusFiles(workspace);
  }
  bindWindowShortcuts({
    editor: () => editor?.focus(), workspace: () => focusFiles(true), archive: () => focusFiles(false),
    tile: () => editor?.command("tile"),
    previousDocument,
    get closeActive() { return editor && (tabs.multiple || desktopPlatform.windowsChrome || mountedTerminal?.hasSessions()) ? closeActive : undefined; },
    newDocument: newDocument,
    terminal: () => openTerminal(currentLocation(), "reveal"),
    save: () => editor?.save(),
    search: finder.focus,
    overview: showOverview,
    sidebar: toggleSidebar,
    settings: showSettings,
    quit: () => invoke("quit"),
  });
  await getCurrentWindow().listen<NoteId>(events.mainNote, event => { void open(event.payload).catch(report); });
  await getCurrentWindow().listen(events.sidebarToggle, toggleSidebar);
  await getCurrentWindow().listen("desktop:workspace-added", () => { void library.showWorkspace().catch(report); });
  await getCurrentWindow().listen<string>(events.editorAction, ({ payload }) => {
    const actions: Record<string, () => unknown> = {
      "close-tab": () => !document.querySelector("dialog[open]") && closeActive(),
      "previous-document": previousDocument, new: newDocument, save: () => editor?.save(), file: () => editor?.file(), find: finder.focus,
      overview: showOverview, outline: toggleOutline, move: async () => {
        if (!editor || !card || card.file) return;
        const active = card;
        await editor.save();
        await move(active.id, active.folder);
      },
      attach,
      locate: async () => {
        if (!editor || !card?.attached) return;
        await editor.save();
        await invoke("locate", { id: editor.id });
      },
    };
    void Promise.resolve().then(() => {
      const action = actions[payload];
      if (action) return action();
      editor?.command(payload);
    }).catch(report);
  });
  function refresh(sourcesChanged = true) {
    dirty = false;
    void Promise.all([library.refresh(sourcesChanged), refreshCard(), finder.refresh()]).catch(report);
  }
  await listen<NoteId | null>(events.notesChanged, ({ payload }) => {
    if (document.hasFocus()) refresh(payload === null);
    else {
      dirty = true;
      // A second window can be frontmost while the workspace is still visible beside it.
      if (editor && (payload === null || payload === editor.id)) void refreshCard().catch(report);
    }
  });
  let titleRefresh = 0;
  await listen("desktop:files-changed", () => {
    const ids = tabs.ids().filter(id => id !== editor?.id);
    if (!ids.length) return;
    const turn = ++titleRefresh;
    void invoke<Pick<Card, "id" | "title">[]>("read_tab_titles", { ids }).then(cards => {
      if (turn === titleRefresh) for (const card of cards) tabs.rename(card.id, card.title);
    }).catch(report);
  });
  await listen<NoteId[]>("desktop:notes-removed", ({ payload }) => {
    void enqueue(async () => {
      for (const id of payload) if (id !== editor?.id) tabs.remove(id);
      if (!editor || !payload.includes(editor.id)) return;
      await editor.removed();
      await present(tabs.previous(), true);
      await library.refresh();
    }).catch(report);
  });
  await listen(events.foldersChanged, () => {
    void library.loadFolders().then(refreshCard).catch(report);
  });
  window.addEventListener("focus", () => {
    if (dirty) refresh();
    library.refreshFiles();
  });
  await saveBeforeClose(async () => {
    if (terminal) await (await terminal).ready();
    if (await invoke<number>("live_terminals") > 0 && await fileDialog(t("结束终端并关闭窗口？"), t("结束并关闭"), {
      detail: t("关闭主窗口会结束所有终端会话及其前台任务。")
    }) === undefined) { await invoke("cancel_quit"); return; }
    await tail.catch(() => {});
    await editor?.close();
    editor = undefined;
    await invoke("close_editor_window");
  });
  const receiveDrops = async () => {
    const cards = await invoke<Pick<Card, "id" | "title">[]>("take_dropped_tabs");
    if (cards.length) await openTabs(cards);
  };
  await getCurrentWindow().listen("desktop:dropped-tabs", () => { void receiveDrops().catch(report); });
  const initial = await invoke<NoteId | null>("initial_note");
  if (initial !== null) await open(initial);
  await receiveDrops();
}
