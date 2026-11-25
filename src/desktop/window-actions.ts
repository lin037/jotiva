import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { finishDocumentNames } from "../ui/file-name";
import { report, root } from "../ui/dom";

export function saveBeforeClose(confirm: () => Promise<void>) {
  let closing = false;
  return getCurrentWindow().onCloseRequested(async event => {
    event.preventDefault();
    if (closing) return;
    closing = true;
    root.inert = true;
    try {
      await finishDocumentNames();
      await confirm();
    } catch (error) {
      await invoke("cancel_quit");
      report(error);
    } finally {
      root.inert = false;
      closing = false;
    }
  });
}
