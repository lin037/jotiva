import { t } from "../../i18n";
import { titlebar as configureTitlebar } from "../../ui/titlebar";
import { desktopPlatform } from "../../desktop/platform";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { events, notes, type Card, type EditorCursor, type NoteId, type NotePage, type QuickView } from "../../desktop/ipc";
import { bindWindowShortcuts, shortcutHint } from "../../desktop/shortcuts";
import { saveBeforeClose } from "../../desktop/window-actions";
import { NoteEditor, type TabSession } from "../../editor/note-editor";
import { bindWindowMenu, dragOnMouseDown, icon, iconButton } from "../../ui/chrome";
import { button, clearError, element, report, root } from "../../ui/dom";
import { editFileName, fileName } from "../../ui/file-name";
import { documentStatus } from "../../ui/document-status";
import { noteList } from "../../ui/note-list";
import { documentPath } from "../workspace/document-file-menu";
import { windowPicker } from "../workspace/source-dialog";
import "./quick.css";

const sourceLabel = (source: string) => source.replace(/ · (未命名窗口|未指定窗口)$/, "");

const documentName = (card: Card) => fileName(card.file ?? card.archivePath ?? card.title);

function foldedCard(view: Extract<QuickView, { kind: "summary" }>, expand: () => unknown) {
  const card = element("div", "quick-folded");
  const header = element("header", "quick-folded-bar");
  const rename = () => editFileName(title, title.textContent, name => notes.rename(view.note.id, name));
  const title = button("", rename, "quick-summary-title", t("改名"));
  const excerpt = element("p", "quick-summary-excerpt muted");
  const update = (note: Card) => {
    title.textContent = documentName(note);
    title.setAttribute("aria-label", t("重命名，") + title.textContent);
    excerpt.textContent = note.excerpt;
  };
  update(view.note);
  title.addEventListener("keydown", event => {
    if (event.key !== "F2") return;
    event.preventDefault();
    rename();
  });
  header.append(title, iconButton("expand", t("展开便签"), expand));
  if (desktopPlatform.customCaption) configureTitlebar(header, true);
  card.append(header);
  if (!view.docked) {
    card.append(excerpt);
    dragOnMouseDown(card);
    // Wait for motion on the filename so a stationary click can enter rename.
    let origin: { x: number; y: number } | undefined;
    let moved = false;
    title.addEventListener("pointerdown", event => {
      if (event.button !== 0) return;
      origin = { x: event.clientX, y: event.clientY };
      moved = false;
      title.setPointerCapture(event.pointerId);
    });
    title.addEventListener("pointermove", event => {
      if (!origin || !(event.buttons & 1) || Math.hypot(event.clientX - origin.x, event.clientY - origin.y) < 4) return;
      origin = undefined;
      moved = true;
      void invoke("drag_window").catch(report);
    });
    title.addEventListener("lostpointercapture", () => { origin = undefined; });
    title.addEventListener("click", event => {
      if (moved && event.detail !== 0) event.stopImmediatePropagation();
    }, true);
    card.dataset.movable = "";
  }
  return { card, id: view.note.id, docked: view.docked, update };
}

export async function mount() {
  root.className = "quick-view jot-quick";
  const editPane = element("div", "quick-edit-pane");
  const otherPane = element("div", "quick-other-pane");
  editPane.hidden = true;
  root.append(editPane, otherPane);
  let latest = -1;
  let presented = -1;
  let tail = Promise.resolve();
  let editor: NoteEditor | undefined;
  let cursor: { id: NoteId; state: EditorCursor } | undefined;
  let foldedHistory: TabSession["history"];
  let search: ReturnType<typeof noteList> | undefined;
  let switchingEpoch: number | undefined;
  let foldedIdle = 0;
  let parked = false;
  let attached = false;
  let folded: ReturnType<typeof foldedCard> | undefined;

  async function releaseEditor(keepHistory = false) {
    if (!editor) return;
    window.clearTimeout(foldedIdle);
    foldedIdle = 0;
    cursor = { id: editor.id, state: editor.cursor() };
    foldedHistory = (await editor.close(keepHistory))?.history;
    editor = undefined;
    editPane.replaceChildren();
  }

  async function afterSave(action: () => unknown, save = true) {
    const epoch = presented;
    if (epoch !== latest || switchingEpoch === epoch) return;
    switchingEpoch = epoch;
    try {
      if (save) await editor?.save();
      if (epoch === latest) await action();
    } finally {
      if (switchingEpoch === epoch) switchingEpoch = undefined;
    }
  }

  function leaveView(action: () => unknown) {
    return afterSave(action, root.dataset.mode !== "search");
  }

  const collapse = () => leaveView(() => invoke("dismiss_quick"));
  const create = () => afterSave(() => invoke("new_quick", { epoch: presented }));
  const find = (tiled = false) => leaveView(() => {
    if (!editPane.hidden) editor?.rememberViewport();
    return invoke("show_find", { tiled });
  });

  function titleBar() {
    const header = element("header", "quick-titlebar jot-titlebar");
    configureTitlebar(header, true);
    return header;
  }

  let pickWindow: ReturnType<typeof windowPicker> | undefined;
  function moreActions(id: NoteId) {
    const menu = element("div", "quick-menu jot-window-menu");
    menu.id = "quick-actions";
    menu.popover = "auto";
    menu.dataset.noDrag = "";
    menu.setAttribute("role", "group");
    menu.setAttribute("aria-label", t("便签操作"));
    const toggle = element("button", "icon-button quick-action-toggle");
    toggle.type = "button";
    toggle.dataset.tip = t("更多");
    toggle.setAttribute("aria-label", t("便签操作"));
    toggle.append(icon("more"));
    bindWindowMenu(toggle, menu, () => editor?.focus());
    function entry(glyph: "add" | "format" | "search" | "open" | "source", label: string, action: () => unknown, key?: string) {
      const row = iconButton(glyph, label, () => {
        menu.hidePopover();
        return action();
      });
      row.classList.replace("icon-button", "quick-menu-item");
      row.classList.add("jot-window-menu-item");
      row.append(element("span", undefined, label));
      if (key) {
        shortcutHint(row, key);
        row.append(shortcutHint(element("kbd"), key));
      }
      return row;
    }
    const format = entry("format", t("编辑工具栏"), async () => {
      const visible = format.getAttribute("aria-pressed") !== "true";
      await editor?.setToolbarVisible(visible);
      format.setAttribute("aria-pressed", String(visible));
    });
    format.setAttribute("aria-pressed", "false");
    const recall = entry("source", t("解除挂载并收回"), () => afterSave(() => invoke("recall_note", { id })));
    recall.dataset.recall = "";
    const add = entry("add", t("在此窗口新建便签"), create, "newDocument");
    add.dataset.newNote = "";
    menu.append(add, entry("search", t("查找便签"), () => find(), "search"), element("hr"), format,
      entry("open", t("在主窗口编辑"), () => afterSave(() => invoke("show_main", { id }))), recall);
    return { toggle, menu };
  }

  function attachmentButton(id: NoteId) {
    const choose = () => afterSave(() => {
      pickWindow ??= windowPicker();
      pickWindow(t("挂载到窗口"), t("挂载"), async choice => { await notes.attach(id, choice); });
    });
    const handle = iconButton("window", t("拖动挂载或点选窗口"), choose);
    handle.dataset.tip = t("拖动挂载");
    handle.hidden = !desktopPlatform.hostWindows;
    handle.draggable = desktopPlatform.hostWindows;
    handle.addEventListener("dragstart", event => {
      if (!event.dataTransfer) return;
      event.dataTransfer.setData("application/x-jot-note", String(id));
      event.dataTransfer.effectAllowed = "copy";
      void invoke("observe_document_drag", { started: true }).catch(report);
    });
    handle.addEventListener("dragend", () => {
      void (async () => {
        try {
          const target = await invoke<{ choice: number | null }>("source_at_drop");
          if (target.choice !== null) {
            const choice = target.choice;
            await afterSave(() => notes.attach(id, choice));
          }
        } finally {
          await invoke("observe_document_drag", { started: false });
        }
      })().catch(report);
    });
    return handle;
  }

  function displayPin(pin: HTMLButtonElement, pinned: boolean) {
    pin.setAttribute("aria-pressed", String(pinned));
    pin.dataset.tip = pinned ? t("取消置顶") : t("跨窗置顶");
  }

  function updateEditChrome(view: Extract<QuickView, { kind: "edit" }>) {
    const pin = editPane.querySelector<HTMLButtonElement>("[data-pin]")!;
    displayPin(pin, view.pinned);
    const add = editPane.querySelector<HTMLButtonElement>("[data-new-note]")!;
    const label = view.standalone ? t("新建独立便签") : t("在此窗口新建便签");
    add.querySelector("span")!.textContent = label;
    add.setAttribute("aria-label", label);
    editPane.querySelector("[draggable=true]")?.setAttribute("aria-description", view.source ? sourceLabel(view.source) : t("独立便签"));
    editPane.querySelector<HTMLElement>("[data-recall]")!.hidden = !view.source;
  }

  function showEditPane() {
    root.dataset.mode = "edit";
    otherPane.replaceChildren();
    editPane.removeAttribute("data-preparing");
  }

  async function render(view: QuickView) {
    clearError();
    attached = view.kind === "edit" ? !!view.source : view.kind === "summary" && view.note.attached;
    if (view.kind === "summary" && (view.rail?.items.length ?? 0) <= 1 && folded?.id === view.note.id && folded.docked === view.docked) {
      presented = view.epoch;
      root.dataset.mode = "summary";
      folded.update(view.note);
      return;
    }
    if (view.kind === "hidden") {
      // AppKit hides the native window. Keep its last painted frame intact for a hot return.
      presented = view.epoch;
      parked = true;
      root.dataset.mode = "hidden";
      if (editor) { editor.rememberViewport(); editor.setMenuContext(false); await editor.save(); }
      if (latest === view.epoch) {
        const bookmark = editor ? { id: editor.id, state: editor.cursor() } : cursor;
        await invoke("quick_inactive", { epoch:view.epoch, bookmark:bookmark ?? null });
      }
      return;
    }
    const continuing = view.kind === "edit" && editor?.id === view.id;
    const returning = continuing && (editPane.hidden || parked);
    parked = false;
    const foldedHere = view.kind === "summary" && editor?.id === view.note.id;
    if (!continuing) pickWindow?.dismiss();
    if (editor && !continuing) {
      if (view.kind === "search" || foldedHere) {
        if (!editPane.hidden) {
          editor.rememberViewport();
          editor.setMenuContext(false);
        }
        if (view.kind === "search") await editor.save();
        else await editor.save();
      } else await releaseEditor();
    }
    if (view.epoch !== latest) return;
    if (view.kind !== "search") {
      search?.dispose();
      search = undefined;
    }
    presented = view.epoch;
    if (view.kind !== "edit") editPane.querySelector<HTMLElement>(".quick-menu:popover-open")?.hidePopover();
    // Keep the existing card or search results painted while the worker opens.
    // A hidden, measurable editor can prepare its layout before taking focus.
    const preparing = view.kind === "edit" && !continuing && otherPane.hasChildNodes();
    editPane.toggleAttribute("data-preparing", preparing);
    if (!preparing) {
      root.dataset.mode = view.kind;
      otherPane.replaceChildren();
    }
    root.toggleAttribute("data-rail", view.kind === "summary" && !!view.rail && view.rail.items.length > 1);
    folded = undefined;
    if (!editor) editPane.replaceChildren();
    editPane.hidden = view.kind !== "edit";
    if (continuing) {
      window.clearTimeout(foldedIdle);
      foldedIdle = 0;
      if (view.kind === "edit") updateEditChrome(view);
      // A file can change outside Jotiva while its editor is parked. Reconcile on
      // the document's save queue without delaying the first local keystroke.
      if (returning) void editor?.synchronize("disk").catch(report);
      editor?.focus();
      editor?.setMenuContext(true, !!view.source);
      return;
    }
    if (editor && !foldedIdle && foldedHere) {
      // A folded card, or a search opened from one, may stay visible indefinitely.
      // Drop its worker after the same grace period as the hidden native window.
      const parked = editor;
      foldedIdle = window.setTimeout(() => {
        tail = tail.catch(report).then(() => {
          if (foldedIdle && editor === parked && editPane.hidden) return releaseEditor(root.dataset.mode !== "hidden");
        });
        void tail.catch(report);
      }, 15_000);
    }
    if (view.kind === "summary") {
      if (view.rail && view.rail.items.length > 1) {
        const rail = element("div", "note-rail");
        const pager = element("div", "note-pagination rail-pagination");
        const current = view.note.id;
        let offset = 0;
        let count = view.rail.items.length;
        const previous = button(t("上一页"), () => load(Math.max(0, offset - 40)));
        const next = button(t("下一页"), () => load(offset + count));
        pager.append(previous, next);
        function draw(page: NotePage) {
          count = page.items.length;
          previous.disabled = offset === 0;
          next.disabled = !page.more;
          pager.hidden = previous.disabled && next.disabled;
          rail.replaceChildren(...page.items.map(card => {
            const row = button(card.title, () => invoke("locate", { id: card.id }), "note-rail-row");
            row.title = card.title;
            if (card.id === current) row.setAttribute("aria-current", "true");
            return row;
          }));
          rail.scrollTop = 0;
        }
        const host = view.rail.host;
        async function load(start: number) {
          const page = await notes.list("", start, { kind: "host", id: host });
          offset = start;
          draw(page);
        }
        draw(view.rail);
        const header = titleBar();
        header.append(element("h1", "quick-title", t("当前窗口的便签")),
          shortcutHint(iconButton("grid", t("平铺便签"), () => find(true)), "overview"));
        otherPane.append(header, rail, pager);
        return;
      }
      folded = foldedCard(view, () => afterSave(() => invoke("locate", { id: view.note.id })));
      otherPane.append(folded.card);
      return;
    }
    if (view.kind === "search") {
      const host = view.host;
      if (!search) {
        search = noteList(id => afterSave(() => invoke("locate", { id })), { host });
      } else search.setHost(host);
      search.setTiled(view.tiled);
      const switchLayout = iconButton(view.tiled ? "list" : "grid", view.tiled ? t("列表显示") : t("平铺显示"), () => find(!view.tiled));
      search.panel.querySelector("header")?.remove();
      const header = titleBar();
      const field = element("label", "quick-query");
      field.dataset.noDrag = "";
      field.append(icon("search"), search.input);
      header.append(field);
      header.append(switchLayout);
      search.panel.prepend(header);
      otherPane.append(search.panel);
      search.focus();
      return;
    }
    const header = titleBar();
    const title = element("h1", "quick-title");
    const { footer, metadata, counter, status } = documentStatus();
    status.classList.add("quick-status");
    const path = documentPath();
    metadata.append(path.control);
    const actions = moreActions(view.id);
    const pin = iconButton("pin", t("置顶便签"), async () => {
      const pinned = pin.getAttribute("aria-pressed") !== "true";
      pin.disabled = true;
      try {
        await invoke("pin_quick", { id: view.id, pinned });
        displayPin(pin, pinned);
      } finally {
        pin.disabled = false;
      }
    });
    pin.dataset.pin = "";
    displayPin(pin, view.pinned);
    actions.toggle.disabled = true;
    header.append(title, iconButton("collapse", t("折叠便签"), collapse, "Esc"), attachmentButton(view.id), pin, actions.toggle, actions.menu);
    const paper = element("main", "quick-editor");
    const editorRoot = element("div", "editor-root");
    paper.append(editorRoot);
    editPane.append(header, paper, footer);
    const position = cursor?.id === view.id ? cursor.state : view.cursor ?? undefined;
    editor = await NoteEditor.open(editorRoot, view.id, status, {
      compact: true, counter, onCard: card => {
        path.update(card);
        title.textContent = documentName(card);
        title.title = title.textContent;
      }, mode: position?.mode, history: cursor?.id === view.id ? foldedHistory : undefined,
    });
    foldedHistory = undefined;
    actions.toggle.disabled = false;
    if (view.epoch === latest) {
      showEditPane();
      updateEditChrome(view);
      editor.focus(position);
      editor.setMenuContext(true, !!view.source);
    }
  }

  function receive(view: QuickView) {
    if (view.epoch <= latest) return;
    latest = view.epoch;
    tail = tail.catch(() => {}).then(async () => {
      if (view.epoch !== latest) return;
      try {
        await render(view);
      } catch (error) {
        if (view.epoch !== latest) return;
        if (view.kind === "edit" && editPane.hasAttribute("data-preparing")) showEditPane();
        report(error);
      }
      if (view.kind !== "hidden" && view.epoch === latest) {
        await invoke("quick_presented", { epoch: view.epoch });
      }
    });
    void tail.catch(report);
  }

  const main = () => afterSave(() => invoke("show_main", { id: editor?.id ?? folded?.id }));
  const pin = () => editPane.querySelector<HTMLButtonElement>("[data-pin]")?.click();
  const returnHost = () => afterSave(() => invoke("return_to_host", { id: editor?.id ?? folded?.id }));
  bindWindowShortcuts({
    main, pin: () => { if (!editPane.hidden) pin(); }, get returnHost() { return attached ? returnHost : undefined; },
    newDocument: create,
    save: () => editor?.save(),
    search: () => find(),
    overview: () => find(true),
    settings: () => invoke("show_settings"),
    quit: () => invoke("quit"),
  }, collapse);
  await Promise.all([
    saveBeforeClose(async () => {
      await tail.catch(() => {});
      const epoch = presented >= 0 ? presented : (await invoke<QuickView>("quick_view")).epoch;
      await editor?.save();
      // Closing search restores a view inside this same window. It must be
      // focusable when the native close response delivers that view.
      if (root.dataset.mode === "search") root.inert = false;
      await invoke("close_quick", { epoch });
    }),
    listen<NoteId[]>("desktop:notes-removed", ({ payload }) => {
      if (!editor || !payload.includes(editor.id)) return;
      const removed = editor;
      tail = tail.catch(() => {}).then(() => removed.removed());
      void tail.catch(report);
    }),
    getCurrentWindow().listen<QuickView>(events.quickView, event => receive(event.payload)),
    listen(events.notesChanged, () => { void search?.refresh().catch(report); }),
    getCurrentWindow().listen<string>(events.editorAction, ({ payload }) => {
      const actions: Record<string, () => unknown> = {
        main, pin, "return-host": returnHost, new: create, save: () => editor?.save(), file: () => editor?.file(), find: () => find(), overview: () => find(true),
      };
      void Promise.resolve().then(() => {
        const action = actions[payload];
        if (action) return action();
        if (root.dataset.mode === "edit") editor?.command(payload);
      }).catch(report);
    }),
  ]);
  receive(await invoke<QuickView>("quick_view"));
}
