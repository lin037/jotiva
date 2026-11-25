import { bindLabel, type Label } from "../i18n";

export const root = document.querySelector<HTMLElement>("#app")!;

const notice = document.createElement("div");
notice.className = "error";
notice.setAttribute("role", "alert");
notice.hidden = true;
document.body.append(notice);

export function report(error: unknown) {
  notice.textContent = error instanceof Error ? error.message : String(error);
  notice.hidden = false;
}

export function clearError() {
  notice.hidden = true;
}

export function element<K extends keyof HTMLElementTagNameMap>(tag: K, className?: string, text?: Label): HTMLElementTagNameMap[K] {
  const result = document.createElement(tag);
  if (className) result.className = className;
  if (tag === "button") result.classList.add("jot-control", "jot-button");
  if (["input", "textarea", "select"].includes(tag)) result.classList.add("jot-control", "jot-field");
  if (text) bindLabel(result, "text", text);
  return result;
}

export function button(label: Label, action: () => unknown, className?: string, tip?: Label) {
  const control = element("button", className, label);
  control.type = "button";
  if (tip) bindLabel(control, "data-tip", tip);
  control.addEventListener("click", async () => {
    clearError();
    try {
      await action();
    } catch (error) {
      report(error);
    }
  });
  return control;
}
