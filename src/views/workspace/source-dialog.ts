import { sources, type SourceChoice } from "../../desktop/ipc";
import { allowWindowAccess } from "../../desktop/window-access";
import { button, element, report } from "../../ui/dom";
import "../../styles/dialog.css";
import "./source-dialog.css";

type WindowPicker = ((title: string, action: string, choose: (id: number) => Promise<void>) => void) & { dismiss(): void };
let picker: WindowPicker | undefined;

export function windowPicker(): WindowPicker {
  if (picker) return picker;
  const dialog = element("dialog", "sheet jot-dialog window-picker");
  const heading = element("h2");
  heading.id = "window-picker-heading";
  dialog.setAttribute("aria-labelledby", heading.id);
  const search = element("input");
  search.type = "search";
  search.placeholder = "搜索应用或窗口";
  search.autocomplete = "off";
  search.setAttribute("role", "combobox");
  search.setAttribute("aria-label", search.placeholder);
  search.setAttribute("aria-autocomplete", "list");
  search.setAttribute("aria-expanded", "true");
  const list = element("div", "window-picker-list");
  list.id = "window-picker-list";
  list.setAttribute("role", "listbox");
  search.setAttribute("aria-controls", list.id);
  const notice = element("p", "form-message");
  notice.setAttribute("role", "status");
  const refresh = button("刷新", load);
  const more = button("显示更多", () => { visible += pageSize; draw(); search.focus(); });
  more.hidden = true;
  const cancel = button("取消", () => dialog.close());
  const actions = element("div", "sheet-actions");
  actions.append(refresh, cancel);
  dialog.append(heading, search, list, more, notice, actions);
  document.body.append(dialog);
  let request = 0;
  let loading = false;
  let submitting = false;
  let all: SourceChoice[] = [];
  let shown: SourceChoice[] = [];
  const pageSize = 60;
  let visible = pageSize;
  let active = -1;
  let choose: (id: number) => Promise<void>;
  let access: AbortController | undefined;

  function highlight(index: number, scroll = false) {
    list.children[active]?.setAttribute("aria-selected", "false");
    active = index;
    const row = list.children[index];
    if (row) {
      row.setAttribute("aria-selected", "true");
      search.setAttribute("aria-activedescendant", row.id);
      if (scroll) row.scrollIntoView({ block: "nearest" });
    } else search.removeAttribute("aria-activedescendant");
  }
  function draw() {
    list.replaceChildren(...shown.slice(0, visible).map(item => {
      const row = element("div", "window-picker-option");
      row.setAttribute("role", "option");
      row.setAttribute("aria-selected", "false");
      row.id = "window-choice-" + item.choice;
      row.dataset.choice = String(item.choice);
      row.append(element("span", "window-picker-title", item.title));
      if (item.title !== item.app) row.append(element("span", "window-picker-app", item.app));
      row.title = item.app === item.title ? item.title : item.app + " · " + item.title;
      return row;
    }));
    more.hidden = visible >= shown.length;
    highlight(active);
  }
  function filter() {
    const query = search.value.trim().toLocaleLowerCase();
    shown = all.filter(item => (item.app + " " + item.title).toLocaleLowerCase().includes(query));
    active = shown.length ? 0 : -1;
    visible = pageSize;
    draw();
    list.scrollTop = 0;
    if (!loading) notice.textContent = shown.length ? "" : all.length ? "没有匹配的窗口" : "打开目标窗口后刷新";
  }

  async function select(id: number) {
    if (submitting || loading) return;
    const ticket = request;
    submitting = search.disabled = refresh.disabled = cancel.disabled = true;
    notice.textContent = "";
    try {
      await choose(id);
      if (ticket === request && dialog.open) dialog.close();
    } catch (error) {
      if (ticket === request) notice.textContent = String(error);
    } finally {
      if (ticket === request) submitting = search.disabled = refresh.disabled = cancel.disabled = false;
    }
  }

  async function load() {
    const ticket = ++request;
    loading = refresh.disabled = true;
    all = []; filter();
    notice.textContent = "正在读取窗口…";
    try {
      const choices = await sources.choices();
      if (!dialog.open || request !== ticket) return;
      if (choices === null) {
        dialog.close();
        void showAfterAccess(heading.textContent!, list.getAttribute("aria-label")!, choose).catch(report);
        return;
      }
      const totals = new Map<string, number>();
      const numbered = new Map<string, number>();
      for (const item of choices) totals.set(item.app, (totals.get(item.app) ?? 0) + 1);
      all = choices.sort((a, b) => a.app.localeCompare(b.app) || a.title.localeCompare(b.title)).map(item => {
        if (item.title) return item;
        const n = (numbered.get(item.app) ?? 0) + 1;
        numbered.set(item.app, n);
        return { ...item, title: totals.get(item.app) === 1 ? item.app : "窗口 " + n };
      });
      loading = false;
      filter();
    } catch (error) {
      if (ticket !== request) return;
      notice.textContent = String(error);
    } finally {
      if (ticket === request) loading = refresh.disabled = false;
    }
  }
  search.addEventListener("input", filter);
  search.addEventListener("keydown", event => {
    if (event.isComposing || !shown.length) return;
    if (event.key === "Enter") {
      event.preventDefault();
      void select(shown[active].choice);
    } else if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      const next = Math.max(0, Math.min(shown.length - 1, active + (event.key === "ArrowDown" ? 1 : -1)));
      if (next === visible) { visible += pageSize; draw(); }
      highlight(next, true);
    }
  });
  list.addEventListener("click", event => {
    const row = (event.target as Element).closest<HTMLElement>("[data-choice]");
    if (row) void select(Number(row.dataset.choice));
  });
  dialog.addEventListener("cancel", event => { if (submitting) event.preventDefault(); });
  dialog.addEventListener("close", () => {
    if (dialog.open) return;
    ++request;
    all = []; shown = []; list.replaceChildren();
    void sources.clearChoices().catch(report);
  });
  async function showAfterAccess(title: string, label: string, onChoose: typeof choose) {
    access?.abort();
    const current = access = new AbortController();
    if (!await allowWindowAccess(title, current.signal) || current.signal.aborted) return;
    heading.textContent = title;
    list.setAttribute("aria-label", label);
    choose = onChoose;
    submitting = search.disabled = cancel.disabled = false;
    search.value = "";
    dialog.showModal();
    search.focus();
    void load().catch(report);
  }
  picker = Object.assign((title: string, action: string, onChoose: typeof choose) => {
    void showAfterAccess(title, action + "目标窗口", onChoose).catch(report);
  }, { dismiss: () => { access?.abort(); if (dialog.open) dialog.close(); } });
  return picker;
}
