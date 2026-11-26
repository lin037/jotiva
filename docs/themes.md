# 外观、排版与主题定制

[项目首页](../README.md) · [使用手册](user-guide.md) · [English](en/themes.md)

Jotiva 将 **「整套外观主题」** 和 **「个人排版偏好」** 拆开管理：你可以随时切换或分享主题配色，同时保留自己设好的字号、字体、标题比例和样式微调。主题仍可以调整默认的正文字号比例。

---

## 一、个人排版与内置外观

打开 **「设置 → 界面外观」** 与 **「设置 → 编辑选项」**，即可调整以下选项（所有已打开的窗口与内置终端会同步生效）：

- **外观模式与强调色**：选择浅色、深色或跟随系统；内置雾蓝、松绿、藤紫、陶土四种配色，也可以「自定…」按钮填充色（Jotiva 会自动计算保证文字可读的对比度）。
- **界面主题**：内置 **默认**、**墨与钢**、**Material** 与 **蓝晒** 四套主题，每套均提供浅色与深色版本。
- **独立于主题的个人排版**：
  - 基础字号（`11–18 px`）同步缩放界面控件、侧栏与便签；
  - 正文字体（系统字体 / 衬线 / 等宽）与 H1–H6 标题字号比例（`80%–240%`）；
  - 横线稿纸、标题折叠控件、代码块自动换行与主窗工具栏开关；
  - **全局样式微调**：直接输入或粘贴 CSS，点击左下角「**格式化**」整理缩进，再点右下角「**保存**」应用。切换任何主题时，这里保存的习惯都会保留；清空后保存即可移除。

---

## 二、导入与分享 CSS 主题

导入的主题是一份非空、UTF-8 编码的 `.css` 文件（上限 128 KB），方便像普通文件一样保存、分享与更新：

1. 在 **「设置 → 界面外观 → 我的主题」** 点击「**导入主题…**」，选择本地 `.css` 文件，Jotiva 会将其复制到应用主题目录，并以文件名（去掉 `.css`）作为主题名称。
2. 在「**界面主题**」列表中选中刚导入的主题即可即时换色。
3. **更新与改名**：重新导入同名 `.css` 时可选择更新现有主题或另存新名。正在使用的主题更新后会立即生效。在「我的主题」中重命名会同步修改已安装的主题文件。

---

## 三、主题编写参考（进阶）

### 快速起步：基于「墨与钢」改色
内置的 [墨与钢主题 CSS](assets/ink-and-steel.css) 和 [蓝晒主题 CSS](assets/blueprint.css) 都在文件顶部集中定义了浅色/深色色表。复制一份另存为新文件名，修改顶部几个变量即可得到一套完整主题：

| 想改什么 | 修改「墨与钢」开头对应的变量 |
| --- | --- |
| 强调色与按钮 | `--ink-accent`（同时影响链接、流程图边框与甘特图任务条）、`--ink-on-accent`（按钮及任务条上的文字色） |
| 纸张与侧栏底色 | `--ink-paper`（编辑区背景）、`--ink-panel`（侧栏背景） |
| 正文与辅助文字 | `--ink-text`（正文与图内文字）、`--ink-muted`（次要文字与连线） |

### 通用主题变量（`--jot-*`）
如果需要跨主题通用（或直接写进「全局样式微调」），推荐覆盖公共 `--jot-*` 变量，详见 [默认 token 定义](../src/styles/tokens.css)：

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

| 定制范围 | 常用入口 |
| --- | --- |
| **基础配色** | `--jot-color-bg`、`--jot-color-text`、`--jot-color-muted`、`--jot-color-hover`、`--jot-color-active`、`--jot-color-line`、`--jot-color-backdrop` |
| **强调与焦点** | `--jot-color-accent`、`--jot-color-accent-fill`、`--jot-color-on-accent`、`--jot-color-focus`、`--jot-focus-ring` |
| **布局与字号** | `--jot-color-chrome`、`--jot-color-sidebar`、`--jot-sidebar-width`、`--jot-outline-width`、`--jot-font-base`、`--jot-editor-scale`、`--jot-quick-scale` |
| **正文与稿纸** | `--jot-editor-font-family`、`--jot-editor-font-size`、`--jot-quick-font-size`、`--jot-editor-pad-inline`、`--jot-color-rule-paper`、`--jot-editor-heading-ratio-1`–`6` |
| **代码块与引用** | `--jot-color-surface`（行内代码底色）、`--jot-color-quote`、`--jot-color-line-strong` |
| **Mermaid 图表** | 流程图/时序图等自动映射 `--jot-color-surface`、`--jot-color-accent`、`--jot-color-text`、`--jot-color-muted`；多系列图（饼图/Git/思维导图/时间线）使用 `--jot-color-series-1` 至 `12` |
| **窗口与局部范围** | `.jot-workspace`、`.jot-document-window`、`.jot-quick`、`.jot-titlebar`、`.jot-sidebar`、`.jot-settings`、`.jot-editor[data-jot-context="workspace"]`、`.jot-editor[data-jot-context="quick"]` |

> [!NOTE]
> 拖动主窗分隔条保存栏宽后，会优先使用你拖出的宽度；双击分隔条可恢复主题默认宽度。请尽量基于上述公开 token 编写样式，编辑器内部生成的节点类名可能随版本调整。
