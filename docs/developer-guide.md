# Jotiva 开发者指南

[项目首页](../README.md) · [使用手册](user-guide.md) · [English](en/developer-guide.md)

Jotiva 是一个 Tauri 2 桌面应用。界面和编辑会话由 TypeScript 管理，Rust 负责文件读写、文档状态和窗口切换。平台相关的代码放在各自的模块里。Markdown 编辑器来自锁定版本的 Jot Markdown Engine。 [Jot Markdown Engine](https://github.com/lin037/jot-md-engine)

## 从源码启动

三端均使用 Rustup、Node.js 24 和 pnpm。项目锁定 pnpm 11.11.0 和 Rust 1.100.0。macOS 需要 Xcode Command Line Tools，Windows 需要 MSVC C++ 桌面开发工具与 WebView2 Runtime，Linux 需要 WebKitGTK 4.1、GTK、AppIndicator、OpenSSL 和 libxdo 的开发包。 [Tauri 系统依赖](https://v2.tauri.app/start/prerequisites/)

如果同时在 Windows 和 WSL 开发，请分别安装前端依赖。WSL 的代码副本可以放在 Linux 用户目录，用该环境的依赖和 Cargo 缓存。 [Linux 桌面回放](verification.md#linux-桌面回放)

在包含本项目 `package.json` 的目录运行。

```sh
pnpm install --frozen-lockfile
pnpm tauri dev
```

开发窗口从 Vite 的 `127.0.0.1:1420` 加载页面。端口被占用时，先检查是否已有一个开发实例在运行。`pnpm dev` 只能在浏览器里看界面。要调试文件、菜单或系统窗口，运行 `pnpm tauri dev`。安装依赖需要访问 npm、Cargo 注册表和编辑器组件的 GitHub Release 文件。提交改动时请保留 `pnpm-lock.yaml` 和 `src-tauri/Cargo.lock`。

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
| `qa/`、`docs/HelloJotiva.md` | `qa/` 放自动测试和桌面验证脚本。HelloJotiva 是可下载的示例笔记。 |

要改保存行为，从 `src/editor/note-editor.ts` 的保存队列开始看，再沿着 Rust 的 `save_note` 找到 `Notes::save`。要改速记窗口，先看 `desktop.rs` 怎样处理窗口事件和切换视图。 [磁盘路径、同步与窗口管理](architecture.md)

## 新增或修改平台适配

外部窗口和文件管理的接口定义在 `src-tauri/src/platform/contract.rs`。`platform/mod.rs` 根据编译目标选择 macOS、Windows 或 Linux 模块。Windows 和 Linux 共用的窗口、对话框、快捷键和网络操作放在 `platform/shared/`。扩展时先看 Tauri 是否已有对应的 API。平台专用的系统调用放进相应的平台目录。

要给窗口挂载增加一种系统事件，在平台模块里接收事件，由共用的 `desktop.rs` 决定显示便签、隐藏便签，还是把它留在「待重新挂载」。要改底层文件替换，从 `storage.rs` 找到对应的平台实现。修订检查、快照和恢复仍通过文档库的原有入口处理。 [桌面平台与保存流程](architecture.md)

## 代码约定

- TypeScript 沿用项目的严格类型检查。窗口专用 DOM 与样式跟着视图走，跨窗口共用后再放进 `ui/` 或 `editor/`。
- 改 IPC 时同时核对 Rust 命令和前端调用，包括字段、事件作用范围、失败反馈。窗口 ID 和文档 ID 的含义不同，按各自生命周期使用。
- Rust 用 `rustfmt` 格式化。写入、重命名或删除文件时，优先复用 `library/` 与 `storage.rs` 的既有入口。检查操作失败、执行两次和重启后恢复的结果。
- 系统调用放在对应的平台模块里。遵守各平台的界面线程要求，耗时操作沿用现有的后台执行方式。报错时，要让用户知道出了什么问题以及能做什么。
- 升级编辑器时，更新依赖与锁文件。用实际安装的编辑器依赖跑 WebKit 回归，检查切标签、输入法、撤销、大纲、公式和图表。 [运行测试](verification.md)

## 提交问题和改动

报告问题时，请附上系统和 Jotiva 的版本、能复现问题的最小 Markdown 示例、操作步骤，以及预期和实际结果。窗口问题请注明关联的应用、屏幕和全屏状态。如果正文丢失，请说明文件放在哪、最后用哪个应用保存，以及 Jotiva 当时显示了什么。发截图或日志前，请删掉私人路径、标题、正文和密钥。

提交 Pull Request 时，写清改了什么、怎么验证的，如果有相关 Issue，也请附上链接。操作方式变了，就一起更新使用手册。

## 分支与发布

`main` 是唯一长期维护的分支。每项改动从最新的 `main` 开分支，通过 Pull Request 合入 `main`。等 CI 通过后再合并。合并后删除远端工作分支，本地可运行 `git fetch --prune origin` 清理已删除的远端分支引用。

| 分支 | 用途 |
| --- | --- |
| `feat/<主题>`、`fix/<主题>` | 功能、修复和日常维护。分支名写改动内容，例如 `fix/windows-window-focus`。 |
| `verify/<主题>` | 需要推送后立即运行跨平台 CI 时使用。验证结束后，确认需要保留的改动已合入 `main`，再删除分支。 |
| `release/X.Y.Z` | 需要集中准备版本号和发布说明时使用。发布 PR 合并后删除。 |
| `build/vX.Y.Z` | 需要预打包时使用。推送会触发 `Release packages`，生成安装包供验证。确认产物已验收且需要保留的改动已合入 `main` 后删除。 |

正式发布时，在 `main` 上要发布的提交打 `vX.Y.Z` 标签。针对该标签手动运行 `Release packages`，勾选 `publish`。工作流会校验标签和代码版本，运行 CI，通过后将安装包发布到 GitHub Releases。用版本标签定位历史版本。 [CI](https://github.com/lin037/jotiva/actions/workflows/ci.yml) · [Release packages](https://github.com/lin037/jotiva/actions/workflows/release.yml)

## 许可证

Jotiva 源码采用 MIT 许可。第三方组件和字体保留各自的许可。更新依赖和锁文件后，用 Python 3.10 或更高版本运行 `python3 scripts/third-party-notices.py`，再将生成的声明随改动一起提交。 [MIT 许可证](../LICENSE) · [第三方许可](../src-tauri/notices/THIRD-PARTY-NOTICES.txt)
