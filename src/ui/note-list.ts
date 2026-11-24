import { t, bindLabel, localized } from "../i18n";
import { primaryPressed, desktopPlatform } from "../desktop/platform";
import { openLibraryFile, notes, type NoteId, type Scope, type SearchPage } from "../desktop/ipc";
import { button, element, report } from "./dom";
import { documentSearch, hitKey, type SearchHit } from "../desktop/document-search";
import "./note-list.css";

export function noteList(onChoose: (id: NoteId, card: SearchHit) => unknown, {
  source, host = null, onDismiss,
}: { source?: Extract<Scope, { kind: "source" }>; host?: number | null; onDismiss?: () => void } = {}) {
  const inline = !!onDismiss;
  const panel = element("div", source ? "note-list library-notes" : "note-list");
  const input = element("input");
  input.type = "search";
  bindLabel(input, "placeholder", localized("搜索便签和文档"));
  bindLabel(input, "aria-label", () => input.placeholder);
  const list = element("div", "results");
  const browser = element("div", "note-browser");
  browser.hidden = inline;
  browser.append(list);
  if (!source) {
    list.id = "jot-search-results";
    list.setAttribute("role", "listbox");
    bindLabel(list, "aria-label", localized("搜索结果"));
    input.setAttribute("role", "combobox");
    input.setAttribute("aria-controls", list.id);
    input.setAttribute("aria-autocomplete", "list");
    input.setAttribute("aria-expanded", String(!inline));
    panel.append(input);
  }
  panel.append(browser);

  let records: SearchHit[] = [];
  const loadedIds = new Set<NoteId | string>();
  const sections = new Map<keyof Omit<SearchPage, "more">, HTMLElement>();
  let rows: HTMLButtonElement[] = [];
  let search: ReturnType<typeof documentSearch> | undefined;
  let page = 0;
  let more = false;
  let selected = 0;
  let active: NoteId | undefined;
  let tiled = false;
  let displayedQuery: string | undefined;
  let timer = 0;
  let serial = 0;
  let intent = 0;
  let disposed = false;
  let pending: { query: string; page: number; work: Promise<void> } | undefined;
  let keyboard = Promise.resolve();

  function select(index: number, scroll = false) {
    rows[selected]?.classList.remove("selected");
    if (!source) rows[selected]?.setAttribute("aria-selected", "false");
    selected = index;
    const row = rows[index];
    if (!row) { input.removeAttribute("aria-activedescendant"); return; }
    row.classList.add("selected");
    if (!source) {
      row.setAttribute("aria-selected", "true");
      input.setAttribute("aria-activedescendant", row.id);
    }
    if (scroll) row.scrollIntoView({ block: "nearest" });
  }

  async function choose(card: SearchHit) {
    if (inline) close();
    return onChoose(card.id ?? await openLibraryFile(card.file!), card);
  }

  function appendRow(parent: HTMLElement, card: SearchHit) {
    const index = rows.length;
    const row = button("", () => runWhenReady(() => {
      const match = records.find(record => hitKey(record) === hitKey(card));
      if (match) return choose(match);
    }), "note-row");
    if (card.id === null) row.dataset.file = card.file;
    else row.dataset.note = String(card.id);
    if (card.archivePath) row.dataset.archivePath = card.archivePath;
    if (card.id === active) row.setAttribute("aria-current", "true");
    row.draggable = !!source || inline;
    row.append(element("strong", undefined, card.title));
    if (source) {
      row.title = card.title;
    } else {
      row.id = "jot-search-option-" + (card.id ?? "file-" + index);
      row.tabIndex = -1;
      row.setAttribute("role", "option");
      row.setAttribute("aria-selected", "false");
      const location = card.attached ? card.source : card.file ?? card.archivePath;
      if (location) row.append(element("small", "muted", location));
      if (card.excerpt && (tiled || input.value)) row.append(element("span", "muted", card.excerpt));
      if (tiled && index < 9) row.append(element("kbd", "note-key", `${desktopPlatform.primaryLabel}${index + 1}`));
    }
    parent.append(row);
    rows.push(row);
  }

  function appendPage(result: (SearchPage<SearchHit>) | { items: SearchHit[]; more: boolean }) {
    const append = (parent: HTMLElement, card: SearchHit) => {
      if (loadedIds.has(hitKey(card))) return;
      loadedIds.add(hitKey(card));
      records.push(card);
      appendRow(parent, card);
    };
    if ("items" in result) {
      for (const card of result.items) append(list, card);
      return;
    }
    const labels = { current: localized("当前窗口"), attached: () => host === null ? t("已挂载便签") : t("其他窗口便签"), documents: localized("文档") };
    for (const key of ["current", "attached", "documents"] as const) {
      if (!result[key].length) continue;
      let items = sections.get(key);
      if (!items) {
        const group = element("div", "note-search-group");
        group.setAttribute("role", "group");
        bindLabel(group, "aria-label", labels[key]);
        items = element("div", tiled ? "note-search-items note-grid" : "note-search-items");
        group.append(element("h3", "note-search-heading", labels[key]), items);
        sections.set(key, items);
        list.append(group);
      }
      for (const card of result[key]) append(items, card);
    }
  }

  function load(append = false): Promise<void> {
    if (disposed || browser.hidden) return Promise.resolve();
    const query = input.value;
    const target = append ? page + 1 : 0;
    if (pending?.query === query) {
      if (pending.page === target) return pending.work;
      if (append) {
        const version = intent;
        return pending.work.then(() => { if (version === intent && page < target) return load(true); });
      }
    }
    if (append && (!more || displayedQuery !== query)) return Promise.resolve();
    const ticket = ++serial;
    if (!append) search = undefined;
    const fetch = source
      ? (index: number) => notes.list(query, index * 40, source)
      : (search ??= documentSearch(query, host));
    const work = (async () => {
      // A background refresh must not discard the loaded extent or move keyboard focus.
      const last = !append && displayedQuery === query ? page : target;
      const batches = [];
      let index = target;
      for (;; index++) {
        const result = await fetch(index);
        if (disposed || ticket !== serial) return;
        batches.push(result);
        if (!result.more || index >= last) break;
      }
      const retained = displayedQuery === query && records[selected] ? hitKey(records[selected]) : undefined;
      const focused = list.contains(document.activeElement);
      const scroll = displayedQuery === query ? list.scrollTop : 0;
      if (!append) {
        list.replaceChildren();
        sections.clear();
        rows = [];
        records = [];
        loadedIds.clear();
      }
      for (const batch of batches) appendPage(batch);
      if (!records.length) list.append(element("p", "muted", () => query ? t("没有匹配的便签或文档") : t("暂无文档")));
      page = index;
      more = batches[batches.length - 1].more;
      displayedQuery = query;
      select(Math.max(0, records.findIndex(card => hitKey(card) === retained)));
      if (!append) {
        list.scrollTop = scroll;
        if (focused) rows[selected]?.focus({ preventScroll: true });
      }
      if (source && more) list.append(sentinel);
      else sentinel.remove();
      if (more && list.clientHeight && list.scrollHeight <= list.clientHeight) requestAnimationFrame(nearEnd);
    })().finally(() => { if (ticket === serial) pending = undefined; });
    pending = { page: target, query, work };
    return work;
  }

  function nearEnd() {
    if (!source && more && list.clientHeight && list.scrollTop + list.clientHeight >= list.scrollHeight - 24) {
      void load(true).catch(report);
    }
  }
  // Source lists scroll with their enclosing tree. Observe the end in that viewport.
  const sentinel = element("div");
  sentinel.setAttribute("aria-hidden", "true");
  const intersection = source && new IntersectionObserver(entries => {
    if (entries.some(entry => entry.isIntersecting)) void load(true).catch(report);
  });
  if (intersection) intersection.observe(sentinel);
  list.addEventListener("scroll", nearEnd, { passive: true });
  const size = new ResizeObserver(nearEnd);
  size.observe(list);

  // Keep a fast "type, ↓, Return" sequence in order, including before the debounced query returns.
  function runWhenReady(action: () => unknown) {
    const version = intent;
    keyboard = keyboard.then(async () => {
      if (disposed || browser.hidden || intent !== version) return;
      if (displayedQuery !== input.value) { clearTimeout(timer); await load(); }
      if (!disposed && !browser.hidden && intent === version && displayedQuery === input.value) await action();
    }).catch(report);
    return keyboard;
  }

  function invalidate() {
    ++intent;
    ++serial;
    pending = undefined;
    search = undefined;
    displayedQuery = undefined;
    clearTimeout(timer);
  }

  function close() {
    invalidate();
    browser.hidden = true;
    input.value = "";
    input.setAttribute("aria-expanded", "false");
    input.removeAttribute("aria-activedescendant");
  }

  function reveal() {
    if (!inline || !browser.hidden) return;
    browser.hidden = false;
    input.setAttribute("aria-expanded", "true");
    void load().catch(report);
  }

  if (inline) {
    input.addEventListener("focus", reveal);
    input.addEventListener("click", reveal);
    document.addEventListener("mousedown", event => {
      if (!browser.hidden && !panel.contains(event.target as Node)) close();
    });
  }

  function showKeys(visible: boolean) { panel.toggleAttribute("data-show-keys", tiled && visible); }
  input.addEventListener("input", () => {
    invalidate();
    timer = window.setTimeout(() => { void load().catch(report); }, 100);
  });

  function nextIndex(key: string) {
    const forward = key === "ArrowDown" || key === "ArrowRight";
    if (tiled && (key === "ArrowDown" || key === "ArrowUp")) {
      // Separate grids can have incomplete last lines. Navigate the actual adjacent visual row.
      const origin = rows[selected].getBoundingClientRect();
      let best = -1, bestY = Infinity, bestX = Infinity;
      rows.forEach((row, index) => {
        const rect = row.getBoundingClientRect();
        const dy = (rect.top - origin.top) * (forward ? 1 : -1);
        const dx = Math.abs(rect.left - origin.left);
        if (dy > 1 && (dy < bestY || (dy === bestY && dx < bestX))) { best = index; bestY = dy; bestX = dx; }
      });
      if (best !== -1) return best;
      return forward ? 0 : rows.length - 1;
    }
    return (selected + (forward ? 1 : -1) + rows.length) % rows.length;
  }

  panel.addEventListener("keydown", event => {
    if (event.defaultPrevented || event.isComposing || browser.hidden) return;
    if (inline && event.key === "Escape") {
      event.preventDefault();
      close();
      input.blur();
      onDismiss?.();
      return;
    }
    showKeys(primaryPressed(event));
    if (tiled && primaryPressed(event) && !event.altKey && !event.shiftKey) {
      const digit = /^Digit([1-9])$/u.exec(event.code);
      if (digit) {
        event.preventDefault();
        void runWhenReady(() => { const card = records[Number(digit[1]) - 1]; if (card) return choose(card); });
      }
      return;
    }
    if (event.ctrlKey || event.altKey || event.metaKey || event.shiftKey) return;
    if (event.key === "Enter" && event.target === input) {
      event.preventDefault();
      void runWhenReady(() => { const card = records[selected]; if (card) return choose(card); });
    } else if (["PageDown", "PageUp", "ArrowDown", "ArrowUp"].includes(event.key) ||
      (tiled && !input.value && ["ArrowLeft", "ArrowRight"].includes(event.key))) {
      event.preventDefault();
      const key = event.key;
      const version = intent;
      void runWhenReady(async () => {
        if (!rows.length) return;
        const forward = ["ArrowDown", "ArrowRight", "PageDown"].includes(key);
        const step = () => key.startsWith("Page")
          ? Math.max(0, Math.min(rows.length - 1, selected + (forward ? 1 : -1) *
            Math.max(1, Math.floor(list.clientHeight / (rows[selected].offsetHeight || 1)))))
          : nextIndex(key);
        if (more && forward && (key === "PageDown" ? step() === rows.length - 1 : step() <= selected)) await load(true);
        if (version !== intent || disposed || !rows.length) return;
        select(step(), true);
        if (event.target !== input) rows[selected].focus({ preventScroll: true });
      });
    }
  });
  panel.addEventListener("keyup", event => showKeys(primaryPressed(event)));
  panel.addEventListener("focusout", event => {
    if (!panel.contains(event.relatedTarget as Node | null)) {
      showKeys(false);
      if (inline && event.relatedTarget) close();
    }
  });
  void load().catch(report);
  return {
    panel, input,
    focus: () => { input.focus(); reveal(); },
    refresh: () => load(),
    setHost(next: number | null) {
      if (host === next) return;
      host = next;
      invalidate();
      void load().catch(report);
    },
    setTiled(value: boolean) {
      if (tiled === value) return;
      tiled = value;
      showKeys(false);
      for (const items of sections.values()) items.classList.toggle("note-grid", tiled);
      if (!browser.hidden) void load().catch(report);
    },
    setActive(id: NoteId) {
      active = id;
      for (const row of rows) {
        if (row.dataset.note === String(id)) row.setAttribute("aria-current", "true");
        else row.removeAttribute("aria-current");
      }
    },
    dispose: () => { disposed = true; size.disconnect(); intersection?.disconnect(); invalidate(); },
  };
}
