import { t } from "../../i18n";
import { displayPath } from "../../desktop/platform";
import { fileName } from "../../ui/file-name";
import { folders, type Folder, type NoteId } from "../../desktop/ipc";
import { button, element, report } from "../../ui/dom";

export function folderEditor(onSaved: (folder: Folder) => void) {
  const dialog = element("dialog", "sheet jot-dialog");
  const form = element("form");
  const heading = element("h2");
  const name = element("input");
  name.required = true;
  name.maxLength = 72;
  const description = element("textarea");
  description.rows = 3;
  description.maxLength = 400;
  const path = element("input");
  path.required = true;
  const place = element("div", "path-field");
  place.append(path, button(t("选择…"), async () => {
    const choice = await folders.choose();
    if (choice) {
      path.value = choice;
      if (!name.value) name.value = fileName(choice).slice(0, 72);
    }
  }));
  const classify = element("input");
  classify.type = "checkbox";
  const option = element("label", "sheet-check");
  option.append(classify, t("参与 Jev 分类"));
  const notice = element("p", "form-message");
  notice.setAttribute("role", "status");
  const save = element("button", "primary", t("保存文件夹"));
  save.type = "submit";
  const cancel = button(t("取消"), () => dialog.close());
  const actions = element("div", "sheet-actions");
  actions.append(cancel, save);
  function field(title: string, control: HTMLElement) {
    const label = element("label", "sheet-field");
    label.append(element("span", undefined, title), control);
    return label;
  }
  form.append(heading, field(t("路径"), place), field(t("名称"), name), field(t("用途说明"), description), option, notice, actions);
  dialog.append(form);
  document.body.append(dialog);
  let id: number | null = null;
  form.addEventListener("submit", async event => {
    event.preventDefault();
    save.disabled = cancel.disabled = true;
    notice.textContent = "";
    try {
      const folder = await folders.save({ id, name: name.value, description: description.value, path: path.value, classify: classify.checked });
      dialog.close();
      onSaved(folder);
    } catch (error) {
      notice.textContent = String(error);
    } finally {
      save.disabled = cancel.disabled = false;
    }
  });
  dialog.addEventListener("cancel", event => { if (save.disabled) event.preventDefault(); });
  return async (folder?: Folder) => {
    const directory = folder?.path ?? await folders.choose().catch(report);
    if (!directory) return;
    id = folder?.id ?? null;
    heading.textContent = folder ? t("归档目录设置") : t("导入归档目录");
    name.value = folder?.name ?? fileName(directory);
    classify.checked = folder?.classify ?? false;
    description.value = folder?.description ?? "";
    path.value = directory;
    notice.textContent = "";
    dialog.showModal();
    name.focus();
  };
}

export function moveDocument(
  onMoved: (result: { path: string | null; warning: string | null }, folder: number | null) => void,
  addFolder: () => void,
) {
  const dialog = element("dialog", "sheet jot-dialog");
  const form = element("form");
  const label = element("label", "sheet-field");
  const select = element("select");
  label.append(element("span", undefined, t("目标文件夹")), select);
  const path = element("p", "folder-location muted");
  const notice = element("p", "form-message");
  notice.setAttribute("role", "status");
  const actions = element("div", "sheet-actions");
  const save = element("button", "primary", t("移动"));
  save.type = "submit";
  const cancel = button(t("取消"), () => dialog.close());
  const add = button(t("添加文件夹…"), () => { dialog.close(); addFolder(); });
  actions.append(add, cancel, save);
  form.append(element("h2", undefined, t("移动便签")), label, path, notice, actions);
  dialog.append(form);
  document.body.append(dialog);
  let id: NoteId;
  let current: number | null = null;
  let choices: Folder[] = [];
  let inbox = "";
  function target() {
    return select.value ? Number(select.value) : null;
  }
  function update() {
    const choice = choices.find(folder => folder.id === target());
    path.textContent = choice ? [choice.description, displayPath(choice.path)].filter(Boolean).join("\n") : displayPath(inbox);
    save.disabled = target() === current;
  }
  select.addEventListener("change", update);
  form.addEventListener("submit", async event => {
    event.preventDefault();
    save.disabled = cancel.disabled = select.disabled = add.disabled = true;
    notice.textContent = "";
    try {
      const destination = target();
      const moved = await folders.move(id, destination);
      dialog.close();
      onMoved(moved, destination);
    } catch (error) {
      notice.textContent = String(error);
    } finally {
      cancel.disabled = select.disabled = add.disabled = false;
      update();
    }
  });
  dialog.addEventListener("cancel", event => { if (select.disabled) event.preventDefault(); });
  return async (note: NoteId, location: number | null, destination = location) => {
    id = note;
    current = location;
    [choices, inbox] = await Promise.all([folders.list(), folders.inbox()]);
    const other = element("option", undefined, t("其他"));
    other.value = "";
    select.replaceChildren(other, ...choices.map(folder => {
      const option = element("option", undefined, folder.name);
      option.value = String(folder.id);
      return option;
    }));
    select.value = destination === null ? "" : String(destination);
    notice.textContent = "";
    update();
    dialog.showModal();
    select.focus();
  };
}
