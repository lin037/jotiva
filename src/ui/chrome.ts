import { invoke } from "@tauri-apps/api/core";
import { button, report } from "./dom";

const glyphs = {
  collapse: "M3 3.5h10M4.5 10.5 8 7l3.5 3.5",
  expand: "M3 12.5h10M4.5 5.5 8 9l3.5-3.5",
  pin: "m7.2 2.4 6.4 6.4m-5.4-5.4L5.7 6.9l-2.1.7 4.8 4.8.7-2.1 3.5-2.5M6 10l-3.5 3.5",
  format: "M2.5 12.5 6.5 3l4 9.5m-6.75-2.75h5.5M11 8.5h3m-1.5 0v4",
  add: "M8 3v10M3 8h10",
  compose: "M8.3 3H4a1.5 1.5 0 0 0-1.5 1.5v7A1.5 1.5 0 0 0 4 13h7a1.5 1.5 0 0 0 1.5-1.5V8m-6 1.6.45-2.15 5.6-5.6 1.7 1.7-5.6 5.6z",
  folderAdd: "M2 5V3.5h4L7.5 5h5A1.5 1.5 0 0 1 14 6.5v5a1.5 1.5 0 0 1-1.5 1.5h-9A1.5 1.5 0 0 1 2 11.5ZM8 7v4M6 9h4",
  search: "M10.8 10.8l2.7 2.7M7 2.5a4.5 4.5 0 1 1 0 9 4.5 4.5 0 0 1 0-9Z",
  open: "M8.5 2.5h5v5m0-5-6 6m-1-4h-4v9h9v-4",
  sidebar: "M2.5 4a1.5 1.5 0 0 1 1.5-1.5h8A1.5 1.5 0 0 1 13.5 4v8a1.5 1.5 0 0 1-1.5 1.5H4A1.5 1.5 0 0 1 2.5 12ZM6 2.5v11",
  folder: "M2 5V4h4l2 2h5.5l-1 6.5H2z",
  grid: "M2.5 2.5h4v4h-4zm7 0h4v4h-4zm-7 7h4v4h-4zm7 0h4v4h-4z",
  list: "M4.5 4h9m-9 4h9m-9 4h9M2 4h.2M2 8h.2M2 12h.2",
  outline: "M2.5 3.5h11M4 6v6.5h3M4 8h3M9 8h4.5M9 12.5h4.5",
  settings: "M6.9 2h2.2l.4 1.5 1.1.6 1.5-.4 1.1 1.9-1.1 1.1v1.4l1.1 1.1-1.1 1.9-1.5-.4-1.1.6-.4 1.5H6.9l-.4-1.5-1.1-.6-1.5.4-1.1-1.9 1.1-1.1V6.7L2.8 5.6l1.1-1.9 1.5.4 1.1-.6ZM10 7.4a2 2 0 1 1-4 0 2 2 0 0 1 4 0Z",
  window: "M2.5 3h11v10h-11zM2.5 6h11",
  keys: "M2 4h12v8H2zM4.5 6.5h.2m2.2 0h.2m2.2 0h.2m2.2 0h.2m-7 3h6",
  appearance: "M11 8a3 3 0 1 1-6 0 3 3 0 0 1 6 0ZM8 1.5V3m0 10v1.5M1.5 8H3m10 0h1.5M3.4 3.4l1.1 1.1m7 7 1.1 1.1M3.4 12.6l1.1-1.1m7-7 1.1-1.1",
  source: "M4 5v6m0-4 6-3m-6 5 6 3M4 2.5h.01m6 1h3v3h-3zm0 6h3v3h-3z",
  more: "M3 8h.1m4.9 0h.1m4.9 0h.1",
};

export function icon(icon: keyof typeof glyphs) {
  const glyph = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  glyph.classList.add("jot-icon");
  glyph.setAttribute("viewBox", "0 0 16 16");
  glyph.setAttribute("aria-hidden", "true");
  const path = document.createElementNS(glyph.namespaceURI, "path");
  path.setAttribute("d", glyphs[icon]);
  glyph.append(path);
  return glyph;
}

export function iconButton(name: keyof typeof glyphs, label: string, action: () => unknown, key?: string, tip?: string) {
  const control = button("", action, "icon-button", tip);
  control.setAttribute("aria-label", label);
  if (key) {
    const shortcut = key.replace("⇧", "Shift+").replace("⌘", "Meta+").replace("Esc", "Escape");
    control.setAttribute("aria-keyshortcuts", shortcut);
    control.setAttribute("aria-description", "快捷键 " + key);
  }
  control.append(icon(name));
  return control;
}

export function bindWindowMenu(toggle: HTMLButtonElement, menu: HTMLElement, onEscape = () => toggle.focus()) {
  menu.tabIndex = -1;
  toggle.popoverTargetElement = menu;
  toggle.setAttribute("aria-controls", menu.id);
  toggle.setAttribute("aria-expanded", "false");
  const items = () => Array.from(menu.querySelectorAll<HTMLButtonElement>("button:not([hidden]):enabled"));
  let keyboardOpen = false;
  toggle.addEventListener("click", event => { keyboardOpen = event.detail === 0; });
  toggle.addEventListener("keydown", event => {
    if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
    event.preventDefault();
    menu.showPopover();
    const rows = items();
    rows[event.key === "ArrowDown" ? 0 : rows.length - 1]?.focus();
  });
  menu.addEventListener("toggle", () => {
    const open = menu.matches(":popover-open");
    toggle.setAttribute("aria-expanded", String(open));
    if (open && !menu.contains(document.activeElement)) {
      if (keyboardOpen) items()[0]?.focus();
      else menu.focus({ preventScroll: true });
    }
    keyboardOpen = false;
  });
  menu.addEventListener("keydown", event => {
    if (event.isComposing) return;
    if (event.key === "Escape") {
      event.preventDefault();
      menu.hidePopover();
      onEscape();
    } else if (event.key === "ArrowUp" || event.key === "ArrowDown") {
      event.preventDefault();
      const rows = items();
      const index = rows.indexOf(document.activeElement as HTMLButtonElement);
      const next = event.key === "ArrowDown" ? index + 1 : index <= 0 ? rows.length - 1 : index - 1;
      rows[next % rows.length]?.focus();
    }
  });
  menu.addEventListener("focusout", event => {
    if (menu.matches(":popover-open") && event.relatedTarget instanceof Node && event.relatedTarget !== toggle
      && !menu.contains(event.relatedTarget)) menu.hidePopover();
  });
}

export function dragOnMouseDown(region: HTMLElement) {
  region.addEventListener("mousedown", event => {
    if (event.button !== 0 || (event.target as Element).closest("button, input, select, [data-no-drag]")) return;
    event.preventDefault();
    void invoke("drag_window").catch(report);
  });
}
