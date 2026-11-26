import { invoke } from "@tauri-apps/api/core";
import { currentAppearance, setStylesheet, type Appearance, type AppearancePreferences, type Stylesheet, type ThemePreset } from "../../appearance/preferences";
import { bindLabel, localized, t, type Label } from "../../i18n";
import { button, element } from "../../ui/dom";
import { fileDialog } from "../../ui/file-dialog";
import { select } from "./controls";

const builtIns = [
  ["default", localized("默认")], ["ink", localized("墨与钢")], ["material", "Material"],
  ["blueprint", localized("蓝晒")],
] as const satisfies readonly (readonly [ThemePreset, Label])[];

function themeName(file: string) {
  return file.replace(/\.css$/i, "");
}

function resolveConflict(file: string): Promise<string> {
  return new Promise(resolve => {
    const dialog = element("dialog", "sheet jot-dialog");
    const title = t("已安装同名主题「{name}」", { name: themeName(file) });
    const cancel = button(localized("取消"), () => dialog.close());
    const actions = element("div", "sheet-actions");
    actions.append(cancel,
      button(localized("换个名字导入…"), () => dialog.close("rename")),
      button(localized("更新现有主题"), () => dialog.close("replace")));
    dialog.append(element("h2", undefined, title),
      element("p", "folder-location", localized("更新会替换已安装的 CSS 文件。")), actions);
    dialog.setAttribute("aria-label", title);
    dialog.addEventListener("close", () => {
      dialog.remove();
      resolve(dialog.returnValue);
    }, { once: true });
    document.body.append(dialog);
    dialog.showModal();
    cancel.focus();
  });
}

export function mountThemeSettings(change: (patch: Partial<AppearancePreferences>) => Promise<unknown>) {
  const preset = select<string>("theme-preset", builtIns, async name => {
    try {
      const builtIn = builtIns.find(([id]) => id === name)?.[0];
      await change(builtIn
        ? { themePreset: builtIn, themeFile: null }
        : { themeFile: name });
    } catch (error) {
      refresh(currentAppearance());
      throw error;
    }
  });
  const bundled = element("optgroup");
  bindLabel(bundled, "label", localized("内置主题"));
  bundled.append(...preset.children);
  preset.append(bundled);
  const installed = element("optgroup");
  bindLabel(installed, "label", localized("我的主题"));

  const panel = element("section", "settings-group");
  const header = element("div", "settings-heading-bar");
  const list = element("div");
  const status = element("p", "settings-detail muted");
  status.setAttribute("role", "status");
  status.hidden = true;
  const importButton = button(localized("导入主题…"), async () => {
    importButton.disabled = true;
    status.hidden = true;
    try {
      const stylesheet = await invoke<Stylesheet | null>("choose_theme_css");
      if (!stylesheet) return;
      let title: string | null = null;
      let replace: string | null = null;
      while (true) {
        const conflict: string | null = await invoke("install_theme", { stylesheet, title, replace });
        if (!conflict) {
          await load();
          const name = themeName(replace ?? title ?? stylesheet.name);
          bindLabel(status, "text", () => t(replace ? "已更新主题「{name}」" : "已导入主题「{name}」", { name }));
          status.hidden = false;
          return;
        }
        const action = await resolveConflict(conflict);
        if (action === "replace") {
          replace = conflict;
        } else if (action === "rename") {
          const name = await fileDialog(t("换个名字导入…"), t("导入"), { initial: title ?? stylesheet.name });
          if (name === undefined) return;
          title = name;
          replace = null;
        } else {
          return;
        }
      }
    } finally {
      importButton.disabled = false;
      importButton.focus({ preventScroll: true });
    }
  });
  header.append(element("h2", undefined, localized("我的主题")), importButton);
  panel.append(header, status, list);

  let files: string[] = [];
  let requested = 0;
  let rendered = "";
  async function load() {
    const request = ++requested;
    const incoming = await invoke<string[]>("list_theme_files");
    if (request !== requested) return;
    files = incoming;
    refresh(currentAppearance());
  }

  function refresh(value: Appearance) {
    const options = value.themeFile && !files.includes(value.themeFile) ? [...files, value.themeFile] : files;
    const key = options.join("\0");
    if (key !== rendered) {
      rendered = key;
      installed.replaceChildren(...options.map(file => {
        const option = element("option", undefined, themeName(file));
        option.value = file;
        return option;
      }));
      list.replaceChildren(...files.map(file => {
        const name = themeName(file);
        const rename = button(localized("重命名…"), async () => {
          const title = await fileDialog(t("重命名主题"), t("保存"), { initial: file });
          if (title !== undefined && title !== file) {
            status.hidden = true;
            await invoke("rename_theme", { file, title });
            await load();
          }
        });
        bindLabel(rename, "aria-label", () => t("重命名主题「{name}」", { name }));
        const row = element("div", "settings-field settings-theme-file");
        row.append(element("span", undefined, name), rename);
        return row;
      }));
      if (options.length) preset.append(installed);
      else installed.remove();
    }
    preset.value = value.themeFile ?? value.themePreset ?? "default";
  }

  return { preset, panel, refresh, load };
}

export function mountGlobalCss() {
  const panel = element("section", "settings-group settings-global-css");
  const heading = element("h2", undefined, localized("全局样式微调"));
  heading.id = "global-css-label";
  const input = element("textarea");
  input.rows = 7;
  input.spellcheck = false;
  input.autocapitalize = "off";
  input.setAttribute("aria-labelledby", heading.id);
  const detail = element("p", "settings-detail muted", localized("应用于所有主题。清空并保存可移除微调。"));
  detail.id = "global-css-detail";
  input.setAttribute("aria-describedby", detail.id);
  let saved = "";
  let busy = false;
  function checkSize(css: string) {
    if (new TextEncoder().encode(css).length > 128 * 1024) throw new Error(t("全局 CSS 不能超过 128 KB"));
  }

  const format = button(localized("格式化"), async () => {
    const css = input.value;
    checkSize(css);
    busy = true;
    sync();
    try {
      const { formatCss } = await import("./format-css");
      let formatted: string;
      try {
        formatted = formatCss(css);
      } catch {
        throw new Error(t("无法格式化 CSS，请检查语法。"));
      }
      checkSize(formatted);
      if (input.value === css) input.value = formatted;
    } finally {
      busy = false;
      sync();
    }
  });
  const save = button(localized("保存"), async () => {
    const css = input.value;
    checkSize(css);
    busy = true;
    sync();
    try {
      const stylesheet = css.trim() ? { name: "global.css", css } : null;
      await setStylesheet(stylesheet);
      saved = stylesheet?.css ?? "";
      if (input.value === css && css !== saved) input.value = saved;
    } finally {
      busy = false;
      sync();
    }
  }, "primary");
  const actions = element("div", "settings-css-actions");
  actions.append(format, save);
  panel.append(heading, detail, input, actions);
  input.addEventListener("input", sync);

  function sync() {
    format.disabled = busy || !input.value.trim();
    save.disabled = busy || input.value === saved;
  }
  function refresh(stylesheet: Stylesheet | null) {
    const css = stylesheet?.css.replace(/\r\n?/g, "\n") ?? "";
    if (input.value === saved && input.value !== css) input.value = css;
    saved = css;
    sync();
  }
  return { panel, refresh };
}
