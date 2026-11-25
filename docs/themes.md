# 外观和主题

在「设置 → 外观」可以选浅色、深色，或跟随系统。基础字号可设为 11–18 pt，标题栏控件、侧栏、对话框和速记都会跟着调整。显示缩放沿用桌面设置。

雾蓝、松绿、藤紫和陶土各有浅色与深色版本。选「自定…」可指定按钮填色。Jotiva 按填色亮度选用黑字或白字，同时给链接、选中图标和焦点边框生成适合当前背景的同色系颜色。内置界面的正文大小文字以 4.5∶1 为最低对比度。 [W3C 文字对比度说明](https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html)

「设置 → 编辑器」中，H1–H6 的字号分别按正文字号的百分比设置，范围为 80%–240%。每项标签会预览相应字号，点「恢复默认」可以还原。这里也能更换正文字体、开启横线稿纸、切换折叠控件、代码换行及主窗工具栏。主窗口和速记默认使用素纸。

在「设置 → 外观 → 界面主题」可选择默认、墨与钢或 Material。每套主题都有浅色和深色版本。切换主题时会保留文档、光标和终端会话。导入的 CSS 最后应用，可继续覆盖主题。

## 导入 CSS

想试试自定义主题，可以下载墨钢主题。它用了直角边框、暖黄色和衬线字体，有浅色和深色两套配色。 [墨钢主题](assets/ink-and-steel.css)

在「设置 → 外观」点「导入 CSS…」，选好文件后，Jotiva 会保存一份副本，并在现有窗口和新窗口里应用这套样式。默认变量最先加载，接着是应用样式，最后是导入的 CSS。CSS 文件不能超过 128 KB，资源访问遵守应用内置的规则。想恢复默认样式，可以从应用菜单选「主题 → 关闭自定义 CSS」。原生菜单和 macOS 窗口按钮仍由操作系统绘制。

```css
:root {
  --jot-color-accent: #6760ad;
  --jot-color-accent-fill: #6760ad;
  --jot-color-on-accent: #ffffff;
  --jot-color-sidebar: #f5f2fa;
  --jot-font-base: 13px;
  --jot-editor-font-size: 15px;
  --jot-sidebar-width: clamp(12rem, 24%, 17rem);
}

:root[data-jot-scheme="dark"] {
  --jot-color-sidebar: #262231;
  --jot-color-accent: #c6b8fd;
  --jot-color-accent-fill: #c6b8fd;
  --jot-color-on-accent: #1c1826;
}

.jot-editor[data-jot-context="quick"] {
  --jot-editor-size: 14px;
}
```

| 改哪里 | 入口 |
| --- | --- |
| 背景、文字、选中效果 | `--jot-color-bg`、`--jot-color-text`、`--jot-color-muted`、`--jot-color-hover`、`--jot-color-active`、`--jot-color-line` |
| 强调色、按钮及焦点 | `--jot-color-accent` 用于链接和图标。按钮填色与文字分别用 `--jot-color-accent-fill` 和 `--jot-color-on-accent`。焦点边框和光晕用 `--jot-color-focus` 和 `--jot-focus-ring`。 |
| 顶部与左侧区域 | `--jot-color-chrome`、`--jot-color-sidebar`、`--jot-sidebar-width`、`--jot-search-width` |
| 基础排版和操作尺寸 | `--jot-font-base`、`--jot-font-ui`、`--jot-type-small`、`--jot-icon-size`、`--jot-control-height` |
| 正文 | `--jot-editor-font-family`、`--jot-editor-font-size`、`--jot-quick-font-size`、`--jot-editor-pad-inline` |
| 标题和稿纸 | `--jot-editor-heading-ratio-1` 至 `--jot-editor-heading-ratio-6` 使用数值倍数。横线颜色用 `--jot-color-rule-paper`。 |
| 大纲 | `.jot-document-outline` 和 `--jot-outline-width`。标题缩进、命中高亮及强调色使用组件的 `--jot-outline-indent`、`--jot-outline-highlight` 和 `--jot-outline-accent`。 |
| 滚动条 | `--jot-scrollbar-thumb`、`--jot-scrollbar-thumb-hover`、`--jot-scrollbar-track` 和 `--jot-scrollbar-thumb-size` 控制颜色与滑块粗细。`--jot-scrollbar-width` 控制页面原生滚动条的可拖动宽度。正文与大纲由组件保留操作区域，共用上述滑块外观。 |
| 窗口和控件 | `.jot-workspace`、`.jot-document-window`、`.jot-titlebar`、`.jot-search`、`.jot-sidebar`、`.jot-settings`、`.jot-quick`、`.jot-statusbar`、`.jot-dialog`、`.jot-tooltip` |
| 弹窗背景 | `--jot-color-backdrop` 控制弹窗后方的遮罩。默认用半透明黑色压暗原来的界面，可在浅、深色主题中分别设置。 |
| 主窗或速记的编辑器 | `.jot-editor[data-jot-context="workspace"]`、`.jot-editor[data-jot-context="quick"]`。局部字号用 `--jot-editor-size`。 |

拖动分隔线后，主窗优先用用户保存的栏宽。双击分隔线清除这个值，恢复主题给的默认宽度。

以上入口是主题可依赖的名称。正文的字体、字号和表格工具栏几何通过组件公开排版 API 传给 DOM 和离屏布局估算。自定义排版使用表中的 token。组件生成的内部节点类名会随组件版本变化。 [默认 token](../src/styles/tokens.css)
