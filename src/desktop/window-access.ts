import { t } from "../i18n";
import { desktopPlatform } from "./platform";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { button, element } from "../ui/dom";
import "../styles/dialog.css";

export function accessibilityHelp() {
  const help = element("details", "access-help");
  const summary = element("summary", undefined, t("已打开开关，仍提示未授权？"));
  const guidance = element("p", undefined, t("替换过应用时，先退出，再从安装位置打开。如果仍提示未授权，在辅助功能列表里移除旧应用的条目，用「＋」添加当前安装的这一份并打开开关。"));
  const path = element("p");
  const finder = button(t("在 Finder 中显示这份 Jotiva"), () =>
    invoke("reveal_file", { path: installedPath }));
  let installedPath: string | undefined;
  finder.hidden = true;
  help.append(summary, guidance, path, finder);
  help.addEventListener("toggle", async () => {
    if (!help.open || installedPath) return;
    try {
      installedPath = await invoke<string>("running_app_path");
      path.textContent = t("当前运行位置：") + installedPath;
      if (installedPath.includes("/AppTranslocation/")) {
        guidance.textContent = t("macOS 正在从隔离位置运行 Jotiva。先退出，将 Jotiva 拖进「应用程序」并从那里打开，再为安装好的 Jotiva 授权。");
      } else if (installedPath.startsWith("/Volumes/")) {
        guidance.textContent = t("如果你打开的是安装映像中的 Jotiva，请先退出，将它拖进「应用程序」，从那里重新打开并授权。如果你有意从外置磁盘运行，请在辅助功能列表中添加下面这份 Jotiva。");
      }
      finder.hidden = false;
    } catch (error) {
      path.textContent = String(error);
    }
  });
  return help;
}

/** Recheck the native process on return, including panels where WebKit never takes DOM focus. */
export function watchAccessibility(update: (trusted: boolean) => void | Promise<void>, onError: (error: unknown) => void) {
  let checking = false;
  let stopped = false;
  async function check() {
    if (checking || stopped) return;
    checking = true;
    try {
      const trusted = await invoke<boolean>("accessibility_trusted");
      if (!stopped) await update(trusted);
    } catch (error) {
      if (!stopped) onError(error);
    } finally {
      checking = false;
    }
  }
  window.addEventListener("focus", check);
  document.addEventListener("visibilitychange", check);
  const timer = window.setInterval(check, 1000);
  void check();
  return () => {
    stopped = true;
    window.clearInterval(timer);
    document.removeEventListener("visibilitychange", check);
    window.removeEventListener("focus", check);
  };
}

/** Keep the requested window action pending while the user visits System Settings. */
export async function allowWindowAccess(title: string, signal?: AbortSignal): Promise<boolean> {
  if (!desktopPlatform.usesAccessibilityPermission) return desktopPlatform.hostWindows && !signal?.aborted;
  const trusted = await invoke<boolean>("accessibility_trusted");
  if (signal?.aborted || trusted) return !signal?.aborted && trusted;

  const dialog = element("dialog", "sheet jot-dialog");
  dialog.setAttribute("aria-label", title);
  const heading = element("h2", undefined, title);
  const instructions = t("在系统设置的「辅助功能」中打开 Jotiva 的开关，再回来选择目标窗口。");
  const notice = element("p", "form-message", instructions);
  notice.setAttribute("role", "status");
  const help = accessibilityHelp();
  const actions = element("div", "sheet-actions");
  const cancel = button(t("取消"), () => dialog.close());
  let requested = false;
  let granted = false;
  const allow = button(t("打开辅助功能设置…"), async () => {
    if (granted && await invoke<boolean>("accessibility_trusted")) { dialog.close("granted"); return; }
    requested = true;
    allow.disabled = true;
    try {
      if (await invoke<boolean>("request_accessibility") && dialog.open) dialog.close("granted");
    } catch (error) {
      notice.textContent = String(error);
    } finally {
      allow.disabled = false;
    }
  }, "primary");
  actions.append(cancel, allow);
  dialog.append(heading, notice, help, actions);
  document.body.append(dialog);

  const abort = () => { if (dialog.open) dialog.close(); };
  return new Promise<boolean>(resolve => {
    signal?.addEventListener("abort", abort);
    dialog.showModal();
    allow.focus();
    const stop = watchAccessibility(async trusted => {
      if (!dialog.open) return;
      if (granted !== trusted) {
        granted = trusted;
        help.hidden = trusted;
        allow.textContent = trusted ? t("继续") : t("打开辅助功能设置…");
        notice.textContent = trusted ? t("已授权，切回 Jotiva 即可继续。") : instructions;
      }
      // Don't arm a "switch to attach" action while System Settings is still in front.
      if (trusted && (document.hasFocus() || await getCurrentWindow().isFocused()) && dialog.open) {
        dialog.close("granted");
      }
    }, error => { notice.textContent = String(error); });
    dialog.addEventListener("close", () => {
      stop();
      signal?.removeEventListener("abort", abort);
      dialog.remove();
      resolve(!signal?.aborted && dialog.returnValue === "granted");
    }, { once: true });
  }).finally(async () => {
    if (requested) await invoke("finish_accessibility_request");
  });
}
