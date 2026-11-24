import { t, bindLabel, localized } from "../../i18n";
import { invoke } from "@tauri-apps/api/core";
import { desktopPlatform } from "../../desktop/platform";
import { notes, type NoteId } from "../../desktop/ipc";
import type { TabSession } from "../../editor/note-editor";
import { copyText, popupMenu } from "../../ui/context-menu";
import { noteActions } from "./document-file-menu";
import { button, element, report } from "../../ui/dom";

interface Tab {
  row: HTMLElement;
  title: HTMLButtonElement;
  close: HTMLButtonElement;
  session?: TabSession;
}

// The order on screen is independent of the order in which documents were last visited.
export function documentTabs(open: (id: NoteId) => Promise<void>, close: (ids: NoteId[]) => Promise<void>, save: (id: NoteId) => Promise<void>) {
  const bar = element("nav", "document-tabs");
  bar.setAttribute("role", "tablist");
  bindLabel(bar, "aria-label", localized("已打开的文档"));
  bar.hidden = true;
  const tabs = new Map<NoteId, Tab>();
  const recent = new Set<NoteId>();
  let active: NoteId | undefined;

  let multi = false;
  function syncMenu() {
    const multiple = tabs.size > 1;
    if (multiple === multi) return;
    multi = multiple;
    void invoke("workspace_tab_menu", { multiple }).catch(report);
  }
  function add(id: NoteId) {
    const existing = tabs.get(id);
    if (existing) return existing;
    const row = element("div", "document-tab");
    const title = button("", () => open(id), "document-tab-name");
    title.draggable = desktopPlatform.hostWindows;
    title.dataset.note = String(id);
    title.setAttribute("role", "tab");
    title.setAttribute("aria-controls", "workspace-editor-panel");
    const dismiss = button("×", () => close([id]), "document-tab-close", localized("关闭标签"));
    dismiss.tabIndex = -1;
    row.append(title, dismiss);
    bar.append(row);
    const tab = { row, title, close: dismiss };
    title.tabIndex = -1;
    title.setAttribute("aria-selected", "false");
    tabs.set(id, tab);
    syncMenu();
    row.addEventListener("contextmenu", event => {
      const ids = [...tabs.keys()];
      const index = ids.indexOf(id);
      const left = ids.slice(0, index), right = ids.slice(index + 1);
      const { duplicate } = noteActions(() => id, title.textContent, open, save);
      void popupMenu(event, [
        { label: t("复制文档内容"), run: async () => {
          await save(id);
          await copyText((await notes.read(id)).markdown);
        } },
        duplicate,
        { label: t("在新窗口打开"), run: async () => { await save(id); await invoke("open_document_window", { id }); } },
        null, { label: t("关闭标签"), run: () => close([id]) },
        ...(left.length ? [{ label: t("关闭左侧标签"), run: () => close(left) }] : []),
        ...(right.length ? [{ label: t("关闭右侧标签"), run: () => close(right) }] : []),
        ...(ids.length > 1 ? [{ label: t("关闭其他标签"), run: () => close([...left, ...right]) }] : []),
      ]);
    });
    title.addEventListener("keydown", event => {
      if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
      const ids = [...tabs.keys()];
      const index = ids.indexOf(id);
      const target = event.key === "ArrowLeft" ? ids[(index + ids.length - 1) % ids.length]
        : event.key === "ArrowRight" ? ids[(index + 1) % ids.length]
        : event.key === "Home" ? ids[0] : event.key === "End" ? ids[ids.length - 1] : undefined;
      if (target !== undefined) {
        event.preventDefault();
        void open(target).then(() => tabs.get(target)?.title.focus()).catch(report);
      } else if (event.key === "Delete" || event.key === "Backspace") {
        event.preventDefault();
        void close([id]).then(() => active !== undefined && !bar.hidden && tabs.get(active)?.title.focus()).catch(report);
      }
    });
    bar.hidden = tabs.size < 2;
    return tab;
  }

  function deactivate() {
    const previous = active === undefined ? undefined : tabs.get(active);
    if (previous) {
      previous.title.setAttribute("aria-selected", "false");
      previous.title.tabIndex = -1;
    }
    active = undefined;
  }

  function select(id: NoteId) {
    deactivate();
    const tab = add(id);
    active = id;
    recent.delete(id);
    recent.add(id);
    bar.hidden = tabs.size < 2;
    tab.title.tabIndex = 0;
    tab.title.setAttribute("aria-selected", "true");
    if (!bar.hidden) tab.row.scrollIntoView({ block: "nearest", inline: "nearest" });
  }

  function remove(id: NoteId) {
    tabs.get(id)?.row.remove();
    tabs.delete(id);
    syncMenu();
    recent.delete(id);
    if (active === id) active = undefined;
    bar.hidden = tabs.size === 0 || (tabs.size === 1 && active !== undefined);
  }

  return {
    bar, select, add, remove, deactivate,
    get multiple() { return multi; },
    ids: () => [...tabs.keys()],
    rename(id: NoteId, name: string) {
      const tab = tabs.get(id);
      if (!tab || tab.title.textContent === name) return;
      tab.title.textContent = tab.title.title = name;
      bindLabel(tab.close, "aria-label", () => t("关闭标签，") + name);
    },
    remember(id: NoteId, session: TabSession) {
      const tab = tabs.get(id);
      if (tab) tab.session = session;
    },
    takeSession(id: NoteId) {
      const tab = tabs.get(id);
      const session = tab?.session;
      if (tab) tab.session = undefined;
      return session;
    },
    previous: (keep: (id: NoteId) => boolean = () => true) =>
      [...recent].reverse().find(id => id !== active && keep(id)) ?? [...tabs.keys()].find(id => id !== active && keep(id)),
  };
}
