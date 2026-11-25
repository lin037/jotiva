import { t, bindLabel, type Label, localized } from "../i18n";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { desktopPlatform } from "../desktop/platform";
import { dragOnMouseDown, icon, iconButton } from "./chrome";
import { element, report } from "./dom";
import appIcon from "../assets/jot.svg";
import "./titlebar.css";

function appMenu() {
  const button = iconButton("menu", localized("应用菜单"), localized("应用菜单"), () => {
    const { left, bottom } = button.getBoundingClientRect();
    return invoke("application_menu", { x: left, y: bottom });
  });
  button.setAttribute("aria-haspopup", "menu");
  return button;
}

function captionButton(action: "close" | "minimize" | "maximize", label: Label, click: () => Promise<void>) {
  const button = iconButton(action, label, label, click);
  button.classList.add("caption-button");
  button.dataset.caption = action;
  bindLabel(button, "title", label);
  return button;
}

export function titlebar(region: HTMLElement, compact = false) {
  if (!desktopPlatform.customCaption || compact) dragOnMouseDown(region);
  if (!desktopPlatform.customCaption) return;
  if (!compact) region.setAttribute("data-tauri-drag-region", "");
  const win = getCurrentWindow();
  const controls = element("div", "caption-controls");
  controls.setAttribute("role", "group");
  bindLabel(controls, "aria-label", localized("窗口控制"));
  if (!compact) {
    const maximize = captionButton("maximize", localized("最大化"), () => win.toggleMaximize());
    const sync = async () => {
      const maximized = await win.isMaximized();
      const label = () => maximized ? t("还原窗口") : t("最大化");
      bindLabel(maximize, "title", label);
      bindLabel(maximize, "data-tip", label);
      bindLabel(maximize, "aria-label", label);
      maximize.replaceChildren(icon(maximized ? "restore" : "maximize"));
    };
    void sync().catch(report);
    void win.onResized(() => void sync().catch(report)).catch(report);
    controls.append(captionButton("minimize", localized("最小化"), () => win.minimize()), maximize);
    const leading = region.querySelector(".workspace-leading");
    if (leading) {
      leading.prepend(appMenu());
      if (desktopPlatform.windowsChrome) {
        const logo = element("img", "titlebar-logo");
        logo.src = appIcon;
        logo.alt = "Jotiva";
        logo.draggable = false;
        leading.prepend(logo);
        for (const part of [logo, leading, region.querySelector(".workspace-trailing")]) part?.setAttribute("data-tauri-drag-region", "");
        region.addEventListener("contextmenu", event => {
          if (!(event.target instanceof Element) || !event.target.hasAttribute("data-tauri-drag-region")) return;
          event.preventDefault();
          void invoke("system_window_menu").catch(report);
        });
      }
    }
    else region.prepend(appMenu());
  }
  controls.append(captionButton("close", localized("关闭窗口"), () => win.close()));
  const trailing = desktopPlatform.windowsChrome && region.querySelector(".workspace-trailing");
  (trailing || region).append(controls);
}

export function secondaryTitlebar(title: Label) {
  const bar = element("header", "secondary-titlebar jot-titlebar");
  const text = element("span", "caption-title", title);
  text.setAttribute("data-tauri-drag-region", "");
  bar.append(text);
  if (desktopPlatform.customCaption) titlebar(bar);
  else bar.setAttribute("data-tauri-drag-region", "");
  document.body.prepend(bar);
  new MutationObserver(() => { text.textContent = document.title; }).observe(
    document.querySelector("title")!, { childList: true, characterData: true, subtree: true });
}
