import { localized } from "../i18n";
import { button, element } from "./dom";
import "../styles/dialog.css";

interface FileDialogOptions {
  initial?: string;
  allowEmpty?: boolean;
  detail?: string;
  signal?: AbortSignal;
}

export function fileDialog(title: string, action: string, { initial, allowEmpty, detail, signal }: FileDialogOptions = {}): Promise<string | undefined> {
  return new Promise(resolve => {
    const dialog = element("dialog", "sheet jot-dialog");
    const form = element("form");
    const heading = element("h2", undefined, title);
    const input = element("input");
    input.required = !allowEmpty;
    input.value = initial ?? "";
    const ok = element("button", "primary", action);
    ok.type = "submit";
    const buttons = element("div", "sheet-actions");
    buttons.append(button(localized("取消"), () => dialog.close()), ok);
    form.append(heading);
    if (initial !== undefined) {
      const label = element("label", "sheet-field");
      label.append(element("span", undefined, localized("名称")), input);
      form.append(label);
    }
    if (detail) form.append(element("p", "folder-location", detail));
    form.append(buttons);
    dialog.setAttribute("aria-label", title);
    dialog.append(form);
    let composing = false;
    let confirmingCandidate = false;
    input.addEventListener("compositionstart", () => { composing = true; });
    input.addEventListener("compositionend", () => {
      composing = false;
      confirmingCandidate = true;
      setTimeout(() => { confirmingCandidate = false; }, 0);
    });
    input.addEventListener("keydown", event => {
      if (event.key !== "Enter" || event.isComposing || event.keyCode === 229 || composing) return;
      event.preventDefault();
      if (!confirmingCandidate) form.requestSubmit();
    });
    dialog.addEventListener("cancel", event => { if (composing) event.preventDefault(); });
    form.addEventListener("submit", event => {
      event.preventDefault();
      if (!composing && !confirmingCandidate) dialog.close("submit");
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
