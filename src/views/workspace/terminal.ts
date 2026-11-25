import { onAppearanceChange } from "../../appearance/preferences";
import { invoke } from "@tauri-apps/api/core";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { localized, bindLabel, t } from "../../i18n";
import { displayPath, desktopPlatform } from "../../desktop/platform";
import { notes } from "../../desktop/ipc";
import { icon, iconButton } from "../../ui/chrome";
import { fileDialog } from "../../ui/file-dialog";
import { popupMenu } from "../../ui/context-menu";
import { button, element, report } from "../../ui/dom";
import "@xterm/xterm/css/xterm.css";
import "./terminal.css";

export type TerminalLocation = { kind: "document"; id: number | null } | { kind: "workspace"; path: string } | { kind: "archive"; folder: number | null };
export type OpenTerminal = (location?: TerminalLocation) => Promise<void>;

interface Session {
  id: number;
  origin: TerminalLocation;
  defaultName: string;
  name: HTMLElement;
  badge: HTMLElement;
  lastUsed: number;
  row: HTMLElement;
  tab: HTMLButtonElement;
  client: Terminal;
  fit: FitAddon;
  screen: HTMLElement;
  live: boolean;
}

function sameLocation(a: TerminalLocation, b: TerminalLocation) {
  switch (a.kind) {
    case "document": return b.kind === "document" && a.id === b.id;
    case "workspace": return b.kind === "workspace" && a.path === b.path;
    case "archive": return b.kind === "archive" && a.folder === b.folder;
  }
}

export function terminalPanel(parent: HTMLElement, toggleButton: HTMLButtonElement, current: () => TerminalLocation, returnFocus: () => void) {
  const panel = element("section", "terminal-panel");
  panel.id = "workspace-terminal";
  panel.hidden = true;
  bindLabel(panel, "aria-label", localized("终端"));
  const handle = element("div", "terminal-divider");
  handle.tabIndex = 0;
  handle.setAttribute("role", "separator");
  handle.setAttribute("aria-orientation", "horizontal");
  handle.setAttribute("aria-controls", panel.id);
  bindLabel(handle, "aria-label", localized("调整终端高度"));
  const bar = element("header", "terminal-heading");
  const title = element("strong", undefined, localized("终端"));
  const ended = element("span", "muted", localized("终端已结束"));
  ended.setAttribute("role", "status");
  const list = element("div", "terminal-sessions");
  list.setAttribute("role", "tablist");
  list.setAttribute("aria-orientation", "vertical");
  bindLabel(list, "aria-label", localized("切换终端"));
  const add = iconButton("add", localized("新建终端"), localized("新建终端"), () => start());
  const zoomLabel = () => panel.classList.contains("terminal-maximized") ? t("还原终端面板") : t("最大化终端面板");
  const zoom = iconButton("maximize", zoomLabel, () => panel.classList.contains("terminal-maximized") ? t("还原") : t("最大化"), () => {
    const expanded = panel.classList.toggle("terminal-maximized");
    zoom.setAttribute("aria-pressed", String(expanded));
    bindLabel(zoom, "aria-label", zoomLabel);
    bindLabel(zoom, "data-tip", () => expanded ? t("还原") : t("最大化"));
    zoom.replaceChildren(icon(expanded ? "restore" : "maximize"));
  });
  zoom.setAttribute("aria-pressed", "false");
  const hideButton = iconButton("close", localized("收起终端"), localized("收起终端"), hide);
  const actions = element("div", "terminal-actions");
  actions.append(add, zoom, hideButton);
  bar.append(title, ended, actions);
  const deck = element("div", "terminal-deck");
  const body = element("div", "terminal-body");
  deck.append(body, list);
  panel.append(handle, bar, deck);
  parent.append(panel);

  const sessions = new Map<number, Session>();
  let active: Session | undefined;
  let lastUse = 0;
  let opening = Promise.resolve();
  function resize() {
    if (!panel.hidden && active?.screen.clientWidth && active.screen.clientHeight) active.fit.fit();
    handle.setAttribute("aria-valuenow", String(panel.offsetHeight));
  }
  new ResizeObserver(resize).observe(body);
  function height(value: number) {
    panel.style.height = Math.max(96, Math.min(parent.clientHeight * 0.75, value)) + "px";
  }
  handle.onpointerdown = event => {
    if (event.button !== 0) return;
    event.preventDefault();
    handle.setPointerCapture(event.pointerId);
  };
  handle.onpointermove = event => {
    if (handle.hasPointerCapture(event.pointerId)) height(panel.getBoundingClientRect().bottom - event.clientY);
  };
  handle.onkeydown = event => {
    if (!["ArrowUp", "ArrowDown"].includes(event.key)) return;
    event.preventDefault();
    height(panel.offsetHeight + (event.key === "ArrowUp" ? 16 : -16));
  };
  function appearanceOptions() {
    const style = getComputedStyle(panel);
    const font = getComputedStyle(body);
    return { theme: { background: style.backgroundColor, foreground: style.color, cursor: style.color },
      fontSize: Number.parseFloat(font.fontSize), fontFamily: font.fontFamily };
  }
  onAppearanceChange(() => {
    const options = appearanceOptions();
    for (const {client} of sessions.values()) client.options = options;
    resize();
  });

  function visible(value: boolean) {
    panel.hidden = !value;
    toggleButton.setAttribute("aria-expanded", String(value));
  }
  function hide() { visible(false); returnFocus(); }
  function update() {
    if (sessions.size === 1 && list.parentNode !== bar) bar.insertBefore(list, actions);
    if (sessions.size !== 1 && list.parentNode !== deck) deck.append(list);
    const names = new Set<string>();
    const duplicates = new Set<string>();
    for (const {name} of sessions.values()) {
      if (names.has(name.textContent)) duplicates.add(name.textContent);
      names.add(name.textContent);
    }
    for (const s of sessions.values()) {
      s.badge.hidden = !duplicates.has(s.name.textContent);
      s.tab.setAttribute("aria-selected", String(s === active));
      s.tab.tabIndex = s === active ? 0 : -1;
      s.row.classList.toggle("ended", !s.live);
    }
    ended.hidden = !active || active.live;
  }
  function activate(s: Session, focusTab = false) {
    if (active) active.screen.hidden = true;
    active = s;
    s.lastUsed = ++lastUse;
    s.screen.hidden = false;
    visible(true);
    update();
    resize();
    if (focusTab || !s.live) s.tab.focus(); else s.client.focus();
  }
  async function rename(s: Session) {
    const value = await fileDialog(t("重命名终端"), t("保存"), {
      initial: s.name.textContent, allowEmpty: true, detail: s.tab.getAttribute("aria-description") ?? undefined,
    });
    if (value === undefined || !sessions.has(s.id)) return;
    s.name.textContent = value || s.defaultName;
    update();
  }
  list.addEventListener("keydown", event => {
    if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
    const entries = [...sessions.values()];
    const index = entries.findIndex(s => s.tab === event.target);
    if (index < 0) return;
    let next: Session;
    switch (event.key) {
      case "ArrowDown": next = entries[(index + 1) % entries.length]; break;
      case "ArrowUp": next = entries[(index + entries.length - 1) % entries.length]; break;
      case "Home": next = entries[0]; break;
      case "End": next = entries[entries.length - 1]; break;
      case "F2":
        event.preventDefault();
        void rename(entries[index]).catch(report);
        return;
      case "Delete":
        event.preventDefault();
        void terminate(entries[index]).catch(report);
        return;
      default: return;
    }
    event.preventDefault();
    activate(next, true);
  });
  async function pump(s: Session) {
    try {
      while (s.live) {
        const bytes = new Uint8Array(await invoke<ArrayBuffer>("read_terminal", { id: s.id }));
        if (!bytes.length || !s.live) break;
        await new Promise<void>(done => s.client.write(bytes, done));
      }
    } catch (e) { if (s.live) report(e); }
    finally {
      if (s.live) {
        s.live = false;
        s.client.options.disableStdin = true;
        update();
        await invoke("close_terminal", {id: s.id}).catch(report);
      }
    }
  }
  function mostRecent(entries: Session[]) {
    return entries.sort((a, b) => b.lastUsed - a.lastUsed)[0];
  }
  function start(location = current(), mode: "new" | "reveal" = "new") {
    const task = opening.then(async () => {
      if (mode === "reveal") {
        const matching = [...sessions.values()].filter(s => s.live && sameLocation(s.origin, location));
        const recent = mostRecent(matching);
        if (recent) { activate(recent); return; }
      }
      const {id, shell, directory, fileName} = await invoke<{ id: number; shell: string; directory: string; fileName: string | null }>("open_terminal", { location, cols:80, rows:24 });
      const screen = element("div", "terminal-screen");
      const client = new Terminal({ ...appearanceOptions(), cursorBlink: true, scrollback: 3000, minimumContrastRatio: 4.5 });
      const fit = new FitAddon();
      const row = element("div", "terminal-session");
      const tab = button("", () => activate(s), "terminal-tab");
      tab.id = "terminal-tab-" + id;
      tab.setAttribute("role", "tab");
      screen.id = "terminal-screen-" + id;
      tab.setAttribute("aria-controls", screen.id);
      screen.setAttribute("role", "tabpanel");
      screen.setAttribute("aria-labelledby", tab.id);
      const path = displayPath(directory);
      const defaultName = fileName || path.split(desktopPlatform.pathSeparator).filter(Boolean).pop() || path;
      const name = element("span", "terminal-name", defaultName);
      const badge = element("small", "terminal-id", "#" + id);
      tab.setAttribute("aria-description", (fileName ? fileName + " · " : "") + shell + "\n" + path);
      tab.setAttribute("aria-keyshortcuts", "F2");
      bindLabel(tab, "data-tip", localized("切换终端"));
      tab.append(icon(fileName ? "file" : "folder"), name, badge);
      const remove = iconButton("trash", localized("结束终端"), localized("结束终端"), () => terminate(s));
      remove.setAttribute("aria-describedby", tab.id);
      remove.classList.add("terminal-remove");
      row.append(tab, remove);
      const s: Session = { id, origin: location, defaultName, name, badge, lastUsed: 0, row, tab, client, fit, screen, live: true };
      tab.addEventListener("contextmenu", event => {
        void popupMenu(event, [{label: t("重命名终端"), run: () => rename(s)}]);
      });
      try {
        body.append(screen);
        visible(true);
        client.loadAddon(fit);
        client.open(screen);
        let writes = Promise.resolve();
        client.onData(data => {
          writes = writes.then(async () => { if (s.live) await invoke("write_terminal", {id, data}); })
            .catch(e => { if (s.live) report(e); });
        });
        client.onResize(({cols, rows}) => { if (s.live) void invoke("resize_terminal", {id, cols, rows}).catch(e => { if (s.live) report(e); }); });
        list.append(row);
        sessions.set(id, s);
        activate(s);
        void pump(s);
      } catch (e) {
        sessions.delete(id);
        row.remove();
        screen.remove();
        client.dispose();
        await invoke("close_terminal", {id});
        throw e;
      }
    });
    opening = task.catch(() => {});
    return task;
  }
  async function terminate(s: Session) {
    if (!sessions.delete(s.id)) return;
    const running = s.live;
    s.live = false;
    s.client.dispose();
    s.screen.remove();
    s.row.remove();
    const next = s === active ? mostRecent([...sessions.values()]) : active;
    if (next) activate(next);
    else {
      active = undefined;
      update();
      hide();
    }
    if (running) await invoke("close_terminal", { id: s.id });
  }
  async function toggle() {
    await opening;
    if (!panel.hidden) hide();
    else if (active) activate(active);
    else await start();
  }
  body.addEventListener("keydown", event => event.stopPropagation());
  // Accept only drags originating on a document row in this workspace.
  let dragged: HTMLElement | null = null;
  document.addEventListener("dragstart", event => {
    dragged = event.target instanceof Element ? event.target.closest("[data-file], [role=tab][data-note]") : null;
  }, true);
  document.addEventListener("dragend", () => { dragged = null; });
  body.addEventListener("dragover", event => {
    if (dragged && active?.live) { event.preventDefault(); if (event.dataTransfer) event.dataTransfer.dropEffect = "copy"; }
  });
  body.addEventListener("drop", event => {
    if (!dragged || !active?.live) return;
    event.preventDefault();
    const row = dragged;
    const s = active;
    dragged = null;
    void (async () => {
      const card = !row.dataset.file && row.dataset.note ? await notes.card(Number(row.dataset.note)) : undefined;
      const path = row.dataset.file || card?.file || card?.archivePath;
      if (!path || !s.live || s !== active) return;
      if (/[\x00-\x1f\x7f-\x9f\u2028\u2029]/u.test(path)) throw Error(t("路径包含终端控制字符"));
      s.client.focus();
      s.client.paste(desktopPlatform.quoteShellPath(displayPath(path)));
    })().catch(report);
  });
  return { start, toggle, ready: () => opening, hasSessions: () => sessions.size > 0 };
}
