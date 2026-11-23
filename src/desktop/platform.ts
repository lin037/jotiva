type OS = "macos" | "windows" | "linux";

interface NativePlatform { os: OS; hostWindows: boolean }
const { os, hostWindows } = (window as Window & { __jotPlatform?: NativePlatform }).__jotPlatform
  ?? { os: "macos", hostWindows: true }; // Browser-only previews use the macOS layout.
const appKit = os === "macos";
export const desktopPlatform = {
  pathSeparator: os === "windows" ? "\\" : "/",
  hostWindows,
  customCaption: !appKit,
  usesAccessibilityPermission: appKit,
  nativeNameDialog: appKit,
  nativePasteboard: appKit,
  nativePopup: appKit,
  shortcutSymbols: appKit,
  switchWindow: appKit ? "⌘Tab" : "Alt+Tab",
  recordingHint: appKit ? "请按下包含 ⌘、⌃ 或 ⌥ 的组合键，或为应用内操作选择功能键。" : "请按下包含 Ctrl 或 Alt 的组合键，或选择功能键。",
  primary: appKit ? "Super" : "Control",
  primaryLabel: appKit ? "⌘" : "Ctrl+",
  editor: appKit ? "mac" : "windows",
  trash: appKit ? "废纸篓" : "回收站",
  reveal: { macos: "在 Finder 中显示", windows: "在文件资源管理器中显示", linux: "在文件管理器中显示" }[os],
  keyStore: { macos: "macOS 钥匙串", windows: "Windows 凭据管理器", linux: "系统密钥环" }[os],
} as const;
document.documentElement.dataset.platform = os;

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
