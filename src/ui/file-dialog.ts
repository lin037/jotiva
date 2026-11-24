import { t } from "../i18n";
import { button, element } from "./dom";
import "../styles/dialog.css";

interface FileDialogOptions {
  initial?: string;
  detail?: string;
  signal?: AbortSignal;
  label?: string;
  commit?: (name: string) => Promise<unknown>;
}

export function fileDialog(title: string, action: string, { initial, detail, signal, label, commit }: FileDialogOptions = {}): Promise<string | undefined> {
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
    buttons.append(button(t("取消"), () => dialog.close()), ok);
    form.append(heading);
    if (initial !== undefined) {
      const label = element("label", "sheet-field");
      label.append(element("span", undefined, t("名称")), input);
      form.append(label);
    }
    if (detail) form.append(element("p", "folder-location", detail));
    const error = element("p", "form-message");
    error.setAttribute("role", "alert");
    error.hidden = true;
    form.append(error, buttons);
    if (label) input.setAttribute("aria-label", label);
    dialog.setAttribute("aria-label", title);
    dialog.append(form);
    let busy = false;
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
    input.addEventListener("input", () => { error.hidden = true; input.removeAttribute("aria-invalid"); });
    dialog.addEventListener("cancel", event => { if (busy || composing) event.preventDefault(); });
    form.addEventListener("submit", async event => {
      event.preventDefault();
      if (composing || confirmingCandidate || busy) return;
      busy = true;
      try {
        if (commit) {
          const name = input.value.trim();
          if (!name) throw new Error(t("请输入文件名"));
          buttons.inert = input.readOnly = true;
          await commit(name);
        }
        dialog.close("submit");
      } catch (reason) {
        error.textContent = reason instanceof Error ? reason.message : String(reason);
        error.hidden = false;
        input.setAttribute("aria-invalid", "true");
        input.focus();
      } finally {
        busy = buttons.inert = input.readOnly = false;
      }
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
