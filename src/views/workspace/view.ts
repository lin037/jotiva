import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { events, notes, sources, type Card, type NoteId } from "../../desktop/ipc";
import { bindWindowShortcuts, shortcutHint } from "../../desktop/shortcuts";
import { saveBeforeClose } from "../../desktop/window-actions";
import { NoteEditor } from "../../editor/note-editor";
import { bindWindowMenu, dragOnMouseDown, icon, iconButton } from "../../ui/chrome";
import { button, element, report, root } from "../../ui/dom";
import { documentStatus } from "../../ui/document-status";
import { noteList } from "../../ui/note-list";
import { documentTabs } from "./document-tabs";
import { documentDrag } from "./document-drag";
import { documentPath } from "./document-file-menu";
import { folderEditor, moveDocument } from "./folder-dialog";
import { librarySidebar } from "./library-sidebar";
import { workspaceDividers } from "./pane-divider";
import { windowPicker } from "./source-dialog";
import "./workspace.css";

export async function mount() {
  root.className = "main-view jot-workspace";
  const titlebar = element("header", "workspace-titlebar jot-titlebar");
  dragOnMouseDown(titlebar);
  const workspace = element("div", "workspace");
  const documentPane = element("main", "document-pane");
  const tabs = documentTabs(open, closeTab);
  const editorRoot = element("div", "editor-root");
  editorRoot.id = "workspace-editor-panel";
  const outline = element("aside", "workspace-outline jot-document-outline");
  outline.id = "document-outline";
  outline.hidden = true;
  const outlineHeading = element("h2", "outline-heading", "文档大纲");
  outlineHeading.id = "document-outline-heading";
  outline.setAttribute("aria-labelledby", outlineHeading.id);
  outline.append(outlineHeading);
  const empty = element("div", "document-empty");
  const startActions = element("div", "workspace-start-actions");
  startActions.append(button("打开 Markdown…", async () => {
    const id = await invoke<NoteId | null>("choose_markdown_file");
    if (id !== null) await open(id);
  }), button("新建文档", create));
  empty.append(startActions);
  const { footer, metadata, counter, status } = documentStatus();
  const windowMenu = element("div", "jot-window-menu");
  windowMenu.id = "document-windows";
  windowMenu.popover = "auto";
  windowMenu.dataset.noDrag = "";
  windowMenu.setAttribute("role", "group");
  windowMenu.setAttribute("aria-label", "当前文档的窗口操作");
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
  const addSource = () => pickWindow("添加来源窗口", "添加", async choice => {
    const key = await sources.add(choice);
    void library.showSource(key).catch(report);
  });
  async function saveOpenNote(id: NoteId) {
    if (editor?.id === id) await editor.save();
  }
  const library = await librarySidebar(open, folder => {
    resumeMove = false;
    editFolder(folder);
  }, addSource, (id, folder) => move(id, folder), saveOpenNote);
  const move = moveDocument((result, folder) => {
    status.textContent = result.warning ?? "已移动";
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
    const action = card.attached ? "更换宿主窗口" : "挂载到窗口";
    sourceButton.textContent = action + "…";
    sourceButton.setAttribute("aria-label", action);
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
    if (current.savedVersion === 0) throw new Error("请先输入并保存正文");
    pickWindow(card?.attached ? "更换便签的宿主窗口" : "选择便签的宿主窗口", "挂载", async choice => {
      const key = await notes.attach(current.id, choice);
      void Promise.all([library.showSource(key), refreshCard()]).catch(report);
    });
  }

  function enqueue(action: () => Promise<void>) {
    tail = tail.catch(() => {}).then(action);
    return tail;
  }

  function open(id: NoteId) {
    return enqueue(() => present(id));
  }

  function openTabs(cards: Pick<Card, "id" | "title">[]) {
    return enqueue(async () => {
      for (const card of cards) {
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
    if (tabs.multiple && editor) return closeTab(editor.id);
  }

  function closeTab(id: NoteId) {
    return enqueue(async () => {
      if (editor?.id === id) await present(tabs.previous(), true);
      else tabs.remove(id);
    });
  }

  async function present(id?: NoteId, dismiss = false) {
    if (editor && editor.id === id) {
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
    status.textContent = "正在打开…";
    editorRoot.hidden = footer.hidden = id === undefined;
    empty.hidden = id !== undefined;
    if (id === undefined) {
      outline.hidden = true;
      outlineButton.setAttribute("aria-expanded", "false");
      return;
    }
    const session = tabs.takeSession(id);
    try {
      editor = await NoteEditor.open(editorRoot, id, status, { counter, mode: session?.mode, history: session?.history });
    } catch (error) {
      tabs.remove(id);
      editorRoot.hidden = footer.hidden = outline.hidden = true;
      outlineButton.setAttribute("aria-expanded", "false");
      empty.hidden = false;
      throw error;
    }
    tabs.select(id);
    windowButton.disabled = outlineButton.disabled = false;
    if (!outline.hidden) void editor.showOutline(outline).catch(report);
    editor.focus(session);
    await refreshCard();
  }

  const sidebar = shortcutHint(iconButton("sidebar", "显示或隐藏侧栏", toggleSidebar), "sidebar");
  sidebar.setAttribute("aria-pressed", "true");
  const add = shortcutHint(iconButton("compose", "新建文档", create), "newDocument");
  add.dataset.tip = "新建文档";
  const outlineButton = shortcutHint(iconButton("outline", "文档大纲", toggleOutline), "outline");
  outlineButton.disabled = true;
  outlineButton.setAttribute("aria-controls", outline.id);
  outlineButton.setAttribute("aria-expanded", "false");
  const finder = noteList(async (id, card) => {
    if (!card.attached) return open(id);
    await editor?.save();
    await invoke("locate", { id });
  }, { onDismiss: () => editor?.focus() });
  finder.panel.classList.add("workspace-search", "jot-search");
  finder.panel.setAttribute("role", "search");
  finder.panel.setAttribute("aria-label", "搜索便签和文档");
  const searchField = element("label", "workspace-search-field");
  searchField.append(icon("search"), finder.input, shortcutHint(element("kbd", "workspace-search-key"), "search"));
  finder.panel.prepend(searchField);
  finder.panel.dataset.noDrag = "";
  const showOverview = () => invoke("show_find", { tiled: true });
  const showSettings = () => invoke("show_settings");
  const settingsButton = shortcutHint(iconButton("settings", "设置", showSettings), "settings");
  const windowButton = element("button", "icon-button");
  windowButton.type = "button";
  windowButton.setAttribute("aria-label", "文档窗口操作");
  windowButton.dataset.tip = "窗口操作";
  bindWindowMenu(windowButton, windowMenu);
  windowButton.disabled = true;
  windowButton.append(icon("window"));
  const sourceButton = button("挂载到窗口…", () => { windowMenu.hidePopover(); return attach(); }, "jot-window-menu-item");
  const filePath = documentPath({ current: () => editor, openCopy: open, move });
  const floatButton = button("作为独立便签打开", async () => {
    windowMenu.hidePopover();
    const current = editor;
    if (!current) return;
    await current.save();
    if (current !== editor) return;
    if (!current.savedVersion) throw new Error("请先输入速记的正文");
    await invoke("float_note", { id: current.id, standalone: true });
  }, "jot-window-menu-item", "独立便签");
  windowMenu.append(hostLabel, sourceButton, floatButton);
  metadata.append(filePath.control);
  const leading = element("div", "workspace-actions workspace-leading");
  const trailing = element("div", "workspace-actions workspace-trailing");
  leading.append(sidebar, add);
  trailing.append(outlineButton, windowButton, settingsButton);
  titlebar.append(leading, finder.panel, trailing, windowMenu);
  workspace.append(library.sidebar, documentPane, outline);
  root.append(titlebar, workspace);
  workspaceDividers(workspace, library.sidebar, outline, () => { void editor?.remeasure().catch(report); });

  documentDrag(library.sidebar, saveOpenNote,
    apply => pickWindow("挂载到窗口", "挂载", apply), library.showSource);

  async function focusFiles(workspace: boolean) {
    if (library.sidebar.hidden) toggleSidebar();
    await library.focusFiles(workspace);
  }
  bindWindowShortcuts({
    editor: () => editor?.focus(), workspace: () => focusFiles(true), archive: () => focusFiles(false),
    tile: () => editor?.command("tile"),
    previousDocument,
    get closeActive() { return tabs.multiple ? closeActive : undefined; },
    newDocument: newDocument,
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
