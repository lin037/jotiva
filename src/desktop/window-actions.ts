import { desktopPlatform } from "./platform";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { report, root } from "../ui/dom";

export function saveBeforeClose(confirm: () => Promise<void>) {
  let closing = false;
  return getCurrentWindow().onCloseRequested(async event => {
    event.preventDefault();
    if (closing) return;
    closing = true;
    root.inert = true;
    try {
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

export async function bindNewDocumentDialog() {
  if (desktopPlatform.nativeNameDialog) return;
  await getCurrentWindow().listen("desktop:name-document", async () => {
    if (document.querySelector("dialog[open]")) return;
    try {
      const { fileDialog } = await import("../ui/file-dialog");
      const name = await fileDialog("新建文档窗口", "创建", "未命名");
      if (!name) return;
      const id = await invoke<number>("create_archive_note", { folder: null, name });
      await invoke("open_document_window", { id });
    } catch (error) { report(error); }
  });
}
