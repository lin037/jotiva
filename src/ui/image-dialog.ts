import { folders, imageSettings, type ImageConfig, type ImageMode, type ImageRule, type ImageUpload } from "../desktop/ipc";
import { displayPath } from "../desktop/platform";
import { bindLabel, localized, message, t, type Label } from "../i18n";
import { button, disclosureSummary, element } from "./dom";
import { imageField, imageFolderLabel, imageFolderName, imageInput, imageModeDetail, imageModeName, imageModes,
  imagePreview, imageRuleFields, imageUploadFields } from "./image-controls";
import "../styles/dialog.css";

type Snapshot = Awaited<ReturnType<typeof imageSettings.read>>;
interface FolderRuleDraft {
  read: (path: string) => Snapshot;
  apply: typeof imageSettings.saveFolder;
  focusAfterClose: (path: string) => void;
}

export async function manageFolderImages(path: string, origin: HTMLElement, draft?: FolderRuleDraft) {
  if (document.querySelector(".image-folder-dialog[open]")) return;
  const dialog = element("dialog", "sheet jot-dialog image-folder-dialog");
  const form = element("form");
  form.noValidate = true;
  const title = element("h2", undefined, localized("此文件夹的图片保存方式"));
  title.id = "image-folder-dialog-title";
  dialog.setAttribute("aria-labelledby", title.id);
  const body = element("div", "image-dialog-body");
  const context = element("p", "image-help image-current");
  const error = element("p", "image-error");
  error.setAttribute("role", "alert");
  const saveHint = element("p", "image-help muted", localized("应用后，点击图片管理页的「保存设置」使规则生效。"));
  const actions = element("div", "sheet-actions");
  const cancel = button(localized("取消"), () => dialog.close());
  const back = button(localized("返回"), () => goBack?.());
  const reload = button(localized("重新载入"), () => load());
  const save = element("button", "primary", localized("应用"));
  save.type = "submit";
  actions.append(back, cancel, reload, save);
  form.append(title, imageFolderLabel(path), context, body, error, actions);
  if (draft) actions.before(saveHint);
  dialog.append(form);
  document.body.append(dialog);
  dialog.addEventListener("close", () => {
    dialog.remove();
    requestAnimationFrame(() => {
      if (draft && dialog.returnValue === "applied") draft.focusAfterClose(path);
      else origin.focus({ preventScroll: true });
    });
  }, { once: true });
  dialog.addEventListener("cancel", event => { if (saving) event.preventDefault(); });

  let snapshot: Snapshot;
  let goBack: (() => void) | undefined;
  let selection: ImageRule | null = null;
  let upload: (() => ImageUpload) | undefined;
  let saving = false;
  let stale = false;

  function show(content: HTMLElement[], previous?: () => void, commit = false) {
    body.replaceChildren(...content);
    error.textContent = "";
    goBack = previous;
    back.hidden = !previous;
    bindLabel(back, "text", localized(draft && previous === ruleSources ? "规则选项" : "返回"));
    save.hidden = !commit;
    saveHint.hidden = !commit;
    reload.hidden = true;
    save.disabled = false;
    (body.querySelector<HTMLElement>("button[data-current]")
      ?? body.querySelector<HTMLElement>("select, input, button, summary")
      ?? cancel).focus();
  }

  function option(title: Label, detail: Label, selected: boolean, action: () => void) {
    const choice = button("", action, "image-dialog-choice");
    choice.append(element("strong", undefined, title), element("small", "image-help muted", detail));
    if (selected) {
      choice.dataset.current = "";
      choice.append(element("small", "image-current", localized("当前")));
    }
    return choice;
  }

  function modes() {
    const { config, folder } = snapshot;
    const current = folder!.effectiveRule;
    const grid = element("div", "image-choices");
    for (const mode of imageModes) {
      const active = !current && config.defaultMode === mode;
      grid.append(option(imageModeName[mode], imageModeDetail[mode], active, () => configure(ruleFor(mode), modes)));
    }
    grid.append(option(localized("目录规则"), localized("套用已有规则，或为这个文件夹自定义。"), !!current, ruleSources));
    show([grid]);
  }

  function ruleFor(mode: ImageMode): ImageRule {
    return { path, mode, directory: "", subfolder: "" };
  }

  function customize(previous?: () => void) {
    const current = snapshot.folder!.effectiveRule;
    configure(current ? { ...current, path } : ruleFor(snapshot.config.defaultMode), previous, true);
  }

  function ruleSources() {
    const { config, folder } = snapshot;
    const choices = element("div", "image-fields");
    const current = folder!.effectiveRule;
    const sources = config.rules.filter(rule => rule.path !== path);
    if (!sources.length && current?.path !== path) {
      customize(draft ? undefined : modes);
      return;
    }
    choices.append(button(localized(current?.path === path ? "编辑当前规则" : "立即自定义"), () => customize(ruleSources)));
    if (sources.length) {
      const useRule = element("details", "image-other-methods");
      useRule.append(disclosureSummary(localized("选择已有规则")),
        element("p", "image-help muted", localized("套用后可调整参数和保存。之后两条规则各自独立。")));
      for (const rule of sources) {
        const pick = button("", () => configure({ ...rule, path }, ruleSources, true), "image-folder-choice");
        pick.append(imageFolderLabel(rule.path), element("small", "image-help muted", imageModeName[rule.mode]));
        useRule.append(pick);
      }
      choices.append(useRule);
    }
    if (current?.path === path) {
      choices.append(button(localized("恢复继承"), () => configure(null, ruleSources)));
    }
    show([choices], draft ? undefined : modes);
  }

  function configure(rule: ImageRule | null, previous?: () => void, custom = false) {
    selection = rule;
    const { folder } = snapshot;
    const config: ImageConfig = { ...snapshot.config };
    const inherited = folder!.inheritedRule;
    const fields = element("div", "image-fields");
    const preview = element("p", "image-preview");
    const heading = element("h3", "image-section-title", () =>
      rule ? imageModeName[rule.mode]() : t("恢复继承"));
    const uploadFields = imageUploadFields("folder-image", value => { Object.assign(config, value); updatePreview(); }, draft ? undefined : path);
    const uploadPanel = element("details", "image-other-methods");
    uploadPanel.open = !config.endpoint;
    uploadPanel.append(
      disclosureSummary(localized("上传服务设置")),
      element("p", "image-help muted", localized("此上传服务也用于其他选择 HTTP 上传的文档。")),
      uploadFields.node,
    );
    upload = rule ? uploadFields.read : undefined;

    let fixed: HTMLElement | undefined;
    if (custom && rule) fields.append(imageRuleFields("folder-image-rule", rule, updatePreview));
    else if (rule?.mode === "fixed") {
      const input = imageInput("folder-image-directory");
      input.readOnly = true;
      input.value = displayPath(config.directory);
      const choose = button(localized("选择…"), async () => {
        const directory = await folders.choose();
        if (directory) {
          rule.directory = directory;
          input.value = displayPath(directory);
          updatePreview();
        }
      });
      fixed = imageField(localized("图片目录"), input, undefined, choose);
      fields.append(fixed);
    } else if (!rule) {
      fields.append(element("p", "image-help muted", () => inherited
        ? t("将使用上级目录 {name} 的规则。", { name: imageFolderName(inherited.path) })
        : t("将使用全局默认方式。")));
    }

    function updatePreview() {
      const applied = rule ?? inherited;
      imagePreview(preview, config, applied?.mode ?? config.defaultMode, applied);
      uploadPanel.hidden = rule?.mode !== "http";
      if (fixed) fixed.hidden = rule?.mode !== "fixed";
    }
    fields.append(uploadPanel, preview);
    updatePreview();
    show(custom ? [fields] : [heading, fields], previous, true);
    uploadFields.set(config);
  }

  async function load() {
    stale = false;
    show([element("p", "image-help muted", localized("正在读取图片设置…"))]);
    try {
      const value = draft ? draft.read(path) : await imageSettings.read(path);
      if (!dialog.open) return;
      if (!value.folder) throw new Error(t("无法读取文件夹的图片设置。"));
      snapshot = value;
      path = value.folder.path;
      const rule = value.folder.effectiveRule;
      bindLabel(context, "text", () => {
        const mode = imageModeName[rule?.mode ?? value.config.defaultMode]();
        if (!rule) return t("当前方式：{mode}（全局默认）", { mode });
        if (rule.path === path) return t("当前方式：{mode}（此文件夹的规则）", { mode });
        return t("当前方式：{mode}（继承自 {name}）", { mode, name: imageFolderName(rule.path) });
      });
      if (draft) customize(value.config.rules.length ? ruleSources : undefined);
      else modes();
    } catch (reason) {
      error.textContent = message(reason instanceof Error ? reason.message : String(reason));
      reload.hidden = false;
    }
  }

  form.addEventListener("submit", async event => {
    event.preventDefault();
    if (save.hidden || saving || stale) return;
    const missing = form.querySelector<HTMLInputElement>("input:required:invalid");
    if (missing) {
      error.textContent = t("请选择图片存放目录。");
      missing.focus();
      return;
    }
    saving = body.inert = true;
    for (const control of [save, cancel, back]) control.disabled = true;
    error.textContent = "";
    try {
      await (draft?.apply ?? imageSettings.saveFolder)(path, selection, selection?.mode === "http" ? upload?.() ?? null : null, snapshot.revision);
      dialog.close("applied");
    } catch (reason) {
      error.textContent = message(reason instanceof Error ? reason.message : String(reason));
      if (!draft) {
        const current = await imageSettings.read(path).catch(() => undefined);
        stale = current !== undefined && current.revision !== snapshot.revision;
      }
      reload.hidden = !stale;
    } finally {
      saving = body.inert = false;
      cancel.disabled = false;
      back.disabled = save.disabled = stale;
    }
  });
  dialog.showModal();
  await load();
}
