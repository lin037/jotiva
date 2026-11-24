import type { Language } from "../i18n";

interface DesktopEnvironment {
  os: "macos" | "windows" | "linux";
  hostWindows: boolean;
  language?: Language;
}

declare global {
  interface Window { __jotPlatform?: DesktopEnvironment }
}

// Standalone browser previews use the macOS layout and source language.
export const environment: DesktopEnvironment = window.__jotPlatform ?? { os: "macos", hostWindows: true };
