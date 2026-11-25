# 外观和主题

在「设置 → 外观」可以选浅色、深色，或跟随系统。基础字号可设为 11–18 pt，标题栏控件、侧栏、对话框和速记都会跟着调整。显示缩放沿用桌面设置。

雾蓝、松绿、藤紫和陶土各有浅色与深色版本。选「自定…」可指定按钮填色。Jotiva 按填色亮度选用黑字或白字，同时给链接、选中图标和焦点边框生成适合当前背景的同色系颜色。内置界面的正文大小文字以 4.5∶1 为最低对比度。 [W3C 文字对比度说明](https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html)

「设置 → 编辑器」中，H1–H6 的字号分别按正文字号的百分比设置，范围为 80%–240%。每项标签会预览相应字号，点「恢复默认」可以还原。这里也能更换正文字体、开启横线稿纸、切换折叠控件、代码换行及主窗工具栏。主窗口和速记默认使用素纸。

在「设置 → 外观 → 界面主题」可选择默认、墨与钢、Material 或蓝晒。每套主题都有浅色和深色版本。蓝晒采用制图纸上的细网格标题栏、平直标签和等宽状态文字，浅色用雾蓝纸底，深色用藏青纸底。文件开头列出了纸张、表面、正文、辅助文字、主色和主色上的文字六项配色。 [蓝晒主题 CSS](assets/blueprint.css)

切换主题时会保留文档、光标和终端会话。导入的 CSS 最后应用，可继续覆盖主题。

## 导入 CSS

内置的「墨与钢」和这份可导入的文件共用一份 CSS。文件开头是浅色和深色两张色表，每张六项。复制文件后可以直接改色值。 [墨与钢主题 CSS](assets/ink-and-steel.css)

| 想改什么 | 修改「墨与钢」文件开头的变量 |
| --- | --- |
| 把金色换成绿色 | 把浅色的 `--ink-accent` 改成 `#85d5b6`，主按钮、链接、流程图边框和甘特任务条会一起换色。`--ink-on-accent` 是按钮和普通任务条上的文字色，换深色填充时宜配浅字。 |
| 换纸张和面板 | `--ink-paper` 控制编辑区底色，`--ink-panel` 控制侧栏，两者也参与流程图节点的配色。 |
| 换正文和辅助文字 | `--ink-text` 用于正文和图中文字，`--ink-muted` 用于次要文字和流程图连线。 |

浅色与深色可独立调整。只换浅色时，编辑第一张表即可。也可以先选内置「墨与钢」，再单独导入下面这三行，为浅色版换上薄荷绿。

```css
:root[data-jot-scheme="light"] {
  --ink-accent: #85d5b6;
}
```

在「设置 → 外观」点「导入 CSS…」，选好文件后，Jotiva 会保存一份副本，并在现有窗口和新窗口里应用这套样式。之后修改本地文件，重新导入即可看到效果。默认变量最先加载，接着是应用样式和选中的预设，最后是导入的 CSS。CSS 文件不能超过 128 KB，资源访问遵守应用内置的规则。想恢复所选预设，可以点「移除」。原生菜单和 macOS 窗口按钮仍由操作系统绘制。

## 编写自己的配色

从其他预设开始时，直接覆盖公共的 `--jot-*` 变量。下面这份 CSS 调整强调色、侧栏底色、栏宽和字号比例，也可以单独导入。

```css
:root {
  --jot-color-accent: #6760ad;
  --jot-color-accent-fill: #6760ad;
  --jot-color-on-accent: #ffffff;
  --jot-color-sidebar: #f5f2fa;
  --jot-editor-scale: 1.15;
  --jot-quick-scale: 1.08;
  --jot-sidebar-width: clamp(12rem, 24%, 17rem);
}

:root[data-jot-scheme="dark"] {
  --jot-color-sidebar: #262231;
  --jot-color-accent: #c6b8fd;
  --jot-color-accent-fill: #c6b8fd;
  --jot-color-on-accent: #1c1826;
}
```

| 改哪里 | 入口 |
| --- | --- |
| 背景、文字、选中效果 | `--jot-color-bg`、`--jot-color-text`、`--jot-color-muted`、`--jot-color-hover`、`--jot-color-active`、`--jot-color-line` |
| 强调色、按钮及焦点 | `--jot-color-accent` 用于链接和图标。按钮填色与文字分别用 `--jot-color-accent-fill` 和 `--jot-color-on-accent`。焦点边框和光晕用 `--jot-color-focus` 和 `--jot-focus-ring`。 |
| 顶部与左侧区域 | `--jot-color-chrome`、`--jot-color-sidebar`、`--jot-sidebar-width`、`--jot-search-width` |
| 基础排版和操作尺寸 | `--jot-font-base`、`--jot-font-ui`、`--jot-type-small`、`--jot-icon-size`、`--jot-control-height` |
| 字号比例 | 内置主题用 `--jot-editor-scale` 和 `--jot-quick-scale` 指定相对基础字号的倍数，设置和编辑器共用同一字号来源。 |
| 正文 | `--jot-editor-font-family`、`--jot-editor-font-size`、`--jot-quick-font-size`、`--jot-editor-pad-inline` |
| Markdown 代码、引用和链接 | 行内代码底色用 `--jot-color-surface`，引用底色和边线用 `--jot-color-quote` 和 `--jot-color-line-strong`，链接用 `--jot-color-accent`。 |
| Mermaid 流程图 | 节点底色用 `--jot-color-surface`，边框用 `--jot-color-accent`，标签用 `--jot-color-text`，连线用 `--jot-color-muted`。 |
| Mermaid 甘特图 | 普通任务条共用按钮的 `--jot-color-accent-fill` 和 `--jot-color-on-accent`，条外文字用正文色。网格用 `--jot-color-line-strong`，进行中的任务用 `--jot-color-active`，关键任务边框用 `--jot-color-error`。 |
| 时序图、类图、状态图、实体关系图 | 节点共用表面色，时序图的批注使用 `--jot-color-quote`、`--jot-color-text` 和 `--jot-color-line-strong`。 |
| 饼图、Git 分支图、思维导图、时间线 | 分类色使用 `--jot-color-series-1` 到 `--jot-color-series-12`。第一色跟随强调色，Git 使用前八色。例如覆盖 `--jot-color-series-2` 即可一起更换第二种扇区色和分支色。 |
| XY 图 | 背景、标题和坐标文字跟随主题。柱线色由 Mermaid 的嵌套参数 `xyChart.plotColorPalette` 控制，当前组件的 `themeVariables` 接口接收字符串、数值和布尔值。 |
| 标题和稿纸 | `--jot-editor-heading-ratio-1` 至 `--jot-editor-heading-ratio-6` 使用数值倍数。横线颜色用 `--jot-color-rule-paper`。 |
| 大纲 | `.jot-document-outline` 和 `--jot-outline-width`。标题缩进、命中高亮及强调色使用组件的 `--jot-outline-indent`、`--jot-outline-highlight` 和 `--jot-outline-accent`。 |
| 滚动条 | `--jot-scrollbar-thumb`、`--jot-scrollbar-thumb-hover`、`--jot-scrollbar-track` 和 `--jot-scrollbar-thumb-size` 控制颜色与滑块粗细。`--jot-scrollbar-width` 控制页面原生滚动条的可拖动宽度。正文与大纲由组件保留操作区域，共用上述滑块外观。 |
| 窗口和控件 | `.jot-workspace`、`.jot-document-window`、`.jot-titlebar`、`.jot-search`、`.jot-sidebar`、`.jot-settings`、`.jot-quick`、`.jot-statusbar`、`.jot-dialog`、`.jot-tooltip` |
| 弹窗背景 | `--jot-color-backdrop` 控制弹窗后方的遮罩。默认用半透明黑色压暗原来的界面，可在浅、深色主题中分别设置。 |
| 主窗或速记的编辑器 | `.jot-editor[data-jot-context="workspace"]`、`.jot-editor[data-jot-context="quick"]`。局部字号用 `--jot-editor-size`。 |

流程图跟随编辑器的字体和字号。甘特图的任务和分组文字跟随主题字体和配色，字号由 Mermaid 的 `gantt.fontSize` 和 `gantt.sectionFontSize` 单独控制。当前组件插件公开的主题接口只接收 `theme` 和 `themeVariables`。

拖动分隔线后，主窗优先用用户保存的栏宽。双击分隔线清除这个值，恢复主题给的默认宽度。

以上入口是主题可依赖的名称。正文的字体、字号和表格工具栏几何通过组件公开排版 API 传给 DOM 和离屏布局估算。自定义排版使用表中的 token。组件生成的内部节点类名会随组件版本变化。 [默认 token](../src/styles/tokens.css)
