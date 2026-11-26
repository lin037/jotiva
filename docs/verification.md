# 测试验证与签名发布指南

[开发者指南](developer-guide.md) · [架构说明](architecture.md) · [English](en/verification.md)

本文档面向参与深度回归测试的贡献者与项目维护者，涵盖日常检查、Playwright WebKit 集成套件、三端桌面回放以及正式发布与签名配置。执行以下命令前，请先运行 `pnpm install --frozen-lockfile`。

---

## 1. 基础检查（每次改动）

| 检查项 | 命令 | 覆盖范围 |
| --- | --- | --- |
| 类型与构建 | `pnpm build` | TypeScript 类型检查与 Vite 生产打包。 |
| 前端单测 | `pnpm test:unit` | 强调色对比度、按钮文字与搜索高亮可读性。 |
| Rust 静态检查 | `cargo fmt --check --manifest-path src-tauri/Cargo.toml` 与 `cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` | 代码格式与 Clippy 零警告校验。 |
| Rust 行为测试 | `pnpm test:rust` | 文件原子写入、冲突/回滚、迁移、窗口状态与快捷键。 |

GitHub Actions 会在推送到 `main` 或 `verify/**` 分支、触发 Pull Request 时自动运行前端构建、三端 Rust 检查、macOS WebKit 集成测试以及 Ubuntu X11／Wayland 桌面回放。[查看 CI 工作流](https://github.com/lin037/jotiva/actions/workflows/ci.yml)

---

## 2. WebKit 集成测试

首次运行前配置 Python 测试环境与 Playwright WebKit：

```sh
python3 -m venv .venv
source .venv/bin/activate
python3 -m pip install -r qa/requirements.txt
python3 -m playwright install webkit
pnpm test:web
```

`pnpm test:web` 会先执行 `pnpm build`，再顺序运行 `test:web:editor` 与 `test:web:ui`。单独运行以下子套件前，请确保已执行过 `pnpm build`（如需指定浏览器可执行文件，可设置 `WEBKIT_EXECUTABLE`）：

| 命令 | 检查重点 |
| --- | --- |
| `pnpm test:web:editor` | 多窗口合并与冲突保留、标签撤销历史、速记恢复与 Worker 回收（支持拆跑 `python3 qa/document-windows.py --sync` / `--history` / `--resume`）。 |
| `pnpm test:web:ui` | 文件树、窗口选择器、焦点导航，以及辅助功能未授权/撤权/重试流程。 |
| `pnpm test:web:visual` | 截图核对标题光标与滚动条外观（需在固定系统、缩放与负载环境下运行）。 |
| `pnpm test:theme` | 主题安装、同名更新、改名，以及全局 CSS 格式化、保存与跨窗口同步。 |
| `pnpm test:shortcuts` | 窗口键位分派、焦点导航、快捷键搜索/录入/保存与恢复默认。 |
| `pnpm test:capacity` | 大目录扫描、文件树分页、多标签与长文滚动性能。 |
| `pnpm test:filing` | 使用本地回环服务校验 Jev 分类、超时、错误、密钥管理与确认移动（无需真实密钥）。 |
| `pnpm test:filing:live` | 可选的真实接口校验：在 `.env` 配置 `JEV_API_KEY`（自定义端点须设置 `JOT_JEV_ENDPOINT` 与 `JOT_JEV_API_KEY` 两个环境变量）。 |
| `python3 qa/document-windows.py --history-compat` | 升级诊断：模拟缺少历史快照接口时已保存正文的切标签兼容性。 |

---

## 3. 原生桌面回放（三端烟雾测试）

### Windows 桌面回放
在已安装 Microsoft Edge 与 WebView2 Runtime 且**桌面未锁屏**的 Windows 环境下，先运行 `pnpm build`，再执行 `node qa/windows-smoke.mjs`。脚本使用隔离配置与 350 行中文 CRLF 测试文件，自动覆盖标签拖拽挂载、长文编辑与快捷键、多窗口同步、宿主关闭解绑以及资源管理器文件选中。

### Linux 桌面回放
Ubuntu 24.04 或 WSL Ubuntu 除基础构建依赖外，需额外安装 `xvfb xauth dbus-x11 openbox xdotool zenity weston thunar fonts-noto-cjk`。运行 `pnpm build` 后执行：

```sh
GDK_BACKEND=x11 WEBKIT_DISABLE_DMABUF_RENDERER=1 LIBGL_ALWAYS_SOFTWARE=1 \
  xvfb-run -a dbus-run-session -- python3 qa/linux-smoke.py
WEBKIT_DISABLE_DMABUF_RENDERER=1 LIBGL_ALWAYS_SOFTWARE=1 \
  xvfb-run -a dbus-run-session -- python3 qa/linux-smoke.py --wayland
```

验证 `.deb` 安装包文件关联时，运行 `xvfb-run -a dbus-run-session -- python3 qa/linux-package.py <安装包路径.deb>`。

### macOS 桌面回放
需在**已登录且未锁屏**的 macOS 桌面运行，执行期间请暂停键鼠操作并按系统提示授权测试二进制（共享缓存位于 `qa/.cache/target`）：

| 场景 | 命令 |
| --- | --- |
| 日常原生验收 | `pnpm test:macos:smoke`（默认热键冲突时可用 `python3 qa/macos/run.py --smoke --quick-key u`） |
| 磁盘写入、外部冲突与重启恢复 | `pnpm test:macos:storage` |
| 快捷键录制、菜单与全局冲突 | `pnpm test:macos:shortcuts` |
| 仅编译隔离测试包 | `python3 qa/macos/run.py --build-only` |
| 多窗口便签层级与联动 | `python3 qa/macos/sibling-windows.py`（可加 `--main-open`）、`python3 qa/scenarios/window_flows.py capture documents` |
| 双屏、全屏宿主与空间切换 | `python3 qa/scenarios/window_flows.py displays fullscreen`、`python3 qa/scenarios/window_flows.py spaces --space-display 1` |
| 冷/热唤起与 WebView 重建 | `python3 qa/macos/run.py --quick-key u` |

---

## 4. 正式发布、签名与自动更新（维护者）

正式发版流程由 [Release packages 工作流](../.github/workflows/release.yml) 驱动：

1. 在 `main` 的目标提交上打版本标签 `vX.Y.Z`（确保与 `package.json`、`Cargo.toml`、`tauri.conf.json` 版本一致）。
2. 手动针对该标签运行 `Release packages` 并勾选 `publish`；预构建测试可通过推送 `build/vX.Y.Z` 分支触发（设置变量 `JOTIVA_SIGNED_RELEASE=true` 可在预构建分支启用签名验证）。
3. 工作流验证标签与 CI 后，生成 macOS Universal（`.dmg` / `.pkg` / `.zip`）、Windows x64 NSIS、Linux x86_64（`.deb` / `.AppImage`）、源码包、`latest.json`、`SHA256SUMS` 与 `HelloJotiva.zip` 并发布至 GitHub Releases。示例压缩包内含中英文 Markdown 和配图，解压后可离线打开。

### GitHub Actions 签名凭据清单

| 用途 | 仓库 Variables / Secrets 配置 |
| --- | --- |
| **Tauri 更新签名** | Variable `JOTIVA_UPDATER_PUBLIC_KEY`（公钥）；Secrets `TAURI_SIGNING_PRIVATE_KEY`、`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`（离线备份私钥，后续版本保持一致）。 |
| **macOS 签名与公证** | Secret `JOTIVA_MAC_CERTIFICATES`（含 Developer ID Application 与 Installer 的 Base64 `.p12`）、`JOTIVA_MAC_CERTIFICATE_PASSWORD`；Variables `APPLE_SIGNING_IDENTITY`、`JOT_INSTALLER_IDENTITY`、`APPLE_TEAM_ID`；Secrets `APPLE_ID`、`APPLE_PASSWORD`（应用专用密码）。 |
| **Windows 代码签名** | Secrets `JOTIVA_WINDOWS_CERTIFICATE`（Base64 PFX）、`JOTIVA_WINDOWS_CERTIFICATE_PASSWORD`；若改用云/硬件签名服务需替换工作流中的导入步骤。 |

> 验证 macOS 辅助功能权限跨版本延续时，需在同一台机器上由同一 Developer ID 签名团队、相同应用标识与安装路径的旧版本原地升级。
