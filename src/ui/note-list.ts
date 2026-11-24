import { primaryPressed, desktopPlatform } from "../desktop/platform";
import { notes, type Card, type NoteId, type Scope, type SearchPage } from "../desktop/ipc";
import { button, element, report } from "./dom";
import "./note-list.css";

export function noteList(onChoose: (id: NoteId, card: Card) => unknown, {
  source, host = null, onDismiss,
}: { source?: Extract<Scope, { kind: "source" }>; host?: number | null; onDismiss?: () => void } = {}) {
  const inline = !!onDismiss;
  const panel = element("div", source ? "note-list library-notes" : "note-list");
  const input = element("input");
  input.type = "search";
  input.placeholder = "搜索便签和文档";
  input.setAttribute("aria-label", input.placeholder);
  const list = element("div", "results");
  const browser = element("div", "note-browser");
  browser.hidden = inline;
  const pager = element("div", "note-pagination");
  pager.setAttribute("aria-label", "分页");
  const previous = button("上一页", () => flip(-1));
  const next = button("下一页", () => flip(1));
  pager.append(previous, next);
  pager.hidden = true;
  browser.append(list, pager);
  if (!source) {
    list.id = "jot-search-results";
    list.setAttribute("role", "listbox");
    list.setAttribute("aria-label", "搜索结果");
    input.setAttribute("role", "combobox");
    input.setAttribute("aria-controls", list.id);
    input.setAttribute("aria-autocomplete", "list");
    input.setAttribute("aria-expanded", String(!inline));
    panel.append(input);
  }
  panel.append(browser);

  let records: Card[] = [];
  let groups: SearchPage | undefined;
  let rows: HTMLButtonElement[] = [];
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

  function choose(card: Card) {
    if (inline) close();
    return onChoose(card.id, card);
  }

  function appendRow(parent: HTMLElement, card: Card) {
    const index = rows.length;
    const row = button("", () => runWhenReady(() => {
      const match = records.find(record => record.id === card.id);
      if (match) return choose(match);
    }), "note-row");
    row.dataset.note = String(card.id);
    if (card.archivePath) row.dataset.archivePath = card.archivePath;
    if (card.id === active) row.setAttribute("aria-current", "true");
    row.draggable = !!source || inline;
    row.append(element("strong", undefined, card.title));
    if (source) {
      row.title = card.title;
    } else {
      row.id = "jot-search-option-" + card.id;
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

  function render() {
    rows = [];
    list.replaceChildren();
    if (groups) {
      const domains: [string, Card[]][] = [
        ["当前窗口", groups.current], [host === null ? "已挂载便签" : "其他窗口便签", groups.attached], ["文档", groups.documents],
      ];
      for (const [label, cards] of domains) {
        if (!cards.length) continue;
        const group = element("div", "note-search-group");
        group.setAttribute("role", "group");
        group.setAttribute("aria-label", label);
        const heading = element("h3", "note-search-heading", label);
        const items = element("div", tiled ? "note-search-items note-grid" : "note-search-items");
        group.append(heading, items);
        for (const card of cards) appendRow(items, card);
        list.append(group);
      }
    } else {
      for (const card of records) appendRow(list, card);
    }
    if (!records.length) list.append(element("p", "muted", input.value ? "没有匹配的便签或文档" : "暂无文档"));
    select(selected);
  }

  function load(target = 0): Promise<void> {
    if (disposed || browser.hidden) return Promise.resolve();
    const query = input.value;
    if (pending?.query === query && pending.page === target) return pending.work;
    const ticket = ++serial;
    const work = (async () => {
      const result = source ? await notes.list(query, target * 40, source) : await notes.search(query, host, target);
      if (ticket !== serial || disposed) return;
      const items = "items" in result ? result.items : [...result.current, ...result.attached, ...result.documents];
      if (!items.length && target > 0) return load(0);
      const retained = target === page && displayedQuery === query ? records[selected]?.id : undefined;
      groups = "items" in result ? undefined : result;
      records = items;
      if (target !== page || query !== displayedQuery) list.scrollTop = 0;
      page = target;
      more = result.more;
      selected = Math.max(0, records.findIndex(card => card.id === retained));
      displayedQuery = query;
      previous.disabled = page === 0;
      next.disabled = !more;
      pager.hidden = previous.disabled && next.disabled;
      render();
    })().finally(() => { if (ticket === serial) pending = undefined; });
    pending = { page: target, query, work };
    return work;
  }

  function flip(direction: number) {
    return runWhenReady(async () => {
      if (direction > 0 ? !more : page === 0) return;
      const version = intent;
      await load(page + direction);
      if (disposed || browser.hidden || version !== intent) return;
      select(direction > 0 ? 0 : rows.length - 1, true);
      if (!source) input.focus();
    });
  }

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
    if (event.ctrlKey || event.altKey || event.metaKey || event.shiftKey || pager.contains(event.target as Node)) return;
    if (event.key === "Enter" && event.target === input) {
      event.preventDefault();
      void runWhenReady(() => { const card = records[selected]; if (card) return choose(card); });
    } else if (event.key === "PageDown" || event.key === "PageUp") {
      event.preventDefault();
      void flip(event.key === "PageDown" ? 1 : -1);
    } else if (["ArrowDown", "ArrowUp"].includes(event.key) ||
      (tiled && !input.value && ["ArrowLeft", "ArrowRight"].includes(event.key))) {
      event.preventDefault();
      void runWhenReady(() => {
        if (!rows.length) return;
        select(nextIndex(event.key), true);
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
    refresh: () => load(displayedQuery === input.value ? page : 0),
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
      render();
    },
    setActive(id: NoteId) {
      active = id;
      for (const row of rows) {
        if (row.dataset.note === String(id)) row.setAttribute("aria-current", "true");
        else row.removeAttribute("aria-current");
      }
    },
    dispose: () => { disposed = true; invalidate(); },
  };
}
