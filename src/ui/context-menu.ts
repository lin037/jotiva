import { invoke } from "@tauri-apps/api/core";
import { clearError, report } from "./dom";

type Item = { label: string; run: () => unknown };

export async function popupMenu(event: MouseEvent, items: (Item | null)[]) {
  event.preventDefault();
  event.stopPropagation();
  clearError();
  try {
    const anchor = event.detail === 0 && event.clientX === 0 && event.clientY === 0 && event.currentTarget instanceof Element
      ? event.currentTarget.getBoundingClientRect() : undefined;
    const selected = await invoke<number | null>("context_menu", {
      labels: items.map(item => item?.label ?? null),
      x: anchor?.left ?? event.clientX,
      y: anchor?.bottom ?? event.clientY,
    });
    if (selected !== null) await items[selected]?.run();
  } catch (error) {
    report(error);
  }
}

export function copyText(text: string) {
  return invoke<void>("copy_text", { text });
}
