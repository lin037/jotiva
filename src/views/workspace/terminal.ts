import { invoke } from "@tauri-apps/api/core";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { localized, bindLabel, t } from "../../i18n";
import { displayPath, desktopPlatform } from "../../desktop/platform";
import { notes } from "../../desktop/ipc";
import { iconButton } from "../../ui/chrome";
import { element, report } from "../../ui/dom";
import "@xterm/xterm/css/xterm.css";
import "./terminal.css";

export type TerminalLocation = { kind: "document"; id: number | null } | { kind: "workspace"; path: string } | { kind: "archive"; folder: number | null };
export type OpenTerminal = (location?: TerminalLocation) => Promise<void>;

interface Session {
  id: number;
  directory: string;
  client: Terminal;
  fit: FitAddon;
  screen: HTMLElement;
  live: boolean;
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
  const select = element("select");
  bindLabel(select, "aria-label", localized("切换终端"));
  const label = element("span", "terminal-directory muted");
  const restart = iconButton("add", localized("新建终端"), () => start());
  const kill = iconButton("close", localized("结束终端"), terminate);
  const collapse = iconButton("minimize", localized("收起终端"), hide);
  bar.append(title, label, select, ended, restart, collapse, kill);
  const body = element("div", "terminal-body");
  panel.append(handle, bar, body);
  parent.append(panel);

  const sessions = new Map<number, Session>();
  let active: Session | undefined;
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
  function theme() {
    const style = getComputedStyle(panel);
    return { background: style.backgroundColor, foreground: style.color, cursor: style.color };
  }
  new MutationObserver(() => { for (const {client} of sessions.values()) client.options.theme = theme(); })
    .observe(document.documentElement, { attributes: true, attributeFilter: ["data-jot-scheme", "style"] });

  function visible(value: boolean) {
    panel.hidden = !value;
    toggleButton.setAttribute("aria-expanded", String(value));
  }
  function hide() { visible(false); returnFocus(); }
  function update() {
    const entries = [...sessions.values()];
    const parts = entries.map(s => s.directory.split(desktopPlatform.pathSeparator).filter(Boolean));
    select.replaceChildren(...entries.map((s, i) => {
      let depth = 1;
      const suffix = (p: string[]) => p.slice(-depth).join(desktopPlatform.pathSeparator);
      while (depth < parts[i].length && parts.some((p, j) => i !== j && entries[j].directory !== s.directory && suffix(p) === suffix(parts[i]))) depth++;
      const base = suffix(parts[i]) || s.directory;
      const same = entries.filter(other => other.directory === s.directory);
      const option = new Option(same.length > 1 ? base + " · " + s.id : base, String(s.id));
      option.title = s.directory;
      return option;
    }));
    select.value = String(active?.id);
    select.hidden = entries.length < 2;
    label.hidden = !select.hidden;
    label.textContent = select.selectedOptions[0]?.text ?? "";
    label.title = select.title = active?.directory ?? "";
    ended.hidden = !active || active.live;
    kill.disabled = !active;
  }
  function activate(s: Session) {
    if (active) active.screen.hidden = true;
    active = s;
    s.screen.hidden = false;
    visible(true);
    update();
    resize();
    if (s.live) s.client.focus(); else restart.focus();
  }
  select.onchange = () => { const s = sessions.get(Number(select.value)); if (s) activate(s); };
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
  function start(location = current()) {
    const task = opening.then(async () => {
      const {id, directory} = await invoke<{id:number; directory:string}>("open_terminal", { location, cols:80, rows:24 });
      const screen = element("div", "terminal-screen");
      const client = new Terminal({ theme: theme(), cursorBlink: true, scrollback: 3000, fontSize: 13,
        fontFamily: getComputedStyle(panel).getPropertyValue("--jot-font-mono"), minimumContrastRatio: 4.5 });
      const fit = new FitAddon();
      const s = {id, directory:displayPath(directory), client, fit, screen, live:true};
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
        sessions.set(id, s);
        activate(s);
        void pump(s);
      } catch (e) {
        sessions.delete(id);
        screen.remove();
        client.dispose();
        await invoke("close_terminal", {id});
        throw e;
      }
    });
    opening = task.catch(() => {});
    return task;
  }
  async function terminate() {
    if (!active) return;
    const s = active;
    // Remove it before awaiting IPC so a second click or a late EOF cannot dispose it twice.
    const running = s.live;
    s.live = false;
    sessions.delete(s.id);
    s.client.dispose();
    s.screen.remove();
    active = undefined;
    const next = [...sessions.values()].pop();
    if (next) activate(next); else { update(); hide(); }
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
