import { t, bindLabel } from "../i18n";
import { desktopPlatform } from "./platform";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import commandRows from "./shortcut-commands.json";
import editorRows from "./shortcut-editor.json";
import referenceRows from "./shortcut-reference.json";
import { report } from "../ui/dom";

export interface ShortcutCommand { id: string; label: string; group: string; key: string | null; desktopKey?: string | null; menu?: string; editor?: string }
export const commands: ShortcutCommand[] = [...commandRows, ...editorRows].map((row: ShortcutCommand) => {
  const key = desktopPlatform.shortcutSymbols ? row.key : row.desktopKey ?? row.key;
  return {...row, key: key?.replace("Primary", desktopPlatform.primary) ?? null};
});
export const fixedShortcuts = referenceRows.flatMap(row => {
  const key = desktopPlatform.shortcutSymbols ? row.key : "desktopKey" in row ? row.desktopKey : row.key.replace(/⌘/g, "Ctrl+").replace(/⇧/g, "Shift+").replace(/⌥/g, "Alt+").replace(/⌃/g, "Ctrl+").replace(/↩/g,"Enter");
  return key ? [{...row, key}] : [];
});
export const globalActions = [["quick", "新建便签"], ["search", "查找便签和文档"], ["dock", "靠右收拢 / 还原"]] as const;
export type GlobalAction = typeof globalActions[number][0];
export type LocalBindings = Record<string, string | null>;
export type GlobalBindings = Record<GlobalAction, string | null> & { quick: string; local?: LocalBindings };
const byId = new Map(commands.map(row => [row.id, row]));
const listeners = new Set<(bindings: LocalBindings) => void>();
let local: LocalBindings = {};
let quickShortcut = "Control+Shift+Space";
let actions = new Map<string, string>();

const symbols: Record<string, string> = {
  Control:"⌃", Alt:"⌥", Shift:"⇧", Super:"⌘", get Space() { return t("空格"); }, Enter:"↩", Backquote:"`",
  Comma:",", Period:".", Slash:"/", Backslash:"\\", Minus:"-", Equal:"=", BracketLeft:"[",
  BracketRight:"]", Semicolon:";", Quote:"'", Tab:"Tab", Backspace:"⌫", ArrowLeft:"←",
  ArrowRight:"→", ArrowUp:"↑", ArrowDown:"↓",
};
const modifiers = ["Control", "Alt", "Shift", "Super"];
function normalized(chord: string): string {
  const parts = chord.split("+");
  const key = parts.pop()!;
  const mods = parts.map(part => ({ctrl:"Control", control:"Control", option:"Alt", alt:"Alt",
    shift:"Shift", meta:"Super", cmd:"Super", command:"Super", super:"Super"})[part.toLowerCase()] ?? part);
  return [...modifiers.filter(mod => mods.includes(mod)), key].join("+");
}
export function shortcutLabel(chord: string | null | undefined) {
  if (!chord) return t("未设置");
  const keys = normalized(chord).split("+");
  if (desktopPlatform.shortcutSymbols) return keys.map(part => symbols[part] ?? part.replace(/^(Key|Digit)/, "")).join("");
  return keys.map(part => ({Control:"Ctrl",Super:"Super"}[part] ?? (modifiers.includes(part) ? part : symbols[part] ?? part.replace(/^(Key|Digit)/,"")))).join("+");
}
export function effectiveShortcut(id: string, overrides: LocalBindings = local) {
  return Object.prototype.hasOwnProperty.call(overrides, id) ? overrides[id] : byId.get(id)?.key ?? null;
}
export function applyLocalShortcuts(overrides: LocalBindings) {
  local = overrides;
  actions = new Map();
  for (const {id} of commandRows) {
    const key = effectiveShortcut(id);
    if (key) actions.set(normalized(key), id);
  }
  document.querySelectorAll<HTMLElement>("[data-shortcut-action]").forEach(updateHint);
  for (const listener of listeners) listener(local);
}
applyLocalShortcuts({});
export function onLocalShortcuts(listener: (bindings: LocalBindings) => void) {
  listeners.add(listener);
  listener(local);
  return () => { listeners.delete(listener); };
}
function updateHint(element: HTMLElement) {
  const id = element.dataset.shortcutAction!;
  const key = id === "quick" ? quickShortcut : effectiveShortcut(id);
  if (element.tagName === "KBD") {
    element.textContent = key ? shortcutLabel(key) : "";
    element.hidden = !key;
  } else if (key) {
    const chord = normalized(key).replace("Super+", "Meta+").replace(/(Key|Digit)([A-Z0-9])$/, "$2");
    element.setAttribute("aria-keyshortcuts", chord);
    bindLabel(element, "aria-description", () => t("快捷键 ") + shortcutLabel(key));
  } else {
    element.removeAttribute("aria-keyshortcuts");
    element.removeAttribute("aria-description");
  }
}
export function shortcutHint<T extends HTMLElement>(element: T, action: string): T {
  element.dataset.shortcutAction = action;
  updateHint(element);
  return element;
}
export async function loadLocalShortcuts() {
  let updated = false;
  let quickUpdated = false;
  await Promise.all([
    listen<string>("desktop:quick-shortcut-changed", ({payload}) => {
      quickUpdated = true;
      quickShortcut = payload;
      document.querySelectorAll<HTMLElement>('[data-shortcut-action="quick"]').forEach(updateHint);
    }),
    listen<LocalBindings>("desktop:shortcuts-changed", ({payload}) => {
      updated = true;
      applyLocalShortcuts(payload);
    }),
  ]);
  const {preferences} = await invoke<{preferences: GlobalBindings}>("read_settings");
  if (!quickUpdated) quickShortcut = preferences.quick;
  if (!updated) applyLocalShortcuts(preferences.local ?? {});
}

function chordForEvent(event: KeyboardEvent) {
  return [event.ctrlKey && "Control", event.altKey && "Alt", event.shiftKey && "Shift",
    event.metaKey && "Super", event.code].filter(Boolean).join("+");
}
export function matchesShortcut(event: KeyboardEvent, id: string) {
  const key = effectiveShortcut(id);
  return !!key && normalized(key) === chordForEvent(event);
}
export function recordedShortcut(event: KeyboardEvent, global = false) {
  const func = /^F([1-9]|1[0-9]|20)$/.test(event.code);
  if (!event.ctrlKey && !event.altKey && !event.metaKey && (global || !func)) return;
  if (!func && !/^(Key[A-Z]|Digit[0-9]|Space|Enter|Tab|Comma|Period|Slash|Backslash|Minus|Equal|BracketLeft|BracketRight|Backquote|Semicolon|Quote|Arrow(Up|Down|Left|Right)|Backspace)$/.test(event.code)) return;
  return chordForEvent(event);
}

type Handler = () => unknown;
export function bindWindowShortcuts(handlers: Partial<Record<string, Handler>>, escape?: Handler) {
  const dispatch = (event: KeyboardEvent, capture: boolean) => {
    if (event.defaultPrevented || event.repeat || event.isComposing || document.querySelector("dialog[open]")) return;
    const chord = chordForEvent(event);
    const action = actions.get(chord);
    // Respect local remaps before the editor handles its context-specific keys.
    // An attached note also needs Command-Enter before the editor consumes it.
    if (action !== "terminal" && event.target instanceof Element && event.target.closest(".terminal-screen")) return;
    const beforeEditor = action === "terminal" || action === "returnHost" || !!(action && local[action]);
    if (capture !== beforeEditor) return;
    const handle = action ? handlers[action] : chord === "Escape" ? escape : undefined;
    if (!handle) return;
    event.preventDefault();
    if (capture) event.stopPropagation();
    void Promise.resolve().then(handle).catch(report);
  };
  window.addEventListener("keydown", event => dispatch(event, true), true);
  window.addEventListener("keydown", event => dispatch(event, false));
}
