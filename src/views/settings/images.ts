import { invoke } from "@tauri-apps/api/core";
import { folders, imageSettings, type ImageConfig, type ImageMode, type ImageRule, type ImageUpload } from "../../desktop/ipc";
import { displayPath } from "../../desktop/platform";
import { bindLabel, localized, message, t, type Label } from "../../i18n";
import { button, disclosureSummary, element } from "../../ui/dom";
import { imageCheck, imageField, imageFolderLabel, imageFolderName, imageInput, imageModeDetail, imageModeName, imageModes,
  imagePreview, imageUploadFields } from "../../ui/image-controls";
import { manageFolderImages } from "../../ui/image-dialog";
import { isDescendant } from "../../ui/file-name";

export function mountImageSettings(page: HTMLElement) {
  const form = element("form", "image-settings-form");
  form.noValidate = true;
  form.hidden = true;
  const content = element("div");
  const defaults = element("section", "settings-group image-settings-defaults");
  const modeOptions = element("fieldset", "image-choice-group");
  modeOptions.append(element("legend", "image-section-title", localized("默认保存方式")));
  const choices = element("div", "image-choices");
  const modeButtons = new Map<ImageMode, HTMLInputElement>();
  for (const mode of imageModes) {
    const choice = imageInput("image-mode-" + mode, "radio");
    choice.name = "image-mode";
    choice.value = mode;
    choice.addEventListener("change", () => {
      draft.defaultMode = mode;
      otherMode = undefined;
      otherMethods.open = false;
      placeFields();
      changed();
    });
    modeButtons.set(mode, choice);
    choices.append(imageCheck(imageModeName[mode], choice, imageModeDetail[mode]));
  }
  modeOptions.append(choices);

  const subfolder = imageInput("image-subfolder");
  subfolder.placeholder = "_assets";
  subfolder.addEventListener("input", () => { draft.subfolder = subfolder.value.trim(); changed(); });
  const directory = imageInput("image-directory");
  directory.readOnly = true;
  const choose = button(localized("选择…"), async () => {
    const path = await folders.choose();
    if (path) {
      draft.directory = path;
      directory.value = displayPath(path);
      changed();
    }
  });
  const upload = imageUploadFields("image", value => { Object.assign(draft, value); changed(); });
  const fields = {
    relative: imageField(localized("图片子文件夹名称"), subfolder, localized("可填写多层子文件夹，例如 media/images。")),
    fixed: imageField(localized("图片目录"), directory, undefined, choose),
    http: upload.node,
  };
  const parameters = element("div", "image-parameters image-fields");
  const otherMethods = element("details", "image-other-methods");
  const otherChoices = element("div", "image-button-row");
  const otherParameters = element("div", "image-fields");
  let otherMode: ImageMode | undefined;
  const methodButtons = new Map(imageModes.map(mode => {
    const choice = button(imageModeName[mode], () => {
      otherMode = mode;
      placeFields();
    });
    otherChoices.append(choice);
    return [mode, choice] as const;
  }));
  otherMethods.append(
    disclosureSummary(localized("配置其他保存方式")),
    element("p", "image-help muted", localized("这里的参数供选择该方式的目录规则使用。若规则单独指定了本地图片位置，以规则为准。")),
    otherChoices, otherParameters,
  );

  const preview = element("p", "image-preview");
  const grouping = imageInput("image-group", "checkbox");
  grouping.addEventListener("input", () => { draft.groupByDocument = grouping.checked; changed(); });
  const compress = imageInput("image-compress", "checkbox");
  compress.addEventListener("input", () => { draft.compress = compress.checked; changed(); });
  const processing = element("div", "image-processing");
  processing.append(
    imageCheck(localized("按文档名创建子文件夹"), grouping, localized("仅在图片保存在本机时生效。")),
    imageCheck(localized("自动压缩静态图片"), compress, localized("减小静态 PNG、JPEG 的体积，保留原尺寸和动图。")),
  );
  defaults.append(modeOptions, parameters, preview, processing, otherMethods);

  const rules = element("section", "settings-group");
  const ruleRows = element("div", "image-rule-list");
  const ruleButtons = new Map<string, HTMLButtonElement>();
  const addRules = element("details", "image-add-rule");
  const addSummary = disclosureSummary(localized("添加目录规则"));
  addRules.append(addSummary);
  const folderChoices = element("div", "image-folder-choices");
  addRules.append(folderChoices, button(localized("浏览系统文件夹…"), async event => {
    const origin = event.currentTarget as HTMLElement;
    const path = await folders.choose();
    if (path) await editRule(path, origin);
  }));
  rules.append(
    element("h2", "image-section-title", localized("按目录单独设置")),
    element("p", "image-help muted", localized("子目录使用最近的上级规则。各级都没有规则时，使用全局默认方式。")),
    ruleRows, addRules,
  );

  const status = element("p", "image-help");
  status.setAttribute("role", "status");
  const error = element("p", "image-error");
  error.setAttribute("role", "alert");
  const discard = button(localized("放弃更改"), async () => {
    setDraft(await imageSettings.read());
  });
  const save = element("button", "primary", localized("保存设置"));
  save.type = "submit";
  const actions = element("div", "image-button-row");
  actions.append(discard, save);
  const footer = element("div", "image-save-bar");
  footer.append(error, status, actions);
  content.append(defaults, rules);
  form.append(content, footer);
  page.append(form);

  let saved: Awaited<ReturnType<typeof imageSettings.read>>;
  let draft: ImageConfig;
  let remoteUpdate = false;
  let saving = false;
  let loadNumber = 0;
  const dirty = () => JSON.stringify(draft) !== JSON.stringify(saved.config);

  function placeFields() {
    if (!otherMode || otherMode === draft.defaultMode) otherMode = imageModes.find(mode => mode !== draft.defaultMode);
    for (const mode of imageModes) {
      const choice = methodButtons.get(mode)!;
      const active = mode === draft.defaultMode;
      choice.hidden = active;
      choice.setAttribute("aria-pressed", String(mode === otherMode));
      fields[mode].hidden = !active && mode !== otherMode;
      (active ? parameters : otherParameters).append(fields[mode]);
    }
  }

  function changed(feedback = false) {
    const unsaved = dirty();
    save.disabled = saving || !unsaved || remoteUpdate;
    discard.hidden = !unsaved && !remoteUpdate;
    discard.disabled = saving;
    footer.toggleAttribute("data-dirty", unsaved);
    bindLabel(status, "text", remoteUpdate ? localized("图片设置已在别处更新，放弃当前更改可载入新设置。")
      : feedback ? localized("已保存") : unsaved ? localized("有未保存的更改") : "");
    imagePreview(preview, draft, draft.defaultMode);
  }

  function drawRules() {
    ruleButtons.clear();
    ruleRows.replaceChildren(...draft.rules.map(rule => {
      const row = button("", event => editRule(rule.path, event.currentTarget as HTMLElement), "image-folder-choice image-rule");
      ruleButtons.set(rule.path, row);
      row.tabIndex = 0;
      row.append(imageFolderLabel(rule.path), element("span", "image-help muted", imageModeName[rule.mode]));
      bindLabel(row, "aria-label", () => t("编辑 {name} 的图片规则", { name: imageFolderName(rule.path) }));
      return row;
    }));
  }

  function editRule(path: string, origin: HTMLElement) {
    return manageFolderImages(path, origin, {
      read: path => {
        const inherited = draft.rules.filter(rule => isDescendant(path, rule.path))
          .sort((a, b) => b.path.length - a.path.length)[0] ?? null;
        const effective = draft.rules.find(rule => rule.path === path) ?? inherited;
        return { config: draft, revision: saved.revision, folder: { path, effectiveRule: effective, inheritedRule: inherited } };
      },
      apply: async (path: string, rule: ImageRule | null, settings: ImageUpload | null, revision: number) => {
        if (remoteUpdate || saved.revision !== revision) throw new Error(t("图片设置已在别处更新，请重新载入后再保存。"));
        const index = draft.rules.findIndex(entry => entry.path === path);
        if (rule && index < 0) draft.rules.push(rule);
        else if (index >= 0) draft.rules.splice(index, 1, ...(rule ? [rule] : []));
        if (settings) {
          Object.assign(draft, settings);
          upload.set(draft);
        }
        addRules.open = false;
        drawRules();
        changed();
      },
      focusAfterClose: path => {
        (ruleButtons.get(path) ?? addSummary).focus({ preventScroll: true });
      },
    });
  }

  function setDraft(value: typeof saved, feedback = false) {
    saved = value;
    draft = structuredClone(value.config);
    remoteUpdate = false;
    modeButtons.get(draft.defaultMode)!.checked = true;
    directory.value = displayPath(draft.directory);
    subfolder.value = draft.subfolder;
    grouping.checked = draft.groupByDocument;
    compress.checked = draft.compress;
    upload.set(draft);
    placeFields();
    drawRules();
    error.textContent = "";
    form.hidden = false;
    changed(feedback);
  }

  form.addEventListener("input", () => { error.textContent = ""; });
  form.addEventListener("submit", async event => {
    event.preventDefault();
    if (!saved || saving || !dirty() || remoteUpdate) return;
    saving = content.inert = true;
    error.textContent = "";
    changed();
    try {
      setDraft(await imageSettings.save(draft, saved.revision), true);
    } catch (reason) {
      error.textContent = message(reason instanceof Error ? reason.message : String(reason));
      const current = await imageSettings.read().catch(() => undefined);
      remoteUpdate = current !== undefined && current.revision !== saved.revision;
    } finally {
      saving = content.inert = false;
      changed(error.textContent === "");
    }
  });

  return async () => {
    const turn = ++loadNumber;
    const [value, roots, archives, inbox] = await Promise.all([
      imageSettings.read(), invoke<string[]>("workspace_roots"), folders.list(), folders.inbox(),
    ]);
    if (turn !== loadNumber || saving || (saved && value.revision < saved.revision)) return;
    folderChoices.replaceChildren();
    const appendChoices = (title: "工作区" | "归档", entries: { path: string; name?: Label }[]) => {
      if (!entries.length) return;
      folderChoices.append(element("p", "image-help muted", localized(title)));
      for (const { path, name } of entries) {
        const choice = button("", event => editRule(path, event.currentTarget as HTMLElement), "image-folder-choice");
        choice.append(imageFolderLabel(path, name));
        folderChoices.append(choice);
      }
    };
    appendChoices("工作区", roots.map(path => ({ path })));
    appendChoices("归档", [{ path: inbox, name: localized("其他") }, ...archives]);
    if (!saved || !dirty()) setDraft(value);
    else if (value.revision !== saved.revision) {
      remoteUpdate = true;
      changed();
    }
  };
}
