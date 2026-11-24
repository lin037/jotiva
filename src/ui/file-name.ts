import { t } from "../i18n";
import { desktopPlatform } from "../desktop/platform";
import { element, report } from "./dom";
import { fileDialog } from "./file-dialog";
import "./file-name.css";

export const fileName = (path: string) => path.slice(path.lastIndexOf(desktopPlatform.pathSeparator) + 1);

export function isDescendant(path: string, parent: string) {
  const separator = desktopPlatform.pathSeparator;
  return path.startsWith(parent.endsWith(separator) ? parent : parent + separator);
}

/** Keep the draft outside the file tree so a directory refresh cannot discard its name. */
export async function nameNewFile<T>(create: (name: string) => Promise<T>, open: (file: T) => unknown) {
  if (document.querySelector("dialog[open]")) return;
  let created: T | undefined;
  await fileDialog(t("新建文档"), t("创建"), {
    initial: t("未命名.md"),
    label: t("新文档名称"),
    commit: async value => { created = await create(value); },
  });
  if (created !== undefined) await open(created);
}

/** Escape or leaving the field cancels. Enter commits through the owning file operation. */
export function editFileName(anchor: HTMLElement, initial: string, commit: (name: string) => Promise<unknown>) {
  const field = element("div", "tree-name-field");
  const input = element("input");
  input.setAttribute("aria-label", t("重命名"));
  input.value = initial;
  const error = element("span", "tree-name-error");
  error.setAttribute("role", "alert");
  field.append(input, error);
  anchor.after(field);
  anchor.hidden = true;
  let saving = false;
  function close(focus: boolean) {
    field.remove();
    anchor.hidden = false;
    if (focus && anchor.isConnected) anchor.focus({ preventScroll: true });
  }
  input.addEventListener("input", () => { error.textContent = ""; input.removeAttribute("aria-invalid"); });
  input.addEventListener("blur", () => { if (!saving) close(false); });
  input.addEventListener("keydown", async event => {
    if (event.isComposing || (event.key !== "Enter" && event.key !== "Escape")) return;
    event.preventDefault();
    event.stopPropagation();
    if (saving) return;
    if (event.key === "Escape" || input.value.trim() === initial) { close(true); return; }
    saving = true;
    input.readOnly = true;
    try {
      if (!input.value.trim()) throw new Error(t("请输入文件名"));
      await commit(input.value.trim());
      close(true);
    } catch (reason) {
      if (field.isConnected) {
        error.textContent = reason instanceof Error ? reason.message : String(reason);
        input.setAttribute("aria-invalid", "true");
        input.focus();
      } else report(reason);
    } finally { saving = false; input.readOnly = false; }
  });
  input.focus();
  const suffix = initial.match(/\.(?:md|markdown|txt)$/i);
  input.setSelectionRange(0, initial.length - (suffix?.[0].length ?? 0));
}
