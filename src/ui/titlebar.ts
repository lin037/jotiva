import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { desktopPlatform } from "../desktop/platform";
import { dragOnMouseDown, icon, iconButton } from "./chrome";
import { element, report } from "./dom";
import "./titlebar.css";

function appMenu() {
  const button = iconButton("menu", "应用菜单", () => {
    const { left, bottom } = button.getBoundingClientRect();
    return invoke("application_menu", { x: left, y: bottom });
  });
  button.setAttribute("aria-haspopup", "menu");
  return button;
}

function captionButton(action: "close" | "minimize" | "maximize", label: string, click: () => Promise<void>) {
  const button = iconButton(action, label, click);
  button.classList.add("caption-button");
  button.dataset.caption = action;
  button.title = label;
  return button;
}

export function titlebar(region: HTMLElement, compact = false) {
  if (!desktopPlatform.customCaption || compact) dragOnMouseDown(region);
  if (!desktopPlatform.customCaption) return;
  if (!compact) region.setAttribute("data-tauri-drag-region", "");
  const win = getCurrentWindow();
  const controls = element("div", "caption-controls");
  controls.setAttribute("role", "group");
  controls.setAttribute("aria-label", "窗口控制");
  if (!compact) {
    const maximize = captionButton("maximize", "最大化", () => win.toggleMaximize());
    const sync = async () => {
      const maximized = await win.isMaximized();
      const label = maximized ? "还原窗口" : "最大化";
      maximize.title = label;
      maximize.setAttribute("aria-label", label);
      maximize.replaceChildren(icon(maximized ? "restore" : "maximize"));
    };
    void sync().catch(report);
    void win.onResized(() => void sync().catch(report)).catch(report);
    controls.append(captionButton("minimize", "最小化", () => win.minimize()), maximize);
    const leading = region.querySelector(".workspace-leading");
    if (leading) leading.prepend(appMenu());
    else region.prepend(appMenu());
  }
  controls.append(captionButton("close", "关闭窗口", () => win.close()));
  region.append(controls);
}

export function secondaryTitlebar(title: string) {
  if (!desktopPlatform.customCaption) return;
  const bar = element("header", "secondary-titlebar jot-titlebar");
  const text = element("span", "caption-title", title);
  text.setAttribute("data-tauri-drag-region", "");
  bar.append(text);
  titlebar(bar);
  document.body.prepend(bar);
  new MutationObserver(() => { text.textContent = document.title; }).observe(
    document.querySelector("title")!, { childList: true, characterData: true, subtree: true });
}
