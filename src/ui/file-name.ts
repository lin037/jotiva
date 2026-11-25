import { t, bindLabel } from "../i18n";
import { desktopPlatform, fileName } from "../desktop/platform";
import { element, report } from "./dom";
import { invoke } from "@tauri-apps/api/core";
import "./file-name.css";

export { fileName } from "../desktop/platform";

export function isDescendant(path: string, parent: string) {
  const separator = desktopPlatform.pathSeparator;
  return path.startsWith(parent.endsWith(separator) ? parent : parent + separator);
}

let creating = false;
let lastCreation = -Infinity;

/** Coalesce key repeat, double-click and a simultaneous native menu command. */
export async function createInline(create: () => Promise<void>) {
  if (creating || performance.now() - lastCreation < 300) return;
  lastCreation = performance.now();
  creating = true;
  try { await create(); } finally { creating = false; }
}

const delayedRefresh = new WeakMap<Element, Map<HTMLElement, () => void>>();
const unfinishedNames = new Set<() => Promise<void>>();

export function finishDocumentNames() {
  return Promise.all([...unfinishedNames].map(save => save()));
}

export function afterNaming(container: HTMLElement, refresh: () => void) {
  const field = container.querySelector(".tree-name-field");
  if (!field) return false;
  let pending = delayedRefresh.get(field);
  if (!pending) {
    delayedRefresh.set(field, pending = new Map());
    field.addEventListener("naming-finished", () => pending?.forEach(run => run()), { once: true });
  }
  pending.set(container, refresh);
  return true;
}

export function nameCreatedDocument(row: HTMLElement, id: number, path = row.dataset.file!) {
  const previousFocus = document.activeElement;
  editFileName(row, fileName(path), async name => {
    const adjusted = await invoke<string | null>("name_created_document", { id, name });
    if (adjusted) {
      const notice = element("div", "file-name-notice", () => t("文件名已存在，已保存为 {name}", { name: adjusted }));
      notice.setAttribute("role", "status");
      document.body.append(notice);
      setTimeout(() => notice.remove(), 4500);
    }
  }, previousFocus instanceof HTMLElement ? previousFocus : row);
}

export function editFileName(anchor: HTMLElement, initial: string, commit: (name: string) => Promise<unknown>, returnTo?: HTMLElement) {
  const created = !!returnTo;
  const field = element("div", "tree-name-field");
  const input = element("input");
  bindLabel(input, "aria-label", () => created ? t("新文档名称") : t("重命名"));
  input.value = initial;
  const error = element("span", "tree-name-error");
  error.setAttribute("role", "alert");
  field.append(input, error);
  anchor.after(field);
  anchor.hidden = true;
  let closed = false;
  let pending: Promise<void> | undefined;
  const saveOnClose = () => finish(false);
  if (created) unfinishedNames.add(saveOnClose);
  function close(focus: boolean) {
    closed = true;
    unfinishedNames.delete(saveOnClose);
    clearTimeout(blurTimer);
    field.remove();
    anchor.hidden = false;
    field.dispatchEvent(new Event("naming-finished"));
    const target = returnTo ?? anchor;
    if (focus && target.isConnected) target.focus({ preventScroll: true });
  }
  input.addEventListener("input", () => { error.textContent = ""; input.removeAttribute("aria-invalid"); });
  let composing = false;
  let confirming = false;
  let blurTimer = 0;
  const finishBlur = () => {
    clearTimeout(blurTimer);
    // Some input methods omit compositionend on blur. Give the final input event
    // time to arrive and cancel the pending save if the candidate UI restores focus.
    blurTimer = window.setTimeout(() => {
      if (document.activeElement !== input && !closed) void finish(false);
    }, composing ? 500 : 120);
  };
  input.addEventListener("focus", () => clearTimeout(blurTimer));
  input.addEventListener("compositionstart", () => { composing = true; });
  input.addEventListener("compositionend", () => {
    composing = false;
    confirming = true;
    setTimeout(() => { confirming = false; }, 0);
    if (document.activeElement !== input) finishBlur();
  });
  input.addEventListener("blur", () => {
    if (pending || closed) return;
    if (!created) close(false);
    else finishBlur();
  });
  function finish(focus: boolean): Promise<void> {
    if (closed) return Promise.resolve();
    return pending ??= save(focus).finally(() => { pending = undefined; });
  }
  async function save(focus: boolean) {
    const value = input.value.trim();
    if (value === initial || (created && !value)) { close(focus); return; }
    input.readOnly = true;
    try {
      if (!value) throw new Error(t("请输入文件名"));
      await commit(value);
      close(focus && document.activeElement === input);
    } catch (reason) {
      if (focus && field.isConnected) {
        error.textContent = reason instanceof Error ? reason.message : String(reason);
        input.setAttribute("aria-invalid", "true");
      } else { close(false); report(reason); }
    } finally { input.readOnly = false; }
  }
  input.addEventListener("keydown", event => {
    if (event.isComposing || event.keyCode === 229 || composing || confirming || !["Enter", "Escape"].includes(event.key)) return;
    event.preventDefault();
    event.stopPropagation();
    if (pending || closed) return;
    clearTimeout(blurTimer);
    if (event.key === "Escape") close(true);
    else void finish(true);
  });
  field.scrollIntoView({ block: "nearest" });
  input.focus();
  const suffix = initial.match(/\.(?:md|markdown|txt)$/i);
  input.setSelectionRange(0, initial.length - (suffix?.[0].length ?? 0));
}
