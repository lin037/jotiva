import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { events } from "../desktop/ipc";
import { setLanguage } from "../i18n";
import { accentColors } from "./accent";

import ink from "../../docs/assets/ink-and-steel.css?inline";
import material from "./material.css?inline";
import blueprint from "../../docs/assets/blueprint.css?inline";

const presets = { default: "", ink, material, blueprint };
export type ThemePreset = keyof typeof presets;
export type ThemeMode = "system" | "light" | "dark";
export type WritingFont = "system" | "serif" | "mono";
export type HeadingScale = [number, number, number, number, number, number];
export const defaultHeadingScale: HeadingScale = [28 / 17, 22 / 17, 20 / 17, 18 / 17, 1, 1];

export interface EditingPreferences {
  font: WritingFont;
  wrapCode: boolean;
  foldHeadings: boolean;
  toolbar: boolean;
  previewTabs: boolean;
  headingScale: HeadingScale | null;
  ruledPaper: boolean;
}

export interface AppearancePreferences {
  language: import("../i18n").Language;
  baseSize: number | null;
  mode: ThemeMode;
  accent: string | null;
  themePreset: ThemePreset;
  themeFile: string | null;
  editing: EditingPreferences;
}

export interface Stylesheet {
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
  themeCss: string | null;
  workspace: WorkspaceLayout;
}

const media = matchMedia("(prefers-color-scheme: dark)");
const subscribers = new Set<(value: Appearance, themeChanged: boolean) => void>();
const fontVariables: Record<WritingFont, string> = {
  system: "--jot-font-ui",
  serif: "--jot-font-serif",
  mono: "--jot-font-mono",
};
let appearance: Appearance;
let presetStyle: HTMLStyleElement | undefined;
let customStyle: HTMLStyleElement | undefined;

function apply(value: Appearance) {
  const preset = value.themePreset ?? "default";
  const selectedTheme = value.themeFile ?? preset;
  const previousTheme = appearance?.themeFile ?? appearance?.themePreset ?? "default";
  const presetChanged = selectedTheme !== previousTheme || (value.themeCss ?? null) !== (appearance?.themeCss ?? null);
  const customChanged = value.stylesheet?.css !== appearance?.stylesheet?.css;
  const scheme = (value.mode === "system" ? media.matches : value.mode === "dark") ? "dark" : "light";
  const themeChanged = presetChanged || customChanged || value.accent !== appearance?.accent
    || document.documentElement.dataset.jotScheme !== scheme;
  if (presetChanged) {
    const css = value.themeFile ? value.themeCss : presets[preset];
    if (!css) {
      presetStyle?.remove();
      presetStyle = undefined;
    } else {
      presetStyle ??= document.createElement("style");
      presetStyle.dataset.jotPreset = selectedTheme;
      presetStyle.textContent = "@layer jot-preset {\n" + css.replace(/^\uFEFF/, "") + "\n}";
      document.head.insertBefore(presetStyle, customStyle ?? null);
    }
  }
  appearance = value;
  setLanguage(value.language);
  const { dataset, style } = document.documentElement;
  dataset.jotScheme = scheme;
  dataset.jotPaper = value.editing.ruledPaper ? "ruled" : "plain";
  style.setProperty("--jot-preferred-size", String(value.baseSize ?? value.systemSize) + "px");
  style.setProperty("--jot-preferred-editor-font", "var(" + fontVariables[value.editing.font] + ")");
  (value.editing.headingScale ?? defaultHeadingScale).forEach((ratio, index) => {
    style.setProperty("--jot-preferred-heading-ratio-" + (index + 1), String(ratio));
  });
  const colors = accentColors(value.accent, scheme === "dark");
  style.setProperty("--jot-preferred-accent-fill", colors.fill);
  style.setProperty("--jot-preferred-accent-ink", colors.ink);
  style.setProperty("--jot-preferred-on-accent", colors.onFill);
  if (customChanged) {
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
  subscribers.forEach(listener => listener(value, themeChanged));
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

export function onAppearanceChange(callback: (value: Appearance, themeChanged: boolean) => void) {
  subscribers.add(callback);
  return () => { subscribers.delete(callback); };
}

export function saveAppearance(preferences: AppearancePreferences) {
  return invoke("save_appearance", { preferences });
}

export function setStylesheet(stylesheet: Stylesheet | null) {
  return invoke("set_stylesheet", { stylesheet });
}
