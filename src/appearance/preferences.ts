import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { events } from "../desktop/ipc";
import { setLanguage } from "../i18n";
import { accentColors } from "./accent";

export type ThemeMode = "system" | "light" | "dark";
export type WritingFont = "system" | "serif" | "mono";
export type HeadingScale = [number, number, number, number, number, number];
export const defaultHeadingScale: HeadingScale = [28 / 17, 22 / 17, 20 / 17, 18 / 17, 1, 1];

export interface EditingPreferences {
  font: WritingFont;
  wrapCode: boolean;
  foldHeadings: boolean;
  toolbar: boolean;
  headingScale: HeadingScale | null;
  ruledPaper: boolean;
}

export interface AppearancePreferences {
  language: import("../i18n").Language;
  baseSize: number | null;
  mode: ThemeMode;
  accent: string | null;
  editing: EditingPreferences;
}

interface Stylesheet {
  name: string;
  css: string;
}

export interface WorkspaceLayout {
  sidebar: number | null;
  outline: number | null;
}

export interface Appearance extends AppearancePreferences {
  systemSize: number;
  stylesheet: Stylesheet | null;
  workspace: WorkspaceLayout;
}

const media = matchMedia("(prefers-color-scheme: dark)");
const subscribers = new Set<(value: Appearance, stylesheetChanged: boolean) => void>();
const fontVariables: Record<WritingFont, string> = {
  system: "--jot-font-ui",
  serif: "--jot-font-serif",
  mono: "--jot-font-mono",
};
let appearance: Appearance;
let customStyle: HTMLStyleElement | undefined;

function apply(value: Appearance) {
  const stylesheetChanged = value.stylesheet?.css !== appearance?.stylesheet?.css;
  appearance = value;
  setLanguage(value.language);
  const { dataset, style } = document.documentElement;
  const dark = value.mode === "system" ? media.matches : value.mode === "dark";
  dataset.jotScheme = dark ? "dark" : "light";
  dataset.jotPaper = value.editing.ruledPaper ? "ruled" : "plain";
  style.setProperty("--jot-preferred-size", String(value.baseSize ?? value.systemSize) + "px");
  style.setProperty("--jot-preferred-editor-font", "var(" + fontVariables[value.editing.font] + ")");
  (value.editing.headingScale ?? defaultHeadingScale).forEach((ratio, index) => {
    style.setProperty("--jot-preferred-heading-ratio-" + (index + 1), String(ratio));
  });
  const colors = accentColors(value.accent, dark);
  style.setProperty("--jot-preferred-accent-fill", colors.fill);
  style.setProperty("--jot-preferred-accent-ink", colors.ink);
  style.setProperty("--jot-preferred-on-accent", colors.onFill);
  if (stylesheetChanged) {
    if (value.stylesheet) {
      customStyle ??= document.createElement("style");
      customStyle.dataset.jotCustomTheme = "";
      customStyle.textContent = value.stylesheet.css;
      document.head.append(customStyle);
    } else {
      customStyle?.remove();
      customStyle = undefined;
    }
  }
  subscribers.forEach(listener => listener(value, stylesheetChanged));
}

export async function bindAppearance() {
  let updated = false;
  await listen<Appearance>(events.appearanceChanged, ({ payload }) => {
    updated = true;
    apply(payload);
  });
  const initial = await invoke<Appearance>("read_appearance");
  if (!updated) apply(initial);
  media.addEventListener("change", () => apply(appearance));
}

export function currentAppearance() {
  return appearance;
}

export function onAppearanceChange(callback: (value: Appearance, stylesheetChanged: boolean) => void) {
  subscribers.add(callback);
  return () => { subscribers.delete(callback); };
}

export function saveAppearance(preferences: AppearancePreferences) {
  return invoke("save_appearance", { preferences });
}

export function setStylesheet(stylesheet: Stylesheet | null) {
  return invoke("set_stylesheet", { stylesheet });
}
