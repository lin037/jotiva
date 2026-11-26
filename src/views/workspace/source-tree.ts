import { t, bindLabel, type Label, localized } from "../../i18n";
import { invoke } from "@tauri-apps/api/core";
import { popupMenu } from "../../ui/context-menu";
import { attachOnSwitch } from "./document-drag";
import { sources, type NoteId, type SourceGroup } from "../../desktop/ipc";
import { iconButton } from "../../ui/chrome";
import { button, chevron, element, report } from "../../ui/dom";
import { noteList } from "../../ui/note-list";

export function treeRow(title: Label, expanded: boolean, toggle: () => void) {
  const entry = button("", toggle, "library-node");
  entry.setAttribute("aria-expanded", String(expanded));
  bindLabel(entry, "title", title);
  const caret = chevron();
  caret.classList.add("tree-caret");
  entry.append(caret, element("span", "node-title", title));
  return entry;
}

export function sourceTree(open: (id: NoteId) => unknown, add: () => void) {
  const panel = element("section", "source-panel jot-sources");
  panel.setAttribute("role", "region");
  bindLabel(panel, "aria-label", localized("窗口便签"));
  const header = element("header", "library-section-heading");
  header.append(element("h2", undefined, localized("窗口便签")), iconButton("add", localized("添加来源窗口"), localized("添加来源"), add));
  const body = element("div", "source-sections");
  panel.append(header, body);
  const apps = new Map<string, boolean>();
  const windows = new Set<string>();
  const lists = new Map<string, ReturnType<typeof noteList>>();
  const collapsed = new Set([false]);
  const scrollPositions = new Map<string, number>();
  const initialized = new Set<boolean>();
  const limits = new Map<string, number>();
  const pageSize = 40;
  let requestedSource: string | undefined;
  let current: NoteId | undefined;
  let data: SourceGroup[] | undefined;
  let serial = 0;

  panel.addEventListener("contextmenu", event => {
    const element = event.target instanceof Element ? event.target : null;
    const window = element?.closest<HTMLElement>("[data-source]");
    const key = window?.dataset.source;
    if (!key) return;
    const live = window?.closest("[data-source-state]")?.getAttribute("data-source-state") === "live";
    const row = element?.closest<HTMLElement>("[data-note]");
    if (row?.dataset.note) {
      const id = Number(row.dataset.note);
      void popupMenu(event, [
        { label: t("在主窗口编辑"), run: () => open(id) },
        ...(live ? [{ label: t("定位到便签"), run: () => invoke("locate", { id }) }] : []),
        { label: t("切换应用挂载…"), run: () => attachOnSwitch(id) },
        null, { label: t("解除此便签挂载"), run: () => invoke("detach_note", { id }) },
      ]);
    } else {
      void popupMenu(event, [
        ...(live ? [{ label: t("定位到来源窗口"), run: () => invoke("locate_source", { key }) }] : []),
        { label: live ? t("关闭窗口挂载") : t("移除窗口关联"), run: () => invoke("close_source", { key }) },
      ]);
    }
  });

  function draw() {
    const focused = (document.activeElement as HTMLElement | null)?.dataset.treeNode;
    for (const tree of body.querySelectorAll(".source-lane-tree:not([hidden])")) {
      scrollPositions.set(tree.id, tree.scrollTop);
    }
    const visible = new Set<string>();
    function more(container: HTMLElement, key: string, total: number) {
      const limit = limits.get(key) ?? pageSize;
      if (total <= limit) return;
      container.append(button(localized("显示更多"), () => { limits.set(key, limit + pageSize); draw(); }, "library-node"));
    }
    const sections = [true, false].map(live => {
      const lane = element("section", "source-lane");
      lane.dataset.sourceState = live ? "live" : "missing";
      const laneKey = String(live);
      const laneName = () => live ? t("已挂载") : t("待重新挂载");
      const label = element("h3", "source-lane-label");
      label.id = "source-heading-" + lane.dataset.sourceState;
      const tree = element("div", "source-lane-tree");
      tree.id = "source-tree-" + lane.dataset.sourceState;
      tree.hidden = collapsed.has(live);
      lane.setAttribute("aria-labelledby", label.id);
      const heading = treeRow(laneName, !tree.hidden, () => {
        if (collapsed.has(live)) collapsed.delete(live);
        else collapsed.add(live);
        draw();
      });
      heading.classList.add("source-lane-heading");
      heading.dataset.treeNode = laneKey;
      heading.setAttribute("aria-controls", tree.id);
      label.append(heading);
      lane.append(label, tree);
      lane.classList.toggle("is-collapsed", tree.hidden);
      if (tree.hidden) return lane;
      const groups = (data ?? []).map(group => ({
        app: group.app, windows: group.windows.filter(item => item.attached === live),
      })).filter(group => group.windows.length);
      lane.classList.toggle("is-empty", !groups.length);
      if (!groups.length) {
        tree.append(element("p", "library-empty muted", () => live ? t("拖入文档或文本以选择窗口") : t("暂无离线窗口")));
      }
      for (const group of groups.slice(0, limits.get(laneKey) ?? pageSize)) {
        const groupKey = JSON.stringify([live, group.app]);
        const expanded = apps.get(groupKey) ?? (group === groups[0]);
        const untitled = (title: string) => !title || title === "未命名窗口" || title === "未指定窗口";
        const single = group.windows.length === 1 && untitled(group.windows[0].title);
        if (!single) {
          const appRow = treeRow(group.app, expanded, () => { apps.set(groupKey, !expanded); draw(); });
          appRow.dataset.treeNode = groupKey;
          tree.append(appRow);
          if (!expanded) continue;
        }
        const children = element("div", single ? undefined : "tree-children");
        const counts = new Map<string, number>();
        for (const item of group.windows.slice(0, limits.get(groupKey) ?? pageSize)) {
          const category = untitled(item.title) ? "" : item.title;
          const count = (counts.get(category) ?? 0) + 1;
          counts.set(category, count);
          const title = untitled(item.title) ? (single ? group.app : t("窗口 ") + count)
            : count > 1 ? item.title + "（" + count + "）" : item.title;
          const shown = windows.has(item.key);
          const windowBranch = element("section", "source-window");
          windowBranch.dataset.source = item.key;
          windowBranch.title = title;
          const row = treeRow(title, shown, () => {
            if (shown) windows.delete(item.key);
            else windows.add(item.key);
            draw();
          });
          row.dataset.treeNode = item.key;
          windowBranch.append(row);
          children.append(windowBranch);
          if (!shown) continue;
          visible.add(item.key);
          let list = lists.get(item.key);
          if (!list) {
            list = noteList((id, card) => card.attached ? invoke("locate", { id }) : open(id), { source: { kind: "source", key: item.key } });
            if (current !== undefined) list.setActive(current);
            lists.set(item.key, list);
          }
          windowBranch.append(list.panel);
        }
        more(children, groupKey, group.windows.length);
        tree.append(children);
      }
      more(tree, laneKey, groups.length);
      return lane;
    });
    for (const [key, list] of lists) {
      if (visible.has(key)) continue;
      list.dispose();
      lists.delete(key);
    }
    body.replaceChildren(...sections);
    for (const tree of body.querySelectorAll(".source-lane-tree:not([hidden])")) {
      tree.scrollTop = scrollPositions.get(tree.id) ?? 0;
    }
    if (focused) body.querySelector<HTMLElement>(`[data-tree-node="${CSS.escape(focused)}"]`)?.focus({ preventScroll: true });
  }

  async function refresh(reveal?: string) {
    if (reveal) requestedSource = reveal;
    const ticket = ++serial;
    const groups = await sources.list();
    if (ticket !== serial) return;
    const changed = JSON.stringify(groups) !== JSON.stringify(data);
    data = groups;
    for (const live of [true, false]) {
      if (initialized.has(live)) continue;
      const first = groups.find(group => group.windows.some(item => item.attached === live))
        ?.windows.find(item => item.attached === live);
      if (first) { windows.add(first.key); initialized.add(live); }
    }
    reveal = requestedSource;
    requestedSource = undefined;
    if (reveal) {
      const group = groups.find(group => group.windows.some(item => item.key === reveal));
      const item = group?.windows.find(item => item.key === reveal);
      if (group && item) {
        const groupKey = JSON.stringify([item.attached, group.app]);
        const groupIndex = groups.filter(g => g.windows.some(w => w.attached === item.attached)).indexOf(group);
        const windowIndex = group.windows.filter(w => w.attached === item.attached).indexOf(item);
        limits.set(String(item.attached), Math.max(groupIndex + 1, limits.get(String(item.attached)) ?? pageSize));
        limits.set(groupKey, Math.max(windowIndex + 1, limits.get(groupKey) ?? pageSize));
        apps.set(groupKey, true);
        collapsed.delete(item.attached);
        windows.add(reveal);
      }
    }
    const previous = [...lists.values()];
    if (changed || reveal) draw();
    await Promise.all(previous.map(list => list.refresh()));
    if (reveal) body.querySelector<HTMLElement>(`[data-source="${CSS.escape(reveal)}"]`)?.scrollIntoView({ block: "nearest" });
  }
  draw();
  void refresh().catch(report);
  return {
    panel, refresh,
    refreshDocuments: () => Promise.all([...lists.values()].map(list => list.refresh())),
    setActive(id: NoteId) { current = id; for (const list of lists.values()) list.setActive(id); },
  };
}
