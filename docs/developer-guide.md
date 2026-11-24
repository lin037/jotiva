# Jotiva 开发者指南

[项目首页](../README.md) · [使用手册](user-guide.md) · [English](en/developer-guide.md)

Jotiva 用 Tauri 2 连接桌面窗口与编辑器。TypeScript 管理界面和编辑会话，Rust 管理文档、文件和窗口状态。系统差异由各自的平台模块处理，正文编辑使用锁定版本的组件。 [Jot Markdown Engine](https://github.com/lin037/jot-md-engine)

## 从源码启动

三端均使用 Rustup、Node.js 24 和 pnpm。项目锁定 pnpm 11.11.0 和 Rust 1.100.0。macOS 需要 Xcode Command Line Tools，Windows 需要 MSVC C++ 桌面开发工具与 WebView2 Runtime，Linux 需要 WebKitGTK 4.1、GTK、AppIndicator、OpenSSL 和 libxdo 的开发包。 [Tauri 系统依赖](https://v2.tauri.app/start/prerequisites/)

在 Windows 和 WSL 同时开发时，两端各自安装前端依赖。WSL 的工作副本可以放在 Linux 用户目录，便于用本端的依赖和 Cargo 缓存。 [Linux 桌面回放](verification.md#linux-桌面回放)

在包含本项目 `package.json` 的目录运行。

```sh
pnpm install --frozen-lockfile
pnpm tauri dev
```

开发窗口使用 Vite 的 `127.0.0.1:1420`。若端口已占用，先确认是否有另一个开发实例。只有 `pnpm dev` 时，浏览器页面缺少文件、菜单和系统窗口对应的 Tauri 接口。安装依赖需要访问 npm、Cargo 注册表及编辑器的 GitHub Release 文件。保留 `pnpm-lock.yaml` 和 `src-tauri/Cargo.lock`。

## 从功能找到文件

| 位置 | 改什么 |
| --- | --- |
| `src/main.ts` | 根据窗口标签加载主窗、设置、速记或独立文档。 |
| `src/views/` | `workspace`、`quick`、`document` 和 `settings` 各有自己的入口 `view.ts`。文件树和标签属于主窗，继续放在 `workspace/`。 |
| `src/editor/` | 管理编辑器会话、修订同步、保存、Worker 和排版。三个编辑入口共享 `note-editor.ts`。 |
| `src/desktop/` | 声明前端 IPC 数据和调用，封装快捷键、窗口操作与焦点。 |
| `src/ui/`、`src/appearance/`、`src/styles/` | 复用控件和便签列表，主题偏好与颜色计算，以及全局样式和 CSS 变量。 |
| `src-tauri/src/main.rs`、`events.rs` | 注册 Tauri 命令与全局状态，定义前后台交换的事件和视图数据。 |
| `src-tauri/src/desktop.rs`、`desktop/note_windows.rs`、`windows.rs` | 处理窗口状态、切换宿主、速记面板和独立文档窗口。 |
| `src-tauri/src/library/`、`storage.rs` | 管理便签、工作区、归档、索引和文件操作。底层文件写入由 `storage.rs` 提供。 |
| `src-tauri/src/host.rs`、`platform/{macos,windows,linux}/`、`menus.rs` | 宿主数据、各桌面适配器与应用菜单。AppKit 桥接源码位于 `platform/macos/objc/`。 |
| `src-tauri/src/settings/` | 保存外观、快捷键和其他应用偏好。 |
| `qa/`、`docs/HelloJotiva.md` | 自动回放与实机检查。HelloJotiva 是供用户下载并修改的示例。 |

改保存时，从 `src/editor/note-editor.ts` 的保存队列往下读，经过 Rust 的 `save_note`，最后到 `Notes::save`。改速记时，从 `desktop.rs` 的宿主事件和视图转场往下读。磁盘布局、同步和窗口生命周期见[实现说明](architecture.md)。

## 给桌面能力加一个平台实现

从 `src-tauri/src/platform/contract.rs` 查看外部窗口和文件管理接口。编译目标在 `platform/mod.rs` 选择 macOS、Windows 或 Linux 模块。`platform/shared/` 提供 Windows 和 Linux 已共用的窗口、对话框、快捷键和网络操作。扩展时先检查 Tauri 的公共 API，再把系统调用放入对应的平台目录。

例如给窗口挂载增加一个系统事件时，在平台模块接收事件，共用的 `desktop.rs` 决定便签显示、隐藏或留在「待重新挂载」。给保存增加一种文件系统行为时，从 `storage.rs` 进入平台文件替换实现。已有的修订检查、快照和恢复逻辑继续从文档库调用。 [桌面平台与保存流程](architecture.md)

## 代码约定

- TypeScript 沿用项目的严格类型检查。窗口专用 DOM 与样式跟着视图走，跨窗口共用后再放进 `ui/` 或 `editor/`。
- 改 IPC 时同时核对 Rust 命令和前端调用，包括字段、事件作用范围、失败反馈。窗口 ID 和文档 ID 的含义不同，按各自生命周期使用。
- Rust 用 `rustfmt` 格式化。涉及文件写入、重命名和删除时，优先复用 `library/` 与 `storage.rs` 的既有入口。覆盖失败、重复触发和重启恢复。
- 系统调用在当前平台适配器中实现。遵守各平台的界面线程要求，耗时操作沿用后台执行路径。原生代码与 TypeScript 都要返回可供用户处理的错误。
- 升级编辑器时，更新依赖与锁文件。用实际安装的编辑器依赖跑 WebKit 回归，检查切标签、输入法、撤销、大纲、公式和图表。 [运行测试](verification.md)

## 提交问题和改动

提问题时带上系统版本、Jotiva 版本、最短的 Markdown 样例、操作步骤及预期与实际结果。涉及外部窗口时补记宿主应用、屏幕和全屏状态，涉及丢失正文时写清文件在什么位置、哪个应用最后保存，以及 Jotiva 的状态提示。发截图或日志前移除个人路径、标题、正文和密钥。

从默认分支派生工作分支，提交一个能独立验证的改动并发 Pull Request。写明改动、验证结果和相关 Issue。操作方式有变化时，同步更新使用手册。

## 许可证

Jotiva 源码采用 MIT 许可。第三方组件和字体保留各自的许可。更新依赖和锁文件后，用 Python 3.10 或更高版本运行 `python3 scripts/third-party-notices.py`，再将生成的声明随改动一起提交。 [MIT 许可证](../LICENSE) · [第三方许可](../src-tauri/notices/THIRD-PARTY-NOTICES.txt)
