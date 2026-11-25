import english from "../i18n/en.json";
import { t, message, language } from "../i18n";
// Long action names stay on the control for VoiceOver. These are the compact,
// visible names for Jotiva and the buttons supplied by the released editor.
const names: Readonly<Record<string, string>> = {
  "显示或隐藏侧栏": "侧栏", "文档大纲": "大纲", "新建便签": "新建",
  "打开文档": "打开", "移动便签": "移动", "移到废纸篓": "丢弃",
  "折叠便签": "折叠", "编辑工具栏": "格式", "作为独立便签打开": "独立便签",
  "展开便签": "展开", "查找便签": "查找", "在主窗口编辑": "主窗编辑", "解除挂载并收回": "收回",
  "挂载到窗口": "关联", "更换宿主窗口": "换窗口", "平铺便签": "平铺", "从列表选择": "选窗口",
  "平铺显示": "平铺", "列表显示": "列表", "窗口关联设置": "设置",
  "添加来源窗口": "添加来源", "添加工作区文件夹": "添加目录",
  "添加文件夹": "添加目录", "文件夹设置": "设置",
  "保存文件夹": "保存", "刷新窗口": "刷新", "显示更多": "更多",
  "另存编辑并重载": "保留副本", "导入 CSS": "导入", "保存快捷键": "保存",
  "授权辅助功能": "授权", "移除密钥": "移除", "系统默认": "默认", "恢复默认": "重置",
  "选择标题级别": "标题", "选择表格尺寸": "表格", "更多格式和插入": "更多",
  "切换有序列表": "有序列表", "切换无序列表": "无序列表", "切换任务列表": "待办",
  "切换引用": "引用", "插入链接": "链接", "命令面板": "命令",
  "插入代码块": "代码块", "插入公式块": "公式块", "插入分隔线": "分隔线",
  "插入图片": "图片", "插入表格": "插表", "关闭 · Esc": "关闭",
  "切换到源码模式": "源码", "切换到即时预览": "即时预览", "切换到预览": "预览",
  "折叠或展开当前标题": "切换折叠",
  "在当前行下方插入表格行": "增行", "删除当前表格行": "删行",
  "在当前列右侧插入表格列": "增列", "删除当前表格列": "删列", "清空当前表格单元格": "清空",
  "用 Jev 归档": "归档", "打开 Jev 归档设置": "归档设置", "安装更新并重启": "安装更新",
  "在 Finder 中显示这份 Jotiva": "查看位置", "打开辅助功能设置": "打开设置",
  "返回编辑器": "返回正文", "聚焦文档大纲": "大纲", "聚焦编辑工具栏": "工具栏",
  "搜索和替换": "替换", "下一个匹配": "下一处", "上一个匹配": "上一处",
  "替换当前匹配": "替换当前", "选中下一个相同文本": "追加选中",
  "在当前块后继续编辑": "移到块后", "在当前块前继续编辑": "移到块前",
  "移到上方单元格": "上一格", "移到下方单元格": "下一格", "展开全部章节": "全部展开",
  "下一个表格单元格": "下一格", "上一个表格单元格": "上一格",
  "上方表格单元格": "向上移格", "下方表格单元格": "向下移格",
  "在下方插入表格行": "增行", "在右侧插入表格列": "增列",
  "删除表格行": "删行", "删除表格列": "删列", "清空表格单元格": "清空",
  "快捷键设置": "快捷键", "插入或编辑链接": "链接",
};

// Resolve the same semantic hint from either localized label. Dynamic controls
// (filenames, shortcuts, session names) declare their hint with data-tip.
const hints = new Map<string, string>();
for (const [label, hint] of Object.entries(names)) {
  hints.set(label, hint);
  const translation = english[label as keyof typeof english];
  if (translation) hints.set(translation.replace(/…$/u, ""), hint);
}

function shortName(button: HTMLElement, origin: Element) {
  const expanded = button.getAttribute("aria-expanded");
  const requested = button.dataset.tip;
  if (requested) return requested;
  if (button.matches(".jot-outline__row")) {
    return expanded !== null && origin.closest(".jot-outline__toggle") ? expanded === "true" ? t("折叠") : t("展开") : t("定位");
  }
  if (button.matches(".jot-heading-fold-button, .library-node[aria-expanded]")) {
    if (button.getAttribute("aria-busy") === "true") return t("取消");
    return expanded === "true" ? t("折叠") : t("展开");
  }
  if (button.matches(".note-row, .workspace-file")) return t("打开");
  if (button.matches(".note-rail-row")) return t("定位");
  if (button.matches(".recording-key")) return button.getAttribute("aria-pressed") === "true" ? t("取消") : t("录入");
  if (button.matches("[data-rows][data-columns]")) return t("插表");
  const level = button.dataset.jotHeadingLevel ?? button.dataset.jotCommand?.match(/^format\.heading-([1-6])$/u)?.[1];
  if (level) return language === "en" ? "Heading " + level : "一二三四五六"[Number(level) - 1] + "级标题";
  const command = button.hasAttribute("data-jot-command");
  const label = (button.getAttribute("aria-label") || (command && button.firstElementChild?.firstChild?.textContent)
    || button.textContent || button.title).trim().replace(/…$/u, "");
  const hint = hints.get(label);
  if (hint) return message(hint);
  // A concise button caption can be its own hint; never guess an action from a long description.
  const caption = (button.textContent || "").trim().replace(/…$/u, "");
  for (const text of [label, caption]) {
    if (/^[\p{Script=Han}]{1,4}$/u.test(text) || /^[\w+-]+(?: [\w+-]+){0,2}$/u.test(text) && text.length <= 22) return message(text);
  }
  return "";
}

const controls = 'button, [role="button"], input[type="button"], input[type="submit"], [data-tip]';

export function bindTooltips() {
  const tip = document.createElement("div");
  tip.className = "jot-tooltip";
  tip.id = "jot-button-tip";
  tip.popover = "manual";
  tip.setAttribute("role", "tooltip");
  let owner: HTMLElement | undefined;
  let origin: Element | undefined;
  let timer: number | undefined;
  let keyboard = false;
  const titleDescriptions = new WeakMap<HTMLElement, string>();

  function hide() {
    window.clearTimeout(timer);
    timer = undefined;
    changes.disconnect();
    visibility.disconnect();
    if (tip.matches(":popover-open")) tip.hidePopover();
    if (owner?.getAttribute("aria-describedby")?.split(/\s+/).includes(tip.id)) {
      const ids = owner.getAttribute("aria-describedby")!.split(/\s+/).filter(id => id !== tip.id);
      if (ids.length) owner.setAttribute("aria-describedby", ids.join(" "));
      else owner.removeAttribute("aria-describedby");
    }
    owner = origin = undefined;
  }

  function preserveTitle(button: HTMLElement) {
    const title = button.title;
    if (title) {
      const label = button.getAttribute("aria-label") || button.textContent?.trim();
      const description = button.getAttribute("aria-description");
      if (!label) button.setAttribute("aria-label", title);
      else if (description === null || description === titleDescriptions.get(button)) {
        if (title !== label) {
          button.setAttribute("aria-description", title);
          titleDescriptions.set(button, title);
        } else if (description !== null) {
          button.removeAttribute("aria-description");
          titleDescriptions.delete(button);
        }
      }
    }
    // An empty title also prevents WebKit inheriting a containing source window's title.
    if (title || (!button.hasAttribute("title") && button.parentElement?.closest("[title]"))) button.title = "";
  }

  function show() {
    if (!owner || !origin || !owner.isConnected || !owner.getClientRects().length || owner.closest("[inert]")) { hide(); return; }
    const host = owner.closest("dialog[open], :popover-open") ?? document.body;
    if (tip.parentElement !== host) host.append(tip);
    const text = shortName(owner, origin);
    if (!text) { hide(); return; }
    tip.textContent = text;
    if (!tip.matches(":popover-open")) tip.showPopover();
    // Keep existing descriptions (including a file path or shortcut) ahead of
    // the short visible hint in the accessible description computation.
    if (!owner.hasAttribute("aria-description") && text !== (owner.getAttribute("aria-label") ?? owner.textContent?.trim())) {
      const ids = owner.getAttribute("aria-describedby")?.split(/\s+/).filter(Boolean) ?? [];
      if (!ids.includes(tip.id)) owner.setAttribute("aria-describedby", [...ids, tip.id].join(" "));
    }
    const anchor = owner.getBoundingClientRect();
    const box = tip.getBoundingClientRect();
    const gap = Number.parseFloat(getComputedStyle(tip).rowGap);
    tip.style.left = Math.max(gap, Math.min(innerWidth - box.width - gap, anchor.left + (anchor.width - box.width) / 2)) + "px";
    const below = anchor.bottom + gap;
    const y = below + box.height + gap > innerHeight ? anchor.top - box.height - gap : below;
    tip.style.top = Math.max(gap, Math.min(innerHeight - box.height - gap, y)) + "px";
  }

  const changes = new MutationObserver(() => {
    if (!owner) return;
    preserveTitle(owner);
    if (origin && tip.matches(":popover-open") && tip.textContent !== shortName(owner, origin)) show();
  });
  const visibility = new IntersectionObserver(entries => {
    if (entries.some(entry => entry.target === owner && !entry.isIntersecting)) hide();
  });

  function schedule(immediate: boolean) {
    window.clearTimeout(timer);
    // Restoring focus can run inside WebKit's closePopover algorithm. Show the
    // keyboard hint on the next task after the enclosing top-layer update.
    timer = window.setTimeout(() => { timer = undefined; show(); }, immediate ? 0 : 440);
  }

  function arm(target: EventTarget | null, immediate: boolean) {
    if (!(target instanceof Element)) return;
    const button = target.closest<HTMLElement>(controls);
    if (!button || button.matches(".jot-window-menu-item, [data-file-menu-path]")) return;
    if (button === owner && (target === origin || shortName(button, target) === shortName(button, origin!))) {
      if (immediate && timer !== undefined) schedule(true);
      return;
    }
    hide();
    owner = button;
    origin = target;
    preserveTitle(button);
    changes.observe(button, { attributes: true, attributeFilter: ["title", "aria-expanded", "aria-busy", "aria-label", "aria-pressed", "data-tip"] });
    visibility.observe(button);
    schedule(immediate);
  }

  document.addEventListener("pointerover", event => {
    if (event.pointerType !== "touch" && !event.buttons) arm(event.target, false);
  }, true);
  document.addEventListener("pointerout", event => {
    if (event.target instanceof Node && owner?.contains(event.target)
      && !(event.relatedTarget instanceof Node && owner.contains(event.relatedTarget))) hide();
  }, true);
  document.addEventListener("pointerdown", () => { keyboard = false; hide(); }, true);
  document.addEventListener("keydown", event => {
    keyboard = true;
    if (["Escape", "Enter", " "].includes(event.key)) hide();
  }, true);
  document.addEventListener("focusin", event => {
    if (keyboard && !(event.target instanceof Element && event.target.closest(".library-tree"))) arm(event.target, true);
  }, true);
  document.addEventListener("focusout", hide, true);
  for (const type of ["dragstart", "pointercancel", "scroll", "close", "visibilitychange"]) document.addEventListener(type, hide, true);
  document.addEventListener("beforetoggle", event => { if (event.target !== tip) hide(); }, true);
  for (const type of ["blur", "resize", "pagehide"]) window.addEventListener(type, hide);
}
