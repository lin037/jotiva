import { t } from "../i18n";
import { invoke } from "@tauri-apps/api/core";
import { report } from "../ui/dom";

// Read semantic regions on demand, so newly mounted tabs, panes and windows participate immediately.
const regionSelector = 'main, nav[aria-label], aside[aria-labelledby], [role="tablist"], [role="search"], [role="region"][aria-label], header.jot-titlebar, footer.jot-statusbar';
const controls = 'button:enabled, input:enabled, textarea:enabled, select:enabled, [contenteditable="true"], [tabindex="0"]';
function visible(node: HTMLElement) {
  return node.checkVisibility() && !node.closest("[inert], [aria-hidden=true]");
}
function target(region: HTMLElement) {
  const candidates = Array.from(region.querySelectorAll<HTMLElement>(controls))
    .filter(node => node.closest(regionSelector) === region && visible(node));
  return candidates.find(node => node.isContentEditable || node.matches('input[type="search"], [aria-current="true"], [aria-selected="true"]'))
    ?? candidates[0] ?? region;
}
function name(region: HTMLElement) {
  return region.getAttribute("aria-label") || document.getElementById(region.getAttribute("aria-labelledby") || "")?.textContent
    || ({ MAIN: t("正文"), HEADER: t("工具栏"), FOOTER: t("状态栏") }[region.tagName]) || t("标签页");
}
export function bindFocusRegions() {
  window.addEventListener("keydown", event => {
    if (event.code !== "F6" || event.repeat || event.isComposing || event.metaKey || event.ctrlKey || event.defaultPrevented
      || document.querySelector('dialog[open], [popover]:not(.jot-tooltip):popover-open, .recording-key[aria-pressed="true"]')) return;
    const regions = Array.from(document.querySelectorAll<HTMLElement>(regionSelector)).filter(visible);
    if (!regions.length) return;
    event.preventDefault();
    event.stopPropagation();
    const visit = (index: number) => {
      const region = regions[index];
      if (!region?.isConnected || !visible(region)) return;
      const next = target(region);
      if (next === region && !region.hasAttribute("tabindex")) region.tabIndex = -1;
      next.focus({ preventScroll: true });
    };
    if (event.altKey) {
      const menu = regions.map((region, i) => `${i + 1}  ${name(region)}`);
      void invoke<number | null>("context_menu", { labels: menu, x: Math.round(innerWidth / 2), y: 40 })
        .then(index => { if (index !== null) visit(index); }).catch(report);
      return;
    }
    const active = document.activeElement?.closest<HTMLElement>(regionSelector);
    const index = active ? regions.indexOf(active) : -1;
    visit(index < 0 ? (event.shiftKey ? regions.length - 1 : 0)
      : (index + (event.shiftKey ? -1 : 1) + regions.length) % regions.length);
  }, true);
}
