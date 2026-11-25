import { t } from "../i18n";
import { environment } from "./environment";

const { os, hostWindows } = environment;
const appKit = os === "macos";
export const desktopPlatform = {
  pathSeparator: os === "windows" ? "\\" : "/",
  hostWindows,
  windowsChrome: os === "windows",
  quoteShellPath: os === "windows"
    ? (path: string) => '"' + path.replace(/[`$"“”„]/gu, char => "`" + char) + '"'
    : (path: string) => path.replace(/[^a-zA-Z0-9_./\-\u0080-\u{10ffff}]/gu, char => "\\" + char),
  customCaption: !appKit,
  usesAccessibilityPermission: appKit,
  nativePasteboard: appKit,
  nativePopup: appKit,
  shortcutSymbols: appKit,
  switchWindow: appKit ? "⌘Tab" : "Alt+Tab",
  get recordingHint() { return appKit ? t("请按下包含 ⌘、⌃ 或 ⌥ 的组合键，或为应用内操作选择功能键。") : t("请按下包含 Ctrl 或 Alt 的组合键，或选择功能键。"); },
  primary: appKit ? "Super" : "Control",
  primaryLabel: appKit ? "⌘" : "Ctrl+",
  editor: appKit ? "mac" : "windows",
  get trash() { return appKit ? t("废纸篓") : t("回收站"); },
  get reveal() { return { macos: t("在 Finder 中显示"), windows: t("在文件资源管理器中显示"), linux: t("在文件管理器中显示") }[os]; },
  get keyStore() { return { macos: t("macOS 钥匙串"), windows: t("Windows 凭据管理器"), linux: t("系统密钥环") }[os]; },
} as const;
document.documentElement.dataset.platform = os;

export const fileName = (path: string) => path.slice(path.lastIndexOf(desktopPlatform.pathSeparator) + 1);

export function primaryPressed(event: KeyboardEvent) {
  return appKit ? event.metaKey && !event.ctrlKey : event.ctrlKey && !event.metaKey;
}

export function deleteSelectionPressed(event: KeyboardEvent) {
  if (event.shiftKey || event.altKey) return false;
  return appKit ? primaryPressed(event) && event.key === "Backspace"
    : !event.metaKey && !event.ctrlKey && event.key === "Delete";
}

/** Present DOS and UNC paths as Explorer does, keeping other device namespaces intact. */
export const displayPath = os === "windows"
  ? (path: string) => path.replace(/^\\\\\?\\UNC\\/i, "\\\\").replace(/^\\\\\?\\(?=[a-z]:\\)/i, "")
  : (path: string) => path;
