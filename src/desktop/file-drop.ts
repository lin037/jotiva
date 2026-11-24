import { desktopPlatform } from "./platform";
import { invoke } from "@tauri-apps/api/core";
import { report } from "../ui/dom";

export function bindFileDrop() {
  if (!desktopPlatform.nativePasteboard) return;
  const markdown = /\.(md|markdown|txt)$/iu;
  document.addEventListener("dragover", event => {
    if (event.dataTransfer?.types.includes("Files") && !document.querySelector("dialog[open]")) event.preventDefault();
  }, true);
  document.addEventListener("drop", event => {
    const files = Array.from(event.dataTransfer?.files ?? []);
    const names = files.map(file => file.name).filter(name => markdown.test(name));
    if (!names.length || document.querySelector("dialog[open]")) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    void invoke("open_dropped_markdown", { names }).catch(report);
  }, true);
}
