# Jotiva 开发者指南

[项目首页](../README.md) · [使用手册](user-guide.md) · [English](en/developer-guide.md)

Jotiva 是用 Tauri 2 构建的 Markdown 桌面应用。TypeScript 负责窗口里的界面与编辑交互，Rust 负责文档状态、磁盘和窗口协调，Objective-C 对接 macOS。应用通过发布包使用编辑器组件 `@jot-md/editor`。首次接触项目时，从这个目录启动便能开发桌面功能。组件本身的语法和渲染实现在 [Jot Markdown Engine](https://github.com/lin037/jot-md-engine)。

## 从源码启动

三端均使用 Rustup、Node.js 24 和 pnpm。项目锁定 pnpm 11.11.0 和 Rust 1.100.0。macOS 需要 Xcode Command Line Tools，Windows 需要 MSVC C++ 桌面开发工具与 WebView2 Runtime，Linux 需要 WebKitGTK 4.1、GTK、AppIndicator、OpenSSL 和 libxdo 的开发包。 [Tauri 系统依赖](https://v2.tauri.app/start/prerequisites/)

在 Windows 和 WSL 同时开发时，两个系统各自安装前端依赖。WSL 可以把工作副本放在 Linux 用户目录，并设置 `CARGO_TARGET_DIR="$HOME/.cache/jotiva-target"`，这样 Vite 的原生模块与 Cargo 构建缓存都使用本端架构。X11 和 Wayland 的原生回放命令见[测试指南](verification.md#linux-桌面回放)。

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
| `qa/`、`showcase/` | 测试夹具和供人试用的虚构文档。 |

改保存时，从 `src/editor/note-editor.ts` 的保存队列往下读，经过 Rust 的 `save_note`，最后到 `Notes::save`。改速记时，从 `desktop.rs` 的宿主事件和视图转场往下读。磁盘布局、同步和窗口生命周期见[实现说明](architecture.md)。

## 代码约定

- TypeScript 沿用项目的严格类型检查。窗口专用 DOM 与样式跟着视图走，跨窗口共用后再放进 `ui/` 或 `editor/`。
- 改 IPC 时同时核对 Rust 命令和前端调用，包括字段、事件作用范围、失败反馈。窗口 ID 和文档 ID 的含义不同，按各自生命周期使用。
- Rust 用 `rustfmt` 格式化。涉及文件写入、重命名和删除时，优先复用 `library/` 与 `storage.rs` 的既有入口。覆盖失败、重复触发和重启恢复。
- 系统回调和窗口显示遵守 macOS 主线程约束。慢磁盘操作与网络请求沿用现有后台执行路径。原生代码与 TypeScript 都要保留可供用户处理的错误。
- 升级编辑器时，更新依赖与锁文件。用实际安装的编辑器依赖跑 WebKit 回归，检查切标签、输入法、撤销、大纲、公式和图表。 [运行测试](verification.md)

## 提交问题和改动

提问题时带上系统版本、Jotiva 版本、最短的 Markdown 样例、操作步骤及预期与实际结果。涉及外部窗口时补记宿主应用、屏幕和全屏状态，涉及丢失正文时写清文件在什么位置、哪个应用最后保存，以及 Jotiva 的状态提示。发截图或日志前移除个人路径、标题、正文和密钥。

从默认分支派生工作分支，提交一个能独立验证的改动并发 Pull Request。写明改动、验证结果和相关 Issue。操作方式有变化时，同步更新使用手册。

## 许可证

Jotiva 源码采用 MIT 许可。第三方组件和字体保留各自的许可。更新依赖和锁文件后，用 Python 3.10 或更高版本运行 `python3 scripts/third-party-notices.py`，再将生成的声明随改动一起提交。 [MIT 许可证](../LICENSE) · [第三方许可](../src-tauri/notices/THIRD-PARTY-NOTICES.txt)
