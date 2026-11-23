import { element, report } from "./dom";
import "./file-name.css";

export const fileName = (path: string) => path.slice(path.lastIndexOf("/") + 1);

/** Escape or leaving the field cancels. Enter commits through the owning file operation. */
export function editFileName(anchor: HTMLElement, initial: string, create: boolean, commit: (name: string) => Promise<unknown>) {
  const field = element("div", "tree-name-field");
  const input = element("input");
  input.setAttribute("aria-label", create ? "新文档名称" : "重命名");
  input.value = initial;
  const error = element("span", "tree-name-error");
  error.setAttribute("role", "alert");
  field.append(input, error);
  anchor.after(field);
  if (!create) anchor.hidden = true;
  else field.classList.add("tree-name-new");
  let saving = false;
  function close(focus: boolean) {
    field.remove();
    if (!create) anchor.hidden = false;
    if (focus && anchor.isConnected) (create ? anchor.querySelector("button") : anchor)?.focus({ preventScroll: true });
  }
  input.addEventListener("input", () => { error.textContent = ""; input.removeAttribute("aria-invalid"); });
  input.addEventListener("blur", () => { if (!saving) close(false); });
  input.addEventListener("keydown", async event => {
    if (event.isComposing || (event.key !== "Enter" && event.key !== "Escape")) return;
    event.preventDefault();
    event.stopPropagation();
    if (saving) return;
    if (event.key === "Escape" || (!create && input.value.trim() === initial)) { close(true); return; }
    saving = true;
    input.readOnly = true;
    try {
      if (!input.value.trim()) throw new Error("请输入文件名");
      await commit(input.value.trim());
      close(!create);
    } catch (reason) {
      if (field.isConnected) {
        error.textContent = reason instanceof Error ? reason.message : String(reason);
        input.setAttribute("aria-invalid", "true");
        input.focus();
      } else report(reason);
    } finally { saving = false; input.readOnly = false; }
  });
  input.focus();
  const suffix = initial.match(/\.markdown$|\.md$/i);
  input.setSelectionRange(0, initial.length - (suffix?.[0].length ?? 0));
}
