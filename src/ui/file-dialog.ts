import { button, element } from "./dom";
import "../styles/dialog.css";

/** Name and deletion sheets exist only while the corresponding action is active. */
export function fileDialog(title: string, action: string, initial?: string, detail?: string, signal?: AbortSignal): Promise<string | undefined> {
  return new Promise(resolve => {
    const dialog = element("dialog", "sheet jot-dialog");
    const form = element("form");
    const heading = element("h2", undefined, title);
    const input = element("input");
    input.required = true;
    input.value = initial ?? "";
    const ok = element("button", "primary", action);
    ok.type = "submit";
    const buttons = element("div", "sheet-actions");
    buttons.append(button("取消", () => dialog.close()), ok);
    form.append(heading);
    if (initial !== undefined) {
      const label = element("label", "sheet-field");
      label.append(element("span", undefined, "名称"), input);
      form.append(label);
    }
    if (detail) form.append(element("p", "folder-location", detail));
    form.append(buttons);
    dialog.setAttribute("aria-label", title);
    dialog.append(form);
    form.addEventListener("submit", event => {
      event.preventDefault();
      dialog.close("submit");
    });
    const dismiss = () => dialog.close();
    dialog.addEventListener("close", () => {
      signal?.removeEventListener("abort", dismiss);
      dialog.remove();
      resolve(dialog.returnValue === "submit" ? input.value.trim() : undefined);
    }, { once: true });
    document.body.append(dialog);
    dialog.showModal();
    signal?.addEventListener("abort", dismiss, { once: true });
    if (signal?.aborted) { dismiss(); return; }
    if (initial !== undefined) {
      input.focus();
      const dot = initial.lastIndexOf(".");
      input.setSelectionRange(0, dot > 0 ? dot : initial.length);
    } else {
      buttons.querySelector("button")?.focus();
    }
  });
}
