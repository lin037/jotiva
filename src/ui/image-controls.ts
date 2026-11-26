import { folders, imageSettings, type ImageConfig, type ImageMode, type ImageRule, type ImageUpload } from "../desktop/ipc";
import { desktopPlatform, displayPath, fileName } from "../desktop/platform";
import { bindLabel, localized, message, t, type Label } from "../i18n";
import { button, element } from "./dom";
import "../styles/images.css";

export const imageModes = ["relative", "fixed", "http"] as const;
export const imageModeName = {
  relative: localized("跟随文档"),
  fixed: localized("固定目录"),
  http: localized("HTTP 上传"),
};
export const imageModeDetail = {
  relative: localized("存到文档旁的图片子文件夹。"),
  fixed: localized("把图片集中存到同一个文件夹。"),
  http: localized("上传图片，并在文档中插入网络链接。"),
};

export function imageFolderName(path: string) {
  return fileName(displayPath(path)) || t("根目录");
}

export function imageFolderLabel(path: string, name?: Label) {
  const label = element("span", "image-folder-label");
  label.append(element("strong", undefined, name ?? (() => imageFolderName(path))));
  return label;
}

export function imageInput(id: string, type = "text") {
  const input = element("input");
  input.id = id;
  input.type = type;
  return input;
}

export function imageField(label: Label, input: HTMLElement, help?: Label, action?: HTMLElement) {
  const row = element("div", "image-field");
  const title = element("label", undefined, label);
  title.htmlFor = input.id;
  row.append(title);
  if (help) {
    const note = element("small", "image-help muted", help);
    note.id = input.id + "-help";
    input.setAttribute("aria-describedby", note.id);
    row.append(note);
  }
  if (action) {
    const controls = element("div", "image-path-input");
    controls.append(input, action);
    row.append(controls);
  } else {
    row.append(input);
  }
  return row;
}

export function imageCheck(label: Label, input: HTMLInputElement, help?: Label) {
  const row = element("label", "image-check");
  const text = element("span");
  text.append(element("span", undefined, label));
  if (help) text.append(element("small", "image-help muted", help));
  row.append(input, text);
  return row;
}

const ruleToken = "${ruleDir}";
function ruleRelativePath(directory: string) {
  if (directory === ruleToken) return "";
  const separators = desktopPlatform.pathSeparator === "\\" ? /[/\\]/ : /\//;
  return directory.startsWith(ruleToken) && separators.test(directory.charAt(ruleToken.length))
    ? directory.slice(ruleToken.length + 1) : null;
}

export function imageRuleFields(id: string, rule: ImageRule, onChange: () => void) {
  const mode = element("select");
  mode.id = id + "-mode";
  for (const value of imageModes) {
    const option = element("option", undefined, imageModeName[value]);
    option.value = value;
    mode.append(option);
  }
  mode.value = rule.mode;

  const subfolder = imageInput(id + "-subfolder");
  subfolder.value = rule.subfolder;
  const relative = imageField(localized("图片子文件夹名称"), subfolder, localized("留空使用全局的子文件夹名称。"));
  subfolder.addEventListener("input", () => { rule.subfolder = subfolder.value.trim(); onChange(); });

  const location = element("select");
  location.id = id + "-location";
  const options = [
    ["default", localized("沿用全局固定目录")],
    ["relative", localized("相对于此文件夹")],
    ["absolute", localized("指定本机文件夹")],
  ] as const;
  for (const [value, title] of options) {
    const option = element("option", undefined, title);
    option.value = value;
    location.append(option);
  }
  const customPath = ruleRelativePath(rule.directory);
  location.value = customPath !== null ? "relative" : rule.directory ? "absolute" : "default";
  const adjacent = imageInput(id + "-relative");
  adjacent.value = customPath ?? "";
  adjacent.placeholder = "../assets";
  const adjacentRow = imageField(localized("相对位置"), adjacent,
    localized("例如 assets 表示此文件夹内，../assets 表示同级文件夹。留空使用此文件夹。"));
  const directory = imageInput(id + "-directory");
  let absolutePath = customPath === null ? rule.directory : "";
  directory.value = displayPath(absolutePath);
  const choose = button(localized("选择…"), async () => {
    const path = await folders.choose();
    if (path) {
      absolutePath = path;
      directory.value = displayPath(path);
      updateDirectory();
    }
  });
  const directoryRow = imageField(localized("图片目录"), directory, undefined, choose);
  const fixed = element("div", "image-fields");
  fixed.append(imageField(localized("图片目录来源"), location), adjacentRow, directoryRow);

  function updateDirectory() {
    const path = adjacent.value.trim();
    rule.directory = location.value === "relative" ? (path ? ruleToken + "/" + path : ruleToken)
      : location.value === "absolute" ? absolutePath : "";
    render();
    onChange();
  }
  function render() {
    relative.hidden = rule.mode !== "relative";
    fixed.hidden = rule.mode !== "fixed";
    adjacentRow.hidden = location.value !== "relative";
    directoryRow.hidden = location.value !== "absolute";
    directory.required = rule.mode === "fixed" && location.value === "absolute";
  }
  mode.addEventListener("change", () => { rule.mode = mode.value as ImageMode; render(); onChange(); });
  location.addEventListener("change", updateDirectory);
  adjacent.addEventListener("input", updateDirectory);
  directory.addEventListener("input", () => { absolutePath = directory.value.trim(); updateDirectory(); });
  const fields = element("div", "image-fields");
  fields.append(imageField(localized("保存方式"), mode), relative, fixed);
  render();
  return fields;
}

export function imagePreview(node: HTMLElement, config: ImageConfig, mode: ImageMode, rule?: ImageRule | null) {
  bindLabel(node, "text", () => {
    if (mode === "http") {
      return config.endpoint
        ? t("文档会引用上传服务返回的图片链接。")
        : t("填写上传接口地址后，文档会引用服务返回的图片链接。");
    }
    const demo = t("周报");
    let base = "notes/_assets";
    let link = t("文档写入相对链接。");
    if (mode === "fixed") {
      const directory = rule?.directory || config.directory;
      if (!directory) return t("请选择图片存放目录。");
      const relative = ruleRelativePath(directory);
      if (relative === null) {
        base = t("图片目录");
        link = t("文档写入本机文件链接。");
      } else {
        const location = relative.split(desktopPlatform.pathSeparator === "\\" ? /[/\\]/ : /\//).filter(part => part && part !== ".");
        base = location.includes("..") ? "assets" : location.length ? "notes/assets" : "notes";
      }
    }
    const path = [base, ...(config.groupByDocument ? [demo] : []), "image.png"].join("/")
      .split("/").join(desktopPlatform.pathSeparator);
    return t("例如「{name}.md」的图片保存到 {path}。{link}", { name: demo, path, link });
  });
}

export function imageUploadFields(id: string, changed: (value: ImageUpload) => void, folder?: string) {
  const endpoint = imageInput(id + "-endpoint", "url");
  endpoint.placeholder = "https://example.com/upload";
  const field = imageInput(id + "-field");
  field.placeholder = "file";
  const pointer = imageInput(id + "-pointer");
  pointer.placeholder = "/data/url";
  const bearer = imageInput(id + "-bearer", "checkbox");
  const status = element("p", "image-help muted");
  status.setAttribute("role", "status");
  const buttons = element("div", "image-button-row");
  const configure = button(localized("配置密钥…"), () => editKey("set"));
  const remove = button(localized("移除密钥"), () => editKey("remove"));
  buttons.append(configure, remove);
  const keys = element("div", "image-fields");
  keys.append(status, buttons);
  const node = element("div", "image-fields");
  node.append(
    imageField(localized("上传接口地址"), endpoint, localized("服务须接受 multipart/form-data 的 POST 请求。")),
    imageField(localized("表单文件字段名"), field, localized("服务接收图片的字段，通常是 file。")),
    imageField(localized("响应中的图片地址路径"), pointer, localized('例如返回 {"data":{"url":"…"}}，填写 /data/url。')),
    imageCheck(localized("使用 Bearer 密钥"), bearer, localized("密钥保存在系统凭据中，修改后立即生效。")),
    keys,
  );

  let lookup = 0;
  let timer: ReturnType<typeof setTimeout>;
  let editingKey = false;
  let hasKey = false;
  function address() {
    const value = endpoint.value.trim();
    if (!/^https?:\/\//i.test(value) || /[\s\\]/.test(value)) return "";
    try {
      const url = new URL(value);
      return url.hostname && !url.username && !url.password && !/[?#]/.test(value) ? value : "";
    } catch {
      return "";
    }
  }
  function keyStatus(text: Label, available: boolean) {
    bindLabel(status, "text", text);
    configure.disabled = editingKey || !available;
    remove.disabled = configure.disabled || !hasKey;
  }
  function checkKey() {
    clearTimeout(timer);
    const turn = ++lookup;
    keys.hidden = !bearer.checked;
    hasKey = false;
    if (!bearer.checked || editingKey) return;
    const value = address();
    if (!value) {
      keyStatus(endpoint.value.trim() ? localized("请填写有效的上传接口地址。") : localized("请先填写上传接口地址。"), false);
      return;
    }
    keyStatus(localized("正在检查密钥…"), false);
    timer = setTimeout(async () => {
      if (!node.isConnected) return;
      try {
        const present = await imageSettings.key(value, "check", folder);
        if (turn === lookup) {
          hasKey = present;
          keyStatus(present ? localized("此服务已配置密钥。") : localized("此服务尚未配置密钥。"), true);
        }
      } catch (error) {
        if (turn === lookup) keyStatus(() => message(error instanceof Error ? error.message : String(error)), true);
      }
    }, 180);
  }
  async function editKey(action: "set" | "remove") {
    const value = address();
    if (!value || editingKey) return;
    clearTimeout(timer);
    const turn = ++lookup;
    editingKey = true;
    keyStatus(localized("正在更新密钥…"), false);
    try {
      const present = await imageSettings.key(value, action, folder);
      editingKey = false;
      if (turn === lookup) {
        hasKey = present;
        keyStatus(present ? localized("此服务已配置密钥。") : localized("此服务尚未配置密钥。"), true);
      } else checkKey();
    } catch (error) {
      editingKey = false;
      if (turn === lookup) keyStatus(() => message(error instanceof Error ? error.message : String(error)), true);
      else checkKey();
    }
  }

  const read = (): ImageUpload => ({
    endpoint: endpoint.value.trim(), field: field.value.trim(), responsePointer: pointer.value.trim(), bearer: bearer.checked,
  });
  node.addEventListener("input", () => changed(read()));
  endpoint.addEventListener("input", checkKey);
  bearer.addEventListener("input", checkKey);
  return { node, read, set(value: ImageUpload) {
    endpoint.value = value.endpoint;
    field.value = value.field;
    pointer.value = value.responsePointer;
    bearer.checked = value.bearer;
    checkKey();
  } };
}
