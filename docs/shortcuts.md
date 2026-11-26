# Jotiva 快捷键参考

[项目首页](../README.md) · [使用手册](user-guide.md) · [English](en/shortcuts.md)

**脱离鼠标**是 Jotiva 的核心设计之一。下表列出所有默认快捷键，你可以在 **「设置 → 快捷键」** 中搜索任意操作并修改键位。macOS 中的 `⌘`、`⌥`、`⌃`、`⇧` 分别对应 `Command`、`Option`、`Control`、`Shift`；Windows 与 Linux 共用一套应用内键位。

> [!TIP]
> **双手不离键盘的典型工作流**
> 1. **随窗速记**：在工作窗口旁按 `Control+Shift+Space` 唤出便签记一笔 → 按 `⌘Enter` / `Ctrl+Enter` 立即把焦点交还原窗口。
> 2. **找回现场**：事后在 Jotiva 按 `⌘K` / `Ctrl+K` 搜索便签，回车直接跳回原窗口，或选「在主窗口编辑」继续深写。
> 3. **面板与长文流转**：按 `F6`（或 `⌘1` / `⌘2` / `⌘3`）在正文、工作区、归档与大纲间跳转 → 按 `⌘/` / `Ctrl+/` 切换即时渲染与完整源码 → 在表格里用 `Tab` / `Enter` 导航并增删行列 → 按 `` ⌃⇧` `` / `` Ctrl+Shift+` `` 调出当前目录终端。

---

## 一、窗口与便签速记

| 操作 | macOS | Windows / Linux |
| --- | --- | --- |
| 新建便签（全局热键） | `Control+Shift+Space` | `Ctrl+Shift+Space` |
| 查找便签和文档（并跳回关联窗口） | `⌘K` | `Ctrl+K` |
| 返回宿主窗口 / 将焦点交还原窗口 | `⌘Enter` | `Ctrl+Enter` |
| 返回主窗口 | `` ⌘` `` | `` Ctrl+` `` |
| 平铺便签 | `⇧⌘O` | `Ctrl+Shift+F8` |
| 切换跨窗置顶 | `⌥⌘P` | `Ctrl+Alt+P` |
| 与主窗口并排 | `⌘\` | `Ctrl+\` |
| 在新窗口打开当前文档 | `⌥⌘O` | `Ctrl+Alt+O` |
| 折叠便签或退出搜索 | `Esc` | `Esc` |

> 在 Linux 上，全局热键、窗口挂载与并排能力依赖所使用的桌面会话（需兼容 EWMH 的 X11 会话；原生 Wayland 可使用独立便签）。

---

## 二、文件、工作区与内置终端

| 操作 | macOS | Windows / Linux |
| --- | --- | --- |
| 新建文档 / 新建独立文档窗口 | `⌘N` / `⇧⌘N` | `Ctrl+N` / `Ctrl+Shift+N` |
| 打开本机文档 | `⌘O` | `Ctrl+O` |
| 立即保存 | `⌘S` | `Ctrl+S` |
| 关闭当前标签或窗口 | `⌘W` | `Ctrl+W` |
| 切换到上一份文档 | `⌃Tab` | `Ctrl+Tab` |
| 直接聚焦：正文 / 工作区 / 归档 | `⌘1` / `⌘2` / `⌘3` | `Ctrl+1` / `Ctrl+2` / `Ctrl+3` |
| 在各区域间循环切换焦点（正文 / 文件树 / 大纲等） | `F6` / `⇧F6` | `F6` / `Shift+F6` |
| 从列表选择要跳转的焦点区域 | `⌥F6` | `Alt+F6` |
| 显示或隐藏左侧栏 | `⇧⌘L` | `Ctrl+Alt+L` |
| 打开当前文档所在目录的终端 | `` ⌃⇧` `` | `` Ctrl+Shift+` `` |
| 打开设置 / 退出 Jotiva | `⌘,` / `⌘Q` | `Ctrl+,` / `Ctrl+Q` |

**侧栏文件树键盘操作**：方向键移动选择或展开/折叠目录，`Enter` 打开选中文件，`F2` 原位重命名，`⌘⌫`（macOS）或 `Delete`（Windows / Linux）移到废纸篓 / 回收站。

---

## 三、即时渲染、长文导航与表格编辑

| 操作 | macOS | Windows / Linux |
| --- | --- | --- |
| 切换「即时预览」与「完整源码」 | `⌘/` | `Ctrl+/` |
| 打开编辑器命令面板 | `⌃⇧P` | `Ctrl+Shift+P` |
| 查找当前文档 / 查找与替换 | `⌘F` / `⌃⇧H` | `Ctrl+F` / `Ctrl+Shift+H` |
| 下一个 / 上一个匹配项 | `⌃G` / `⌃⇧G` | `Ctrl+G` / `Ctrl+Shift+G` |
| 跳转到指定行 | `⌃⇧L` | `Ctrl+Shift+L` |
| 折叠或展开当前标题章节 | `⌃⇧O` | `Ctrl+Shift+O` |
| 粗体 / 斜体 | `⌘B` / `⌘I` | `Ctrl+B` / `Ctrl+I` |
| 行内代码 / 插入或编辑链接 | `⌃⇧C` / `⌃⇧K` | `Ctrl+Shift+C` / `Ctrl+Shift+K` |
| 设为 1 至 6 级标题 | `⌃⇧1` – `⌃⇧6` | `Ctrl+Shift+1` – `Ctrl+Shift+6` |
| 切换任务列表 / 勾选或取消完成状态 | `⌃⇧9` / `⌘⌥Enter` | `Ctrl+Shift+9` / `Ctrl+Alt+Enter` |
| 撤销 / 重做 | `⌘Z` / `⇧⌘Z` | `Ctrl+Z` / `Ctrl+Y` |
| 跳到行首或行末 / 文首或文末 | `⌘←` `⌘→` / `⌘↑` `⌘↓` | `Home` `End` / `Ctrl+Home` `Ctrl+End` |

### 表格内全键盘编辑

| 操作 | macOS | Windows / Linux |
| --- | --- | --- |
| 跳到下一格 / 上一格 | `Tab` / `⇧Tab` | `Tab` / `Shift+Tab` |
| 跳到下方 / 上方单元格 | `Enter` / `⇧Enter`（或 `⌥↓` / `⌥↑`） | `Enter` / `Shift+Enter`（或 `Alt+↓` / `Alt+↑`） |
| 在下方插入行 / 在右侧插入列 | `⌃⇧↓` / `⌃⇧→` | `Ctrl+Shift+↓` / `Ctrl+Shift+→` |
| 删除当前行 / 删除当前列 | `⌘⌥⌫` / `⌘⌥⇧⌫` | `Ctrl+Alt+Backspace` / `Ctrl+Alt+Shift+Backspace` |
| 清空当前单元格内容 | `⌥⇧⌫` | `Alt+Shift+Backspace` |

---

## 四、自定义快捷键

在 **「设置 → 快捷键」** 中可通过操作名称或按键组合快速筛选。点击对应操作的键位按钮，按下新的组合键并点击「**保存快捷键**」即可生效（按 `Esc` 取消录入）。默认标为「未设置」的动作（例如**重排有序列表编号**、**展开全部章节**，以及全局的**查找便签和文档**、**靠右收拢或还原**）也可按需绑定快捷键。
