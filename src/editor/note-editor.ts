import { t, bindLabel, localized } from "../i18n";
import { primaryPressed } from "../desktop/platform";
import type { MarkdownEditor, MarkdownEditorOptions } from "@jot-md/editor";
import { normalizeInsertedText } from "@jot-md/editor/adapter";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { Channel, invoke } from "@tauri-apps/api/core";
import { currentAppearance, onAppearanceChange } from "../appearance/preferences";
import { events, notes, type Card, type EditorCursor as Cursor, type Note, type NoteId } from "../desktop/ipc";
import { commands, matchesShortcut } from "../desktop/shortcuts";
import { button, report } from "../ui/dom";
import { fileDialog } from "../ui/file-dialog";
import { applyIncoming } from "./document-sync";
import { trackCharacterCount } from "./character-count";
import { trackListMarkers } from "./list-layout";
import { retainScrollEnd } from "./scroll-end";
import { bindEditorShortcuts } from "./shortcuts";
import "./editor.css";

type DocumentStorage = "draft" | "archive" | "file";

type DocumentId = MarkdownEditorOptions["session"]["documentId"];
export type TabSession = Cursor & { history: MarkdownEditorOptions["history"] };

function editorLayout(root: HTMLElement) {
  const metric = document.createElement("span");
  metric.className = "jot-editor-metrics";
  metric.setAttribute("aria-hidden", "true");
  const headings = [1, 2, 3, 4, 5, 6].map(level => {
    const heading = document.createElement("span");
    heading.style.fontSize = `calc(1em * var(--jot-editor-heading-ratio-${level}))`;
    metric.append(heading);
    return heading;
  });
  root.append(metric);
  const style = getComputedStyle(metric);
  const canvas = document.createElement("canvas").getContext("2d")!;
  canvas.font = `${style.fontSize} ${style.fontFamily}`;
  root.style.setProperty("--jot-editor-source-space", `${canvas.measureText(" ").width}px`);
  // Pass the painted heading metrics to the engine's virtual layout, including imported CSS.
  const headingLineHeights = headings.map(heading => Number.parseFloat(getComputedStyle(heading).height)) as
    [number, number, number, number, number, number];
  const layout = {
    fontFamily: style.fontFamily,
    fontSize: Number.parseFloat(style.fontSize),
    codeBlockWrap: currentAppearance().editing.wrapCode ? "wrap" as const : "scroll" as const,
    metrics: {
      tableToolbarHeight: Math.round(Number.parseFloat(style.height)),
      listIndent: Number.parseFloat(style.width),
      headingLineHeights,
    },
  };
  metric.remove();
  return layout;
}

const colors = {
  background: "var(--jot-color-bg)",
  text: "var(--jot-color-text)",
  muted: "var(--jot-color-muted)",
  link: "var(--jot-color-accent)",
  codeBackground: "var(--jot-color-surface)",
  quote: "var(--jot-color-text)",
  quoteBackground: "var(--jot-color-quote)",
  quoteBorder: "var(--jot-color-line-strong)",
  searchMatch: "var(--jot-color-match)",
  searchCurrent: "var(--jot-color-match-current)",
  selection: "var(--jot-color-selection)",
};

export class NoteEditor {
  #timer = 0;
  #keptCopy?: { id: NoteId; markdown: string };
  #tail = Promise.resolve();
  #closing = false;
  #afterComposition?: number;
  #removed = false;
  #unlisten?: UnlistenFn;
  #keys = new AbortController();
  #fileButton = button(localized("用 Jev 归档"), () => this.file(), "workspace-move");
  #filing?: AbortController;
  #openedFiling?: { version: number; ticket: number };
  #stopAppearance?: () => void;
  #modeWatcher?: MutationObserver;
  #disposeOutline?: () => void;
  #stopCount?: () => void;
  #menuActive = false;
  #menuAttached = false;
  #pausedViewport: {
    top: number;
    snapshot: ReturnType<MarkdownEditor["input"]["view"]["scrollSnapshot"]>;
  } | undefined;

  private constructor(
    readonly id: NoteId,
    readonly source: string,
    private version: number,
    private markdown: string,
    private readonly editor: MarkdownEditor,
    private readonly worker: Worker,
    private readonly status: HTMLElement,
    private storage: DocumentStorage,
    private readonly onCard?: (card: Card) => void,
  ) {}

  static async open(root: HTMLElement, id: NoteId, status: HTMLElement, {
    compact = false, counter, mode = "live-preview", history, onCard,
  }: {
    compact?: boolean;
    counter?: HTMLElement;
    mode?: MarkdownEditor["mode"];
    history?: MarkdownEditorOptions["history"];
    onCard?: (card: Card) => void;
  } = {}) {
    root.classList.add("jot-editor");
    root.dataset.jotContext = compact ? "quick" : "workspace";
    const worker = new Worker(new URL("./editor.worker.ts", import.meta.url), { type: "module" });
    let editor: MarkdownEditor | undefined;
    let session: NoteEditor | undefined;
    try {
      const [
        note,
        { MarkdownEditor, createHeadingFoldPlugin },
        { createToolbarPlugin },
        { createMermaidPlugin },
        { createLazyKaTeXRenderer },
      ] = await Promise.all([
        invoke<Note>("open_editor_note", { id }),
        import("@jot-md/editor/component"),
        import("@jot-md/editor/toolbar"),
        import("@jot-md/editor/mermaid"),
        import("@jot-md/editor/math/lazy"),
      ]);
      const renderMath = createLazyKaTeXRenderer(undefined, async () => {
        const [renderer] = await Promise.all([
          import("@jot-md/editor/math"),
          import("@jot-md/editor/math.css"),
        ]);
        return renderer;
      });
      const preferences = currentAppearance().editing;
      root.style.removeProperty("--jot-ordered-width");
      root.style.removeProperty("--jot-ordered-task-width");
      const initial = normalizeInsertedText(note.markdown);
      let layout = editorLayout(root);
      editor = new MarkdownEditor({
        root,
        mode,
        history,
        openShortcutSettings: () => { void invoke("show_settings", { section: "shortcuts" }).catch(report); },
        plugins: [
          createHeadingFoldPlugin({ enabled: preferences.foldHeadings }),
          createMermaidPlugin({ load: () => import("mermaid") }),
          compact ? createToolbarPlugin({
            enabled: false,
            groups: [
              ["history.undo", "history.redo"],
              ["toolbar.open-heading", "format.bold", "format.italic", "format.link"],
              ["toolbar.open-table", "toolbar.open-more"],
              ["search.open", "command.palette"],
            ],
          }) : createToolbarPlugin({ enabled: preferences.toolbar }),
        ],
        surface: { renderMath, table: { mode: "adaptive" } },
        theme: { colors, layout },
        session: {
          endpoint: worker,
          documentId: String(id) as DocumentId,
          generation: 1,
          source: initial,
          syntaxProfile: {
            id: "default",
            version: 1,
            enabledExtensions: [{ id: "commonmark-fence-language", version: "1.0.0" }],
          },
        },
        input: { onInputTransaction: () => session?.changed() },
        onError: report,
      });
      editor.commands.unregister("mode.preview");
      const refreshShortcuts = bindEditorShortcuts(editor);
      session = new NoteEditor(id, note.source, note.version, initial, editor, worker, status,
        note.file ? "file" : note.location?.folder != null ? "archive" : "draft", onCard);
      if (note.filingTicket !== undefined) session.#openedFiling = { version: note.version, ticket: note.filingTicket };
      session.#fileButton.hidden = session.storage !== "draft";
      status.before(session.#fileButton);
      const ready = editor;
      const refreshLayout = (stylesheetChanged = false) => {
        const next = editorLayout(root);
        if (Number.isFinite(next.fontSize) && next.metrics.tableToolbarHeight > 0
          && JSON.stringify(next) !== JSON.stringify(layout)) {
          layout = next;
          void ready.setLayoutStyle(layout).catch(report);
        } else if (stylesheetChanged) {
          void ready.invalidateLayout().catch(report);
        }
      };
      trackListMarkers(ready, root, refreshLayout);
      await editor.open();
      // WebKit chooses the native insertion caret before the worker projects the first '#'.
      // Use the heading metrics for that input transaction, on its empty line only.
      editor.input.view.contentDOM.addEventListener("beforeinput", event => {
        if (event.inputType !== "insertText" || event.isComposing || event.data !== "#" || ready.mode !== "live-preview") return;
        const selection = getSelection();
        const anchor = selection?.anchorNode;
        const row = (anchor instanceof Element ? anchor : anchor?.parentElement)?.closest<HTMLElement>(".cm-line");
        if (!selection?.isCollapsed || !row || !root.contains(row) || row.textContent ||
          !row.matches(":not([data-jot-live-node]), .jot-live-preview-blank-line:not(.jot-live-preview-native-block--block-quote,.jot-live-preview-source-fenced-code,.jot-live-preview-source-math-block,.jot-live-preview-list-item)")) return;
        // CodeMirror replaces the edited row during input. Keep its place marked through
        // that replacement until the live-preview projection supplies the final metrics.
        const previous = row.previousElementSibling as HTMLElement | null;
        const marker = previous ?? row.parentElement!;
        marker.dataset.beforeHeading = previous ? "next" : "first";
        row.getBoundingClientRect();
        requestAnimationFrame(() => {
          void ready.settled().finally(() => { delete marker.dataset.beforeHeading; }).catch(report);
        });
      }, { capture:true, signal:session.#keys.signal });
      retainScrollEnd(editor.input.view, session.#keys.signal);
      if (counter) session.#stopCount = trackCharacterCount(editor, counter);
      const opened = session;
      session.#modeWatcher = new MutationObserver(() => {
        if (opened.#menuActive) opened.publishMenu();
      });
      session.#modeWatcher.observe(root, { attributes: true, attributeFilter: ["data-jot-editor-mode"] });
      session.#stopAppearance = onAppearanceChange(({ editing }, stylesheetChanged) => {
        refreshLayout(stylesheetChanged);
        if (ready.isPluginEnabled("heading-fold") !== editing.foldHeadings) {
          void ready.setPluginEnabled("heading-fold", editing.foldHeadings).then(refreshShortcuts).catch(report);
        }
        if (!compact && ready.isPluginEnabled("toolbar") !== editing.toolbar) {
          void ready.setPluginEnabled("toolbar", editing.toolbar).catch(report);
        }
      });
      if (note.fileUnavailable) {
        session.showRecovery(t("原文件当前不可用，显示上次保存的内容"));
      } else if (localStorage.getItem("jot:recovered-note") === String(id)) {
        localStorage.removeItem("jot:recovered-note");
        bindLabel(status, "text", localized("原路径不可用，正在编辑「其他」中的副本"));
      } else {
        bindLabel(status, "text", () => note.version ? t("已保存") : t("新便签"));
      }
      root.addEventListener("focusin", event => {
        const button = event.target;
        if (button instanceof HTMLElement && button.hasAttribute("data-jot-toolbar-command")) {
          // The component moves arrow-key focus with preventScroll so the document stays put.
          button.scrollIntoView({ block: "nearest", inline: "nearest" });
        }
      }, { signal: session.#keys.signal });
      ready.input.view.contentDOM.addEventListener("compositionend", () => {
        const version = opened.#afterComposition;
        if (version !== undefined) {
          opened.#afterComposition = undefined;
          window.setTimeout(() => { void opened.synchronize(version).catch(report); }, 0);
        }
      }, { signal: session.#keys.signal });
      session.#unlisten = await listen<number | null>(events.notesChanged, ({ payload }) => {
        if (payload === null || payload === id) void opened.synchronize().catch(report);
      });
      // A save in another window can land between the initial read and listener setup.
      await session.synchronize();
      window.addEventListener("keydown", event => {
        if (!opened.#menuActive || event.defaultPrevented || event.isComposing || event.repeat
          || document.querySelector("dialog[open]")) return;
        const cycle = matchesShortcut(event, "mode.cycle");
        if (!cycle) {
          // The source editor also assigns Command-/ to commenting out text.
          // Releasing Jotiva's shortcut must not expose that destructive fallback.
          if (event.code !== "Slash" || !primaryPressed(event) || event.altKey || event.shiftKey
            || !(event.target instanceof Node) || !root.contains(event.target)
            || commands.some(command => matchesShortcut(event, command.id))) return;
        }
        event.preventDefault();
        event.stopPropagation();
        if (cycle) void opened.cycleMode().catch(report);
      }, { capture: true, signal: session.#keys.signal });
      return session;
    } catch (error) {
      session?.dispose();
      if (!session) { editor?.dispose(); worker.terminate(); }
      await notes.release(id);
      throw error;
    }
  }

  private changed() {
    if (this.#closing || this.#removed) return;
    this.#filing?.abort();
    this.#filing = undefined;
    this.#fileButton.disabled = false;
    if (this.#openedFiling) {
      void invoke("cancel_filing", { id: this.id, ...this.#openedFiling }).catch(report);
      this.#openedFiling = undefined;
    }
    if (this.status.textContent !== t("正在编辑")) bindLabel(this.status, "text", localized("正在编辑"));
    window.clearTimeout(this.#timer);
    this.#timer = window.setTimeout(() => { void this.save().catch(report); }, 650);
  }

  get savedVersion() {
    return this.version;
  }

  private async applyDisk(disk: Note) {
    if (this.#removed || disk.version <= this.version) return;
    const incoming = normalizeInsertedText(disk.markdown);
    if (!await applyIncoming(this.editor, this.markdown, incoming)) {
      this.#afterComposition = disk.version;
      return;
    }
    this.#filing?.abort();
    this.markdown = incoming;
    this.version = disk.version;
    if (this.editor.value === incoming) {
      bindLabel(this.status, "text", localized("已保存"));
    }
  }

  synchronize(knownVersion?: number | "disk") {
    if (this.#closing || this.#removed) return Promise.resolve();
    const synced = this.#tail.catch(() => {}).then(async () => {
      if (this.#closing || this.#removed) return;
      if (knownVersion === "disk") {
        const disk = await invoke<Note | null>("refresh_editor_note", { id: this.id, version: this.version });
        if (disk) await this.applyDisk(disk);
        return;
      }
      let version = knownVersion;
      if (version === undefined) {
        const card = await notes.card(this.id);
        this.setMenuContext(this.#menuActive, this.#menuAttached, card.file ? "file" : card.folder === null ? "draft" : "archive");
        this.onCard?.(card);
        version = card.version;
      }
      if (version > this.version) await this.applyDisk(await notes.read(this.id));
    });
    this.#tail = synced;
    return synced.catch(error => {
      this.showRecovery();
      throw error;
    });
  }

  save(): Promise<void> {
    window.clearTimeout(this.#timer);
    const save = this.#tail.catch(() => {}).then(async () => {
      await this.editor.input.settled();
      if (this.#removed || this.editor.confirmedValue === this.markdown) return;
      let markdown = this.editor.confirmedValue;
      try {
        this.version = await notes.save(this.id, this.version, markdown);
      } catch (error) {
        const base = this.version;
        // Only merge another Jotiva revision. A file changed directly on disk needs explicit recovery.
        if ((await notes.card(this.id)).version <= base) throw error;
        await this.applyDisk(await notes.read(this.id));
        if (base === this.version) throw error;
        await this.editor.input.settled();
        markdown = this.editor.confirmedValue;
        if (markdown === this.markdown) return;
        this.version = await notes.save(this.id, this.version, markdown);
      }
      this.markdown = markdown;
      bindLabel(this.status, "text", () => this.editor.value === markdown ? t("已保存") : t("正在编辑"));
    });
    this.#tail = save;
    return save.catch(error => {
      this.showRecovery();
      throw error;
    });
  }

  private showRecovery(message = t("保存失败")) {
    this.status.textContent = message;
    const recover = button(localized("另存编辑并重载"), () => {
      recover.disabled = true;
      const task = this.#tail.catch(() => {}).then(() => this.keepEdits());
      this.#tail = task;
      return task.finally(() => { recover.disabled = false; });
    });
    bindLabel(recover, "title", localized("把当前编辑保存在「其他」，然后重新读取文档"));
    this.status.append(recover);
  }

  private async preserveCopy(draft: string) {
    if (this.#keptCopy?.markdown !== draft) {
      const id = await notes.create();
      try {
        await notes.save(id, 0, draft);
      } catch (error) {
        await notes.release(id);
        throw error;
      }
      this.#keptCopy = { id, markdown: draft };
    }
    return this.#keptCopy.id;
  }

  async removed() {
    this.#removed = true;
    window.clearTimeout(this.#timer);
    this.editor.setReadOnly(true);
    await this.#tail.catch(() => {});
    await this.editor.input.settled();
    if (this.editor.value !== this.markdown) await this.preserveCopy(this.editor.value);
  }

  private async keepEdits() {
    this.editor.setReadOnly(true);
    try {
      await this.editor.input.settled();
      const draft = this.editor.confirmedValue;
      const copy = await this.preserveCopy(draft);
      let disk;
      try {
        disk = await notes.read(this.id);
      } catch {
        // The snapshot is durable. Leaving this editor can now succeed even on a disconnected volume.
        this.markdown = draft;
        localStorage.setItem("jot:recovered-note", String(copy));
        await invoke("show_main", { id: copy });
        return;
      }
      this.version = disk.version;
      this.markdown = normalizeInsertedText(disk.markdown);
      this.editor.setValue(this.markdown);
      await this.editor.input.settled();
      window.clearTimeout(this.#timer);
      bindLabel(this.status, "text", localized("编辑副本已保存在「其他」"));
    } finally {
      this.editor.setReadOnly(false);
      this.editor.focus();
    }
  }

  async file() {
    await this.save();
    if (this.#closing || this.#removed || this.#filing || this.storage !== "draft") return;
    if (!this.version || this.editor.value !== this.markdown) {
      bindLabel(this.status, "text", localized("请先输入并保存正文"));
      return;
    }
    const version = this.version;
    const request = new AbortController();
    this.#filing = request;
    this.#fileButton.disabled = true;
    bindLabel(this.status, "text", localized("正在查询归档目录…"));
    let ticket: number | undefined;
    const cancel = () => {
      if (ticket !== undefined) void invoke("cancel_filing", { id: this.id, version, ticket }).catch(report);
    };
    request.signal.addEventListener("abort", cancel, { once: true });
    const started = new Channel<number>(value => {
      ticket = value;
      if (request.signal.aborted) cancel();
    });
    type Reply = { kind: "setup" } | { kind: "message"; message: string } | { kind: "suggestion"; folder: string; ticket: number };
    try {
      const reply = await invoke<Reply | null>("file_note", { id: this.id, version, manual: true, started });
      if (request.signal.aborted || !reply) return;
      if (reply.kind === "setup") {
        this.status.replaceChildren(button(localized("打开 Jev 归档设置"), () => invoke("show_settings", { section: "filing" })));
      } else if (reply.kind === "message") {
        this.status.textContent = reply.message;
      } else {
        ticket = reply.ticket;
        bindLabel(this.status, "text", localized("等待确认归档目录"));
        const confirmed = await fileDialog(t("Jev 建议归入「{folder}」", { folder: reply.folder }), t("移到「{folder}」", { folder: reply.folder }), { signal: request.signal });
        if (request.signal.aborted) return;
        if (confirmed === undefined) {
          cancel();
          bindLabel(this.status, "text", localized("便签保留在「其他」"));
          return;
        }
        const message = await invoke<string>("confirm_filing", { id: this.id, version, ticket });
        if (!request.signal.aborted) this.status.textContent = message;
      }
    } catch (error) {
      if (!request.signal.aborted) this.status.textContent = String(error);
    } finally {
      if (this.#filing === request) {
        this.#filing = undefined;
        this.#fileButton.disabled = false;
      }
    }
  }

  cursor(): Cursor {
    const { state, scrollDOM } = this.editor.input.view;
    return {
      anchor: state.selection.main.anchor,
      head: state.selection.main.head,
      top: this.#pausedViewport?.top ?? scrollDOM.scrollTop,
      mode: this.editor.mode,
    };
  }

  rememberViewport() {
    const view = this.editor.input.view;
    this.#pausedViewport ??= { top: view.scrollDOM.scrollTop, snapshot: view.scrollSnapshot() };
  }

  focus(cursor?: Cursor) {
    const view = this.editor.input.view;
    if (cursor) {
      const length = view.state.doc.length;
      view.dispatch({
        selection: { anchor: Math.min(cursor.anchor, length), head: Math.min(cursor.head, length) },
      });
    }
    this.editor.focus();
    if (this.#pausedViewport) {
      view.dispatch({ effects: this.#pausedViewport.snapshot });
      this.#pausedViewport = undefined;
    } else if (cursor) {
      view.scrollDOM.scrollTop = cursor.top;
    }
  }

  async setToolbarVisible(visible: boolean) {
    await this.editor.setPluginEnabled("toolbar", visible);
    this.editor.focus();
  }

  async showOutline(container: HTMLElement) {
    const { mountOutline } = await import("@jot-md/editor/outline");
    if (this.#closing || container.hidden) return;
    this.#disposeOutline ??= mountOutline(container, this.editor, { onError: report }).dispose;
  }

  hideOutline() {
    this.#disposeOutline?.();
    this.#disposeOutline = undefined;
  }

  private async cycleMode() {
    const next = this.editor.mode === "source" ? "live-preview" : "source";
    await this.editor.setMode(next);
    this.editor.focus();
  }

  command(id: string) {
    if (id === "mode.cycle") { void this.cycleMode().catch(report); return true; }
    if (id === "tile") {
      void this.save().then(() => invoke("tile_document", { id: this.id })).catch(report);
      return true;
    }
    if (id === "open-window") {
      void this.save().then(() => invoke("open_document_window", { id: this.id })).catch(report);
      return true;
    }
    return this.editor.executeCommand(id, undefined, { preserveViewport: true });
  }

  setMenuContext(active: boolean, attached = false, storage = this.storage) {
    if (this.#menuActive === active && this.#menuAttached === attached && this.storage === storage) return;
    this.storage = storage;
    this.#fileButton.hidden = storage !== "draft";
    this.#menuActive = active;
    this.#menuAttached = attached;
    this.publishMenu();
  }

  private publishMenu() {
    void invoke("document_menu", {
      exists: this.#menuActive,
      storage: this.storage,
      mode: this.editor.mode,
      editable: this.#menuActive && this.editor.canEdit,
      attached: this.#menuActive && this.#menuAttached,
    }).catch(report);
  }

  remeasure() {
    return this.editor.invalidateLayout();
  }

  async close(keepTab = false): Promise<TabSession | undefined> {
    const input = this.editor.input.view;
    if (input.compositionStarted) {
      const committed = new Promise<void>(resolve => {
        input.contentDOM.addEventListener("compositionend", () => resolve(), { once: true });
      });
      input.contentDOM.blur();
      await committed;
    }
    this.#closing = true;
    let tab: TabSession | undefined;
    try {
      // A composition can send its last input just after compositionend.
      await this.editor.input.settled();
      this.editor.setReadOnly(true);
      await this.save();
    } catch (error) {
      this.#closing = false;
      if (!this.#removed) this.editor.setReadOnly(false);
      throw error;
    }
    if (keepTab && this.version) {
      tab = { ...this.cursor(), history: undefined };
      try {
        tab.history = this.editor.captureHistory();
      } catch (error) {
        // The Markdown is already saved. A failed undo snapshot must not block navigation.
        console.warn(t("未保留本次撤销记录"), error);
      }
    }
    this.dispose();
    await notes.release(this.id);
    return tab;
  }

  private dispose() {
    this.#filing?.abort();
    this.#fileButton.remove();
    this.#keys.abort();
    this.#unlisten?.();
    this.#stopAppearance?.();
    this.#stopCount?.();
    this.#modeWatcher?.disconnect();
    this.hideOutline();
    this.setMenuContext(false);
    this.editor.dispose();
    this.worker.terminate();
  }
}
