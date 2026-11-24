# Jotiva 使用手册

[项目首页](../README.md) · [English](en/user-guide.md)

Jotiva 可以直接打开电脑里的 Markdown（`.md`、`.markdown`）和 UTF-8 文本（`.txt`）。你可以在工作区整理资料，也可以把一张便签留在正在看的窗口旁。写下的内容是普通文件，换个编辑器也能继续打开。

## 安装

到 GitHub Releases 下载适合电脑的安装包。 [下载 Jotiva](https://github.com/lin037/jotiva/releases)

| 平台 | 安装方法 |
| --- | --- |
| macOS 14 或更新版本 | `universal` 同时适用 Apple 芯片和 Intel。打开 DMG，把 Jotiva 拖到「应用程序」。也可以双击 PKG，或解压 ZIP 后把 Jotiva 移到「应用程序」。 |
| Windows x64 | 运行 `-setup.exe`，按安装器提示操作。应用使用 Microsoft Edge WebView2。 |
| Linux x86_64 | Ubuntu 24.04 可用 `sudo apt install ./文件名.deb` 安装。也可以把 AppImage 保存到自己的目录，执行 `chmod +x ./文件名.AppImage`，再打开它。 |

macOS 下载包没有经过 Apple 公证，Windows 安装包没有代码签名。系统可能提示验证发布者。Linux 上，窗口管理与文件菜单的行为会随桌面环境变化。 [Tauri AppImage 说明](https://v2.tauri.app/distribute/appimage/)

## 打开一份文档

1. 下载 [HelloJotiva.md](HelloJotiva.md) 到自己的文件夹。在 Jotiva 按 `⌘O`（Mac）或 `Ctrl+O`（Windows、Linux），选中下载的文件。
2. 勾一下待办，在表格里补上一个想法。按 `⌘/` 或 `Ctrl+/` 可以在即时预览和 Markdown 源码之间切换。纯文本文件直接显示正文。
3. 停下输入后会自动保存，也可以按 `⌘S` 或 `Ctrl+S`。底部显示保存状态和文件位置。想熟悉大纲，可以点右侧标题跳到相应段落。
4. 加入一个本地目录后，侧栏会显示里面的文档。多份文档可以用标签切换，也可以右键选「在新窗口打开」，把两份内容并排看。

## 在另一个应用旁写便签

| 平台 | 窗口挂载 |
| --- | --- |
| macOS | 在「设置 → 窗口关联」授权辅助功能，允许 Jotiva 识别其他应用的窗口。 |
| Windows | 可以从 Jotiva 选目标窗口，也可以把文档拖到目标窗口。 |
| Linux | 兼容 EWMH 的 X11 会话支持挂载。原生 Wayland 可使用独立便签。全局按键能否触发取决于桌面会话。 |

点一下希望跟随的窗口，再按 `Control+Shift+Space`。写完按 `⌘Enter`（Mac）或 `Ctrl+Enter`（Windows、Linux）将焦点交还给已挂载的窗口。按 `Esc` 折叠成摘要。目标窗口关掉后，到「待重新挂载」或用全局搜索找回文档。

已有文档也可以右键选「挂载到窗口」。要把便签留在屏幕上，点图钉置顶。选择「在主窗口编辑」可以换到大窗口继续写。关闭已挂载的便签会保存并解除它的挂载。

**macOS 已打开辅助功能开关，仍然提示授权？** 退出 Jotiva 并从「应用程序」重新打开。如果刚替换过应用，展开提示里的「已打开开关，仍提示未授权？」，核对正在运行的是哪一份 Jotiva，再到「系统设置 → 隐私与安全性 → 辅助功能」更新该条目。

## 整理和找回文件

- 打开的本地文件保留在原处。移除工作区只撤掉侧栏入口。把文档拖到另一个工作区目录会移动磁盘上的文件。
- 新速记保存后可在「归档 → 其他」找到。可以加入自己的归档目录，再从编辑器底部的路径菜单移动便签。
- 如果别的应用也改了同一份文档，用「另存编辑并重载」把本次输入存到「其他」，再读取原文件。原文件暂时不可用时，Jotiva 会尝试显示上次保存的内容。

想备份速记，在「归档 → 其他」选一份便签，通过状态栏或右键菜单让文件管理器显示它。也可以直接到下面的位置。

| 平台 | 默认速记目录 |
| --- | --- |
| macOS | `~/Library/Application Support/io.github.lin037.jot.desktop/notes/其他/` |
| Windows | `%APPDATA%\io.github.lin037.jot.desktop\notes\其他\` |
| Linux | `~/.local/share/io.github.lin037.jot.desktop/notes/其他/`，设置了 `XDG_DATA_HOME` 时使用该目录。 |

### 让 Jev 推荐归档位置

Jev 归档默认关闭。启用后，Jotiva 在你主动点「用 Jev 归档」时把便签摘录和参与分类的目录名称、说明发给你配置的服务。你确认建议后，文档才会移动。

1. 在「设置 → 归档」填服务商提供的基础地址或完整接口地址，按需改模型名称。远程服务需要 HTTPS。本机可用 `http://127.0.0.1:端口` 或 `http://[::1]:端口`。
2. 查看当前地址的密钥状态，点「配置密钥」后在系统密码框输入。密钥分别存入 macOS 钥匙串、Windows 凭据管理器或 Linux 系统密钥环。Linux 需要 Secret Service、`secret-tool` 和 `zenity`。
3. 打开「归档」中的目录设置，勾选「参与 Jev 分类」并写下目录用途。在「其他」打开一张便签，点「用 Jev 归档」查看建议。文本较短或没有匹配时，可以自行选择目录。

## 按自己的习惯写

按 `⌘,`（Mac）或 `Ctrl+,`（Windows、Linux）打开设置。「外观」可以切换主题和字号，也能 [导入自己的 CSS](themes.md)。「快捷键」支持按动作名称或组合键搜索。点击键位，录入新组合后保存。

[查看三端快捷键](shortcuts.md)
