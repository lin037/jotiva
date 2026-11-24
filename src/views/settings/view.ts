import { t, message, type Label, bindLabel, localized } from "../../i18n";
import { secondaryTitlebar } from "../../ui/titlebar";
import { desktopPlatform } from "../../desktop/platform";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { accentPreset, accentPresets } from "../../appearance/accent";
import { currentAppearance, defaultHeadingScale, onAppearanceChange, saveAppearance, setStylesheet, type AppearancePreferences, type EditingPreferences, type HeadingScale, type ThemeMode, type WritingFont } from "../../appearance/preferences";
import { commands, effectiveShortcut, fixedShortcuts, globalActions, recordedShortcut, shortcutLabel, type GlobalBindings, type GlobalAction } from "../../desktop/shortcuts";
import { events } from "../../desktop/ipc";
import { accessibilityHelp, watchAccessibility } from "../../desktop/window-access";
import { icon } from "../../ui/chrome";
import { button, element, report, root } from "../../ui/dom";
import "./settings.css";

type Section = "appearance" | "editor" | "filing" | "shortcuts" | "windows";

interface SettingsView {
  preferences: GlobalBindings;
  defaults: GlobalBindings;
  trusted: boolean;
  shortcutError: string | null;
}

const sections = [
  ["appearance", localized("外观"), "appearance"],
  ["editor", localized("编辑器"), "format"],
  ["filing", localized("归档"), "folder"],
  ["shortcuts", localized("快捷键"), "keys"],
  ["windows", localized("窗口关联"), "window"],
] as const;

function field(name: Label, controls: HTMLElement | HTMLElement[], id?: string) {
  const row = element("div", "settings-field");
  const label = id ? element("label", undefined, name) : element("span", undefined, name);
  const items = element("div", "settings-field-controls");
  if (id) label.setAttribute("for", id);
  items.append(...(Array.isArray(controls) ? controls : [controls]));
  row.append(label, items);
  return row;
}

function select<T extends string>(id: string, options: readonly (readonly [T, Label])[], changed: (value: T) => unknown) {
  const input = element("select");
  input.id = id;
  for (const [value, text] of options) {
    const option = element("option", undefined, text);
    option.value = value;
    input.append(option);
  }
  input.addEventListener("change", () => { void Promise.resolve().then(() => changed(input.value as T)).catch(report); });
  return input;
}

type EditingPatch = Partial<EditingPreferences> | ((value: EditingPreferences) => Partial<EditingPreferences>);

function mountEditorSettings(page: HTMLElement, change: (patch: EditingPatch) => Promise<unknown>) {
  const family = select<WritingFont>("writing-font", [["system", localized("系统字体")], ["serif", localized("衬线")], ["mono", localized("等宽")]], font => change({ font }));
  const editing = element("section", "settings-group");
  editing.append(element("h2", undefined, localized("正文")), field(localized("正文字体"), family, family.id));
  const behavior = element("section", "settings-group");
  behavior.append(element("h2", undefined, localized("编辑控件")));
  const toggles = [
    ["ruledPaper", localized("横线稿纸")],
    ["wrapCode", localized("代码块内自动换行")],
    ["foldHeadings", localized("显示标题折叠控件")],
    ["toolbar", localized("显示主编辑器工具栏")],
  ] as const;
  const checkboxes = new Map<typeof toggles[number][0], HTMLInputElement>();
  for (const [key, label] of toggles) {
    const check = element("input");
    check.type = "checkbox";
    check.id = key;
    check.addEventListener("change", () => { void change({ [key]: check.checked }).catch(report); });
    checkboxes.set(key, check);
    (key === "ruledPaper" ? editing : behavior).append(field(label, check, check.id));
  }
  const headings = element("section", "settings-group");
  const headingBar = element("div", "settings-heading-bar");
  const resetHeadings = button(localized("恢复默认"), () => change({ headingScale: null }));
  resetHeadings.id = "reset-heading-scale";
  headingBar.append(element("h2", undefined, localized("标题字号")), resetHeadings);
  headings.append(headingBar, element("p", "settings-detail muted", localized("以正文为 100%，可设为 80%–240%。")));
  const samples = element("div", "settings-heading-samples");
  const levels = defaultHeadingScale.map((_, index) => {
    const input = element("input");
    input.id = "heading-scale-" + (index + 1);
    input.type = "number";
    input.min = "80";
    input.max = "240";
    input.step = "1";
    input.required = true;
    input.addEventListener("change", () => {
      if (!input.reportValidity()) {
        const saved = currentAppearance().editing.headingScale ?? defaultHeadingScale;
        input.value = String(Math.round(saved[index] * 100));
        return;
      }
      const ratio = input.valueAsNumber / 100;
      void change(current => {
        const headingScale: HeadingScale = [...(current.headingScale ?? defaultHeadingScale)];
        headingScale[index] = ratio;
        return { headingScale };
      }).catch(report);
    });
    const row = field("H" + (index + 1), [input, element("span", "muted", "%")], input.id);
    const label = row.querySelector("label")!;
    bindLabel(input, "aria-label", () => t("{level} 级标题字号", { level: index + 1 }));
    label.style.setProperty("--sample-heading-size", "var(--jot-editor-heading-ratio-" + (index + 1) + ")");
    samples.append(row);
    return input;
  });
  headings.append(samples);
  page.append(editing, headings, behavior);

  return (value: EditingPreferences) => {
    family.value = value.font;
    for (const [key, input] of checkboxes) input.checked = value[key];
    const scale = value.headingScale ?? defaultHeadingScale;
    levels.forEach((input, i) => {
      if (document.activeElement !== input) input.value = String(Math.round(scale[i] * 100));
    });
    resetHeadings.disabled = value.headingScale === null;
  };
}

function mountFilingSettings(page: HTMLElement) {
  type FilingState = { enabled: boolean; endpoint: string; addressKind: "base" | "endpoint"; model: string };
  const filing = element("form", "settings-group");
  const allowFiling = element("input");
  allowFiling.id = "filing-enabled";
  allowFiling.type = "checkbox";
  const endpoint = element("input");
  endpoint.id = "filing-endpoint";
  endpoint.type = "url";
  endpoint.spellcheck = false;
  const kind = select("filing-address-kind", [["base", localized("基础地址")], ["endpoint", localized("完整接口地址")]] as const, addressChanged);
  const model = element("input");
  model.id = "filing-model";
  model.placeholder = "jev-latest";
  model.maxLength = 128;
  model.spellcheck = false;
  const saveStatus = element("p", "settings-detail muted");
  saveStatus.setAttribute("role", "status");
  const save = element("button", "primary", localized("保存设置"));
  save.type = "submit";
  const actions = element("div", "settings-field-controls settings-css-actions");
  actions.append(save);
  filing.append(
    element("h2", undefined, localized("Jev 归档")),
    element("p", "settings-detail", localized("点击「用 Jev 归档」时，Jotiva 会把当前便签的前 6000 个字符和参与分类的目录名称、说明发给下方的服务。确认建议后，文件才会移动。")),
    field(localized("允许 Jev 归档"), allowFiling, allowFiling.id),
    field(localized("地址类型"), kind, kind.id),
    field(localized("服务地址"), endpoint, endpoint.id),
    field(localized("模型名称"), model, model.id),
    actions, saveStatus,
  );

  const keys = element("section", "settings-group");
  const keyStatus = element("p", "settings-detail muted");
  keyStatus.id = "filing-key-status";
  keyStatus.setAttribute("role", "status");
  const keyNotice = element("p", "settings-detail muted");
  keyNotice.setAttribute("role", "status");
  const configure = button(localized("配置密钥…"), () => changeKey("set"));
  const remove = button(localized("移除密钥"), () => changeKey("remove"));
  const keyActions = element("div", "settings-field-controls settings-css-actions");
  keyActions.append(configure, remove);
  keys.append(element("h2", undefined, localized("API 密钥")),
    element("p", "settings-detail", () => t("密钥保存在{store}中，仅用于上方填写的服务地址。", { store: desktopPlatform.keyStore })),
    keyStatus, keyActions, keyNotice);
  page.append(filing, keys);

  let hasKey: boolean | null = null;
  let busy = false;
  let loaded = false;
  let revision = 0;
  let timer: number | undefined;
  function syncControls() {
    for (const control of [allowFiling, kind, endpoint, model, save]) control.disabled = busy || !loaded;
    configure.disabled = busy || !loaded || !endpoint.value.trim() || !endpoint.validity.valid;
    bindLabel(configure, "text", () => hasKey ? t("更换密钥…") : t("配置密钥…"));
    remove.hidden = hasKey !== true;
    remove.disabled = busy;
  }
  function displayKey(present: boolean) {
    hasKey = present;
    bindLabel(keyStatus, "text", () => present ? t("此服务已配置密钥。") : t("此服务尚未配置密钥。"));
    syncControls();
  }
  function keyRequest(action: "check" | "set" | "remove") {
    return invoke<boolean>("filing_key", { action, endpoint: endpoint.value.trim(), addressKind: kind.value });
  }
  function addressChanged() {
    const current = ++revision;
    clearTimeout(timer);
    hasKey = null;
    keyNotice.textContent = "";
    endpoint.placeholder = kind.value === "base" ? "https://example.com/v1" : "https://example.com/v1/systemone";
    syncControls();
    if (!endpoint.value.trim() || !endpoint.validity.valid) {
      bindLabel(keyStatus, "text", () => endpoint.value.trim() ? t("请填写完整的服务地址。") : t("填写服务地址后，即可配置密钥。"));
      return;
    }
    bindLabel(keyStatus, "text", localized("正在检查此服务的密钥…"));
    timer = window.setTimeout(async () => {
      try {
        const present = await keyRequest("check");
        if (current === revision) displayKey(present);
      } catch (error) {
        if (current === revision) bindLabel(keyStatus, "text", () => t("无法确认此服务的密钥状态。") + String(error));
      }
    }, 200);
  }
  endpoint.addEventListener("input", addressChanged);
  filing.addEventListener("input", () => { saveStatus.textContent = ""; });

  async function changeKey(action: "set" | "remove") {
    if (busy) return;
    busy = true;
    ++revision;
    clearTimeout(timer);
    keyNotice.textContent = "";
    syncControls();
    try {
      displayKey(await keyRequest(action));
    } catch (error) {
      if (hasKey === null) addressChanged();
      keyNotice.textContent = String(error);
    } finally {
      busy = false;
      syncControls();
    }
  }
  function displaySettings(state: FilingState) {
    allowFiling.checked = state.enabled;
    endpoint.value = state.endpoint;
    kind.value = state.addressKind;
    model.value = state.model;
    addressChanged();
  }
  filing.addEventListener("submit", async event => {
    event.preventDefault();
    if (busy) return;
    busy = true;
    syncControls();
    try {
      displaySettings(await invoke<FilingState>("save_filing", {
        enabled: allowFiling.checked, endpoint: endpoint.value.trim(), addressKind: kind.value, model: model.value.trim(),
      }));
      bindLabel(saveStatus, "text", localized("设置已保存。"));
    } catch (error) {
      saveStatus.textContent = String(error);
    } finally {
      busy = false;
      syncControls();
    }
  });

  syncControls();
  return async () => {
    if (busy) return;
    if (loaded) { addressChanged(); return; }
    busy = true;
    try {
      displaySettings(await invoke<FilingState>("read_filing"));
      loaded = true;
    } catch (error) {
      saveStatus.textContent = String(error);
    } finally {
      busy = false;
      syncControls();
    }
  };
}

export async function mount() {
  secondaryTitlebar(localized("Jotiva 设置"));
  root.className = "settings-view jot-settings";
  const current = await invoke<SettingsView>("read_settings");
  const content = element("main", "settings-content");
  bindLabel(content, "aria-label", localized("设置内容"));
  const navigation = element("nav", "settings-nav");
  bindLabel(navigation, "aria-label", localized("设置分类"));
  const panels = new Map<Section, HTMLElement>();
  const nav = new Map<Section, HTMLButtonElement>();
  for (const [id, title, glyph] of sections) {
    const tab = button("", () => show(id), "settings-tab");
    tab.append(icon(glyph), element("span", undefined, title));
    navigation.append(tab);
    nav.set(id, tab);
    const page = element("section", "settings-page");
    page.hidden = true;
    page.append(element("h1", undefined, id === "editor" ? localized("编辑器画面") : title));
    content.append(page);
    panels.set(id, page);
  }
  root.append(navigation, content);

  let savingAppearance = Promise.resolve<unknown>(undefined);
  function apply(patch: Partial<AppearancePreferences>, editor?: EditingPatch) {
    const request = savingAppearance.catch(() => {}).then(() => {
      const { baseSize, mode, accent, language, editing } = currentAppearance();
      const changes = typeof editor === "function" ? editor(editing) : editor;
      return saveAppearance({ baseSize, mode, accent, language, editing: { ...editing, ...changes }, ...patch });
    });
    savingAppearance = request;
    return request;
  }

  const appearance = element("form", "appearance-form settings-group");
  const font = element("input");
  font.id = "base-size";
  font.type = "number";
  font.min = "11";
  font.max = "18";
  font.step = "0.5";
  font.required = true;
  const applySize = element("button", "primary", localized("应用"));
  applySize.type = "submit";
  const systemSize = button(localized("系统默认"), () => apply({ baseSize: null }));
  const language = select<AppearancePreferences["language"]>("interface-language", [["zh-CN", "简体中文"], ["en", "English"]],
    language => apply({ language }));
  const scheme = select<ThemeMode>("theme-mode", [["system", localized("跟随系统")], ["light", localized("浅色")], ["dark", localized("深色")]], mode => apply({ mode }));
  const accent = element("input");
  accent.id = "accent";
  accent.type = "color";
  bindLabel(accent, "aria-label", localized("自定义强调色"));
  accent.addEventListener("change", () => { void apply({ accent: accent.value }).catch(report); });
  const palette = element("div", "settings-palette");
  palette.setAttribute("role", "group");
  bindLabel(palette, "aria-label", localized("强调色"));
  const swatches = accentPresets.map((preset, index) => {
    const swatch = button("", () => apply({ accent: index ? preset.light : null }), "settings-swatch");
    bindLabel(swatch, "aria-label", () => message(preset.name));
    bindLabel(swatch, "title", () => message(preset.name) + (index ? "" : t("（默认）")));
    swatch.style.setProperty("--swatch-light", preset.light);
    swatch.style.setProperty("--swatch-dark", preset.dark);
    palette.append(swatch);
    return swatch;
  });
  const custom = element("label", "settings-custom-accent");
  custom.append(accent, element("span", undefined, localized("自定…")));
  palette.append(custom);
  appearance.append(
    field(localized("界面语言"), language, language.id),
    field(localized("界面模式"), scheme, scheme.id),
    field(localized("基础字号"), [font, element("span", "muted", "pt"), applySize, systemSize], font.id),
    field(localized("强调色"), palette),
  );
  appearance.addEventListener("submit", event => {
    event.preventDefault();
    void apply({ baseSize: font.valueAsNumber }).catch(report);
  });

  const cssName = element("p", "settings-detail muted");
  const file = element("input");
  file.type = "file";
  file.accept = ".css,text/css";
  file.hidden = true;
  const importCss = button(localized("导入 CSS…"), () => file.click());
  const removeCss = button(localized("移除"), () => setStylesheet(null));
  const cssActions = element("div", "settings-field-controls settings-css-actions");
  cssActions.append(file, importCss, removeCss);
  file.addEventListener("change", async () => {
    const picked = file.files?.[0];
    if (!picked) return;
    file.value = "";
    try {
      if (picked.size > 128 * 1024 || !picked.name.toLowerCase().endsWith(".css")) throw new Error(t("请选择不超过 128 KB 的 CSS 文件"));
      await setStylesheet({ name: picked.name, css: await picked.text() });
    } catch (error) {
      report(error);
    }
  });
  const css = element("section", "settings-group");
  css.append(element("h2", undefined, localized("自定义 CSS")), cssName, cssActions);
  panels.get("appearance")!.append(appearance, css);

  const refreshEditor = mountEditorSettings(panels.get("editor")!, change => apply({}, change));

  function refreshAppearance(value = currentAppearance()) {
    font.value = String(value.baseSize ?? value.systemSize);
    systemSize.disabled = value.baseSize === null;
    scheme.value = value.mode;
    language.value = value.language ?? "zh-CN";
    const selected = accentPreset(value.accent);
    swatches.forEach((swatch, index) => swatch.setAttribute("aria-pressed", String(accentPresets[index] === selected)));
    const dark = document.documentElement.dataset.jotScheme === "dark";
    accent.value = selected ? selected[dark ? "dark" : "light"] : value.accent!;
    custom.classList.toggle("is-selected", !selected);
    bindLabel(cssName, "text", () => value.stylesheet?.name ?? t("导入 CSS 可调整界面和正文样式。"));
    removeCss.hidden = value.stylesheet === null;
    refreshEditor(value.editing);
  }
  onAppearanceChange(refreshAppearance);
  refreshAppearance();

  let preferences: GlobalBindings = { ...current.preferences, local: { ...current.preferences.local } };
  let recording: string | undefined;
  const shortcuts = element("form");
  const controls = new Map<string, {label: string; button: HTMLButtonElement; clear?: HTMLButtonElement}>();
  const status = element("p", "settings-detail muted", () => current.shortcutError ?? t("点一下要更改的键位，按下新的组合键，再保存。按 Esc 取消录入。"));
  status.setAttribute("role", "status");

  function valueOf(id: string) {
    return id.startsWith("global:") ? preferences[id.slice(7) as GlobalAction]
      : effectiveShortcut(id, preferences.local ?? {});
  }
  function renderBindings() {
    for (const [id, {button, label, clear}] of controls) {
      const value = valueOf(id);
      bindLabel(button, "text", () => recording === id ? t("正在录入…") : shortcutLabel(value));
      bindLabel(button, "aria-label", () => message(label) + " " + button.textContent);
      button.setAttribute("aria-pressed", String(recording === id));
      bindLabel(button, "data-tip", () => recording === id ? t("取消") : t("录入"));
      if (clear) clear.hidden = value == null;
    }
  }

  async function record(id?: string) {
    recording = id;
    renderBindings();
    try {
      await invoke("record_shortcut", { active: id !== undefined });
    } catch (error) {
      recording = undefined;
      renderBindings();
      throw error;
    }
  }
  async function setKey(id: string, value: string | null) {
    if (id.startsWith("global:")) preferences = { ...preferences, [id.slice(7)]: value };
    else {
      const local = { ...preferences.local };
      if (value === effectiveShortcut(id, {})) delete local[id]; else local[id] = value;
      preferences = { ...preferences, local };
    }
    await record();
    bindLabel(status, "text", localized("待保存"));
  }
  function keyField(id: string, label: string) {
    const control = button("", () => record(recording === id ? undefined : id), "recording-key");
    control.id = id.startsWith("global:") ? "key-" + id.slice(7) : "key-local-" + id;
    const clear = id === "global:quick" ? undefined : button(localized("清除"), () => setKey(id, null));
    if (clear) bindLabel(clear, "aria-label", () => t("清除“{action}”的快捷键", { action: message(label) }));
    controls.set(id, {label, button:control, clear});
    return field(() => message(label), clear ? [control, clear] : control, control.id);
  }
  const save = element("button", "primary", localized("保存快捷键"));
  save.type = "submit";
  const reset = button(localized("恢复默认"), async () => {
    preferences = { ...current.defaults, local: {} };
    await record();
    bindLabel(status, "text", localized("待保存"));
  });
  const shortcutActions = element("div", "settings-group shortcut-actions");
  const buttons = element("div", "settings-field-controls settings-css-actions");
  buttons.append(save, reset);
  const searchKeys = element("input");
  searchKeys.type = "search";
  bindLabel(searchKeys, "placeholder", localized("搜索操作或快捷键"));
  bindLabel(searchKeys, "aria-label", () => searchKeys.placeholder);
  shortcutActions.append(searchKeys, buttons, status);
  shortcuts.append(shortcutActions);
  const globalGroup = element("section", "settings-group");
  globalGroup.append(element("h2", undefined, localized("全局快捷键")));
  for (const [id, label] of globalActions) globalGroup.append(keyField("global:" + id, label));
  shortcuts.append(globalGroup);
  const groups = new Map<string, HTMLElement>();
  function group(title: string) {
    let section = groups.get(title);
    if (!section) {
      section = element("section", "settings-group");
      section.append(element("h2", undefined, () => message(title)));
      groups.set(title, section);
      shortcuts.append(section);
    }
    return section;
  }
  for (const row of commands) group(row.group).append(keyField(row.id, row.label));
  for (const row of fixedShortcuts) group(row.group).append(field(() => message(row.label), element("kbd", "settings-key", row.key)));
  function filterShortcuts() {
    const query = searchKeys.value.trim().toLocaleLowerCase();
    for (const section of [globalGroup, ...groups.values()]) {
      const heading = section.querySelector("h2")?.textContent ?? "";
      const rows = Array.from(section.querySelectorAll<HTMLElement>(".settings-field"));
      for (const row of rows) {
        const label = row.querySelector("label,span")?.textContent ?? "";
        const key = row.querySelector(".recording-key,kbd")?.textContent ?? "";
        row.hidden = !!query && !(heading + " " + label + " " + key).toLocaleLowerCase().includes(query);
      }
      section.hidden = rows.every(row => row.hidden);
    }
  }
  searchKeys.addEventListener("input", filterShortcuts);
  window.addEventListener("jot:language", () => { renderBindings(); filterShortcuts(); });
  renderBindings();
  shortcuts.addEventListener("submit", async event => {
    event.preventDefault();
    if (save.disabled) return;
    save.disabled = true;
    try {
      await record();
      await invoke("save_settings", { preferences });
      bindLabel(status, "text", localized("快捷键已保存"));
    } catch (error) {
      status.textContent = String(error);
    } finally {
      save.disabled = false;
    }
  });
  function acceptNative(chord: string) {
    const id = recording;
    if (!id) return;
    void setKey(id, chord).catch(report);
  }
  window.addEventListener("keydown", event => {
    if (!recording || event.isComposing || event.repeat) return;
    if (event.key === "Escape" || event.key === "Tab" && !event.ctrlKey && !event.metaKey && !event.altKey) {
      if (event.key === "Escape") event.preventDefault();
      void record().catch(report);
      return;
    }
    if (["Shift", "Alt", "Control", "Meta"].includes(event.key)) return;
    event.preventDefault();
    event.stopPropagation();
    const value = recordedShortcut(event, recording.startsWith("global:"));
    if (value) acceptNative(value);
    else status.textContent = desktopPlatform.recordingHint;
  }, true);
  window.addEventListener("blur", () => { if (recording) void record().catch(report); });
  await getCurrentWindow().listen<string>(events.shortcutRecorded, event => acceptNative(event.payload));
  panels.get("shortcuts")!.append(shortcuts);

  const loadFiling = mountFilingSettings(panels.get("filing")!);
  const permission = element("p", "settings-detail muted");
  permission.setAttribute("role", "status");
  const authorization = element("section", "settings-group");
  const help = accessibilityHelp();
  const requestAccess = button(localized("授权辅助功能…"), () => invoke("request_accessibility"));
  authorization.append(
    element("h2", undefined, localized("macOS 辅助功能")),
    element("p", "settings-detail", localized("授权后，Jotiva 可以关联指定窗口，在切回该窗口时显示对应便签。")),
    permission, requestAccess, help,
  );
  if (desktopPlatform.usesAccessibilityPermission) panels.get("windows")!.append(authorization);
  else panels.get("windows")!.append(element("p","settings-detail",() => desktopPlatform.hostWindows ? t("可以在窗口列表中选择要关联的应用窗口。") : t("当前桌面支持独立速记。")));

  let stopCheckingPermission: (() => void) | undefined;
  function showPermission(trusted: boolean) {
    bindLabel(permission, "text", () => trusted ? t("已授权") : t("当前运行的 Jotiva 尚未获得授权"));
    requestAccess.hidden = help.hidden = trusted;
  }
  showPermission(current.trusted);

  function show(id: Section) {
    if (!panels.has(id)) return;
    if (recording) void record().catch(report);
    for (const [key, pane] of panels) {
      const tab = nav.get(key)!;
      pane.hidden = key !== id;
      if (key === id) tab.setAttribute("aria-current", "page");
      else tab.removeAttribute("aria-current");
    }
    content.scrollTop = 0;
    if (id === "filing") void loadFiling();
    stopCheckingPermission?.();
    stopCheckingPermission = undefined;
    if (id === "windows" && desktopPlatform.usesAccessibilityPermission) {
      stopCheckingPermission = watchAccessibility(showPermission, error => { permission.textContent = String(error); });
    }
  }
  await getCurrentWindow().listen<Section>(events.settingsSection, ({ payload }) => show(payload));
  const initial = (window as Window & { __jotSettingsSection?: Section }).__jotSettingsSection;
  show(initial && panels.has(initial) ? initial : "appearance");
}
