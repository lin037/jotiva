# Jotiva 开发者指南

[项目首页](../README.md) · [使用手册](user-guide.md) · [架构说明](architecture.md) · [测试与发布](verification.md) · [English](en/developer-guide.md)

Jotiva 是一个基于 Tauri 2 构建的跨平台桌面应用。前端（TypeScript）负责界面视图、编辑会话与快捷键分发；后端（Rust）负责文件持久化、文档库状态、桌面宿主窗口关联、内置终端与系统适配。Markdown 核心编辑与增量解析来自锁版本的 [Jot Markdown Engine](https://github.com/lin037/jot-md-engine)。

---

## 本地构建与启动

### 环境依赖
- **Rust**：通过 `rustup` 安装（仓库 toolchain 文件锁定为 `1.100.0`）。
- **Node.js 24** 与 **pnpm 11.11.0**。
- **平台系统依赖**（参见 [Tauri 系统前置条件](https://v2.tauri.app/start/prerequisites/)）：
  - **macOS**：Xcode Command Line Tools
  - **Windows**：MSVC C++ 桌面开发工具与 Microsoft Edge WebView2 Runtime
  - **Linux**：WebKitGTK 4.1、GTK、AppIndicator、OpenSSL 与 `libxdo` 的开发包

> 若在 Windows 与 WSL 间同时开发，请在两个环境中分别安装前端依赖（WSL 的仓库副本建议放在 Linux 文件系统目录）。

### 启动开发实例

```sh
pnpm install --frozen-lockfile
pnpm tauri dev
```

`pnpm dev` 只启动 Vite，地址为 `127.0.0.1:1420`。前端依赖 Tauri 的窗口接口，本地启动应用请用 `pnpm tauri dev`。安装依赖需访问 npm、Cargo crates 与编辑器发布的依赖包；提交前请保持 `pnpm-lock.yaml` 与 `src-tauri/Cargo.lock` 同步更新。

---

## 代码目录导航

| 路径 | 职责说明 |
| --- | --- |
| `src/main.ts` | 按 Tauri 窗口标签挂载主窗、设置、速记或独立文档视图。 |
| `src/views/` | `workspace`（主窗、文件树、标签、内置终端）、`quick`（速记）、`document`（独立文档窗）与 `settings`（设置）。 |
| `src/editor/` | 编辑器生命周期、Worker 通信、保存队列、修订合并、图片处理与大纲（三个编辑入口共用 `note-editor.ts`）。 |
| `src/desktop/` | 前端 IPC 封装、快捷键清单与窗口焦点控制。 |
| `src/appearance/`、`src/ui/`、`src/styles/` | 主题与配色计算、通用组件与列表、CSS tokens 与全局样式。 |
| `src-tauri/src/main.rs`、`events.rs` | 注册 Tauri 命令、应用状态与前后端事件定义。 |
| `src-tauri/src/desktop.rs`、`desktop/note_windows.rs`、`windows.rs` | 窗口状态流转、宿主绑定、速记面板与独立文档窗口调度。 |
| `src-tauri/src/library/`、`storage.rs` | 便签库、工作区与归档索引、文件移动及原子写入替换。 |
| `src-tauri/src/images/`、`terminal.rs`、`updates.rs` | 图片保存/压缩/上传、PTY 终端会话与软件更新。 |
| `src-tauri/src/host.rs`、`platform/{macos,windows,linux}/`、`menus.rs` | 跨平台系统契约、宿主窗口适配与应用菜单。 |
| `src-tauri/src/settings/` | 外观、快捷键与 Jev 归档设置及凭据查询。 |
| `qa/`、`scripts/` | 单元测试、Playwright WebKit 套件、三端原生回放脚本与打包脚本。 |

更深入的设计边界与数据流转见 [架构说明](architecture.md)。

---

## 日常开发与检查

修改代码后，在仓库根目录运行：

```sh
pnpm build          # TypeScript 类型检查与前端生产构建
pnpm test:unit      # 前端单元测试
pnpm test:rust      # Rust 测试套件
```

提交 Rust 改动前，请确保格式化与静态检查通过：

```sh
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

- 涉及桌面窗口、WebKit 集成测试、三端烟雾测试或签名发版流程，请参阅 [测试与发布验证](verification.md)。
- 升级 `@jot-md/editor` 等依赖或修改锁文件后，请使用 Python 3.10+ 运行 `python3 scripts/third-party-notices.py`，将更新后的 [THIRD-PARTY-NOTICES.txt](../src-tauri/notices/THIRD-PARTY-NOTICES.txt) 一并提交。

---

## 工程约定

- **TypeScript**：遵守严格类型检查；仅在特定窗口使用的 DOM/样式留在对应 `views/` 子目录，跨视图复用的逻辑再下沉到 `ui/` 或 `editor/`。
- **IPC 边界**：修改前后端命令时同步核对字段类型、事件作用域与错误提示；严格区分窗口 ID 与文档 ID 的生命周期。
- **Rust 与文件安全**：保持 `rustfmt` 格式规范；文件写入、重命名与删除优先复用 `library/` 与 `storage.rs` 现有入口，并关注失败回滚与重启恢复行为。
- **平台适配**：系统调用收敛在 `src-tauri/src/platform/`，统一通过 `platform/contract.rs` 暴露给上层，优先复用 Tauri 官方 API；向用户报错时提供明确的上下文与可操作建议。

---

## 分支策略与贡献流程

`main` 为唯一长期分支。日常开发请从最新 `main` 拉取分支并通过 Pull Request 合入（等待 CI 全部通过后合并，合并后清理远端分支）。

| 分支命名 | 适用场景 |
| --- | --- |
| `feat/<主题>`、`fix/<主题>`、`docs/<主题>` | 新功能、缺陷修复、文档与日常维护。 |
| `verify/<主题>` | 需要推送后触发跨平台 CI 验证时使用，合入或验证完成后删除。 |
| `release/X.Y.Z` | 集中准备版本号与发布说明，发布 PR 合并后删除。 |
| `build/vX.Y.Z` | 触发 `Release packages` 预打包验证安装包，验收完成后删除。 |

参与流程与 Issue 规范见 [贡献指南](../CONTRIBUTING.md)。Jotiva 源码采用 [MIT 许可证](../LICENSE)。
