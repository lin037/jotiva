import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { version } from "../../package.json";
import { bindLabel, localized, t } from "../i18n";
import { button, element, report } from "../ui/dom";
import { environment } from "./environment";

type Status = { kind: "idle" | "manual" | "checking" | "current" | "downloading" | "ready" }
  | { kind: "available"; version: string }
  | { kind: "failed"; message: string };

export async function watchUpdates(receive: (status: Status) => void) {
  if (!environment.updatesEnabled) { receive({ kind: "manual" }); return; }
  let changed = false;
  await listen<Status>("desktop:update-status", ({ payload }) => { changed = true; receive(payload); });
  const status = await invoke<Status>("read_update");
  if (!changed) receive(status);
}

export function mountUpdateSettings(panel: HTMLElement) {
  const group = element("section", "settings-group");
  const current = element("p", undefined, () => t("当前版本 {version}", { version }));
  const status = element("p", "settings-detail");
  status.setAttribute("role", "status");
  const check = button(localized("检查更新"), () => invoke("check_update"));
  const install = button(localized("安装更新并重启"), () => invoke("install_update"), "primary");
  install.hidden = true;
  group.append(current, status, check, install);
  panel.append(group);
  const messages = {
    idle: localized("启动时自动检查更新"), manual: localized("此安装包可通过发行页或系统包管理器更新。"),
    checking: localized("正在检查更新…"), current: localized("已是最新版本"),
    downloading: localized("正在下载并验证更新…"), ready: localized("正在保存并关闭窗口…"),
  };
  void watchUpdates(value => {
    check.hidden = value.kind === "manual";
    check.disabled = ["checking", "downloading", "ready"].includes(value.kind);
    install.hidden = value.kind !== "available";
    const label = value.kind === "available" ? () => t("发现新版本 {version}", { version: value.version })
      : value.kind === "failed" ? () => t("更新失败：{message}", { message: value.message }) : messages[value.kind];
    bindLabel(status, "text", label);
  }).catch(report);
}
