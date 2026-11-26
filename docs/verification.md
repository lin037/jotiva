# 运行测试

[开发者指南](developer-guide.md)

请在仓库根目录运行下面的命令。第一次运行前，先用 `pnpm install --frozen-lockfile` 安装依赖。

## 每次改动

| 场景 | 命令 | 检查内容 |
| --- | --- | --- |
| 类型与前端生产构建 | `pnpm build` | TypeScript、前端打包及依赖解析。 |
| 前端单元测试 | `pnpm test:unit` | 强调色、按钮文字和搜索高亮的可读性。 |
| Rust 代码检查 | `cargo fmt --check --manifest-path src-tauri/Cargo.toml` 与 `cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` | 格式和静态检查。 |
| Rust 行为测试 | `pnpm test:rust` | 文件操作、版本冲突、迁移和回滚，以及窗口状态和快捷键。 |

Rust 的共享文档与存储用例在三个桌面目标上运行。macOS 另编译 Objective-C 桥接层，Windows 直接链接 Win32，Linux 构建需要 GTK 与 WebKitGTK 的开发包。

GitHub Actions 在推送与 Pull Request 上运行前端构建、macOS、Windows、Linux 的 Rust 检查及 macOS 上的 WebKit 集成测试。Ubuntu 作业还会在隔离的 X11 桌面启动应用并回放。 [CI 结果](https://github.com/lin037/jotiva/actions/workflows/ci.yml)

## WebKit 集成

```sh
python3 -m venv .venv
source .venv/bin/activate
python3 -m pip install -r qa/requirements.txt
python3 -m playwright install webkit
pnpm test:web
```

浏览器脚本在本机加载应用前端和发布版 Worker/Wasm，用测试数据模拟文件与桌面接口。每组文档测试都用独立的浏览器上下文和测试数据。请使用与 Playwright 版本匹配的浏览器。排查问题时，可以用 `WEBKIT_EXECUTABLE` 指定兼容的 WebKit。

| 命令 | 主要检查 |
| --- | --- |
| `pnpm test:web:editor` | 多窗口合并和冲突保留、标签撤销历史、速记恢复和 Worker 回收。可单跑 `python3 qa/document-windows.py --sync`、`--history` 或 `--resume`，也可组合。 |
| `pnpm test:web:ui` | 文件树、窗口选择器和焦点导航。窗口测试模拟未授权、取消、授权后继续、使用中撤权和列表读取失败后重试。 |
| `pnpm test:web:visual` | 截图核对标题光标和滚动条。光标截图要在闪烁前捕捉，适合固定系统、缩放和负载的机器。 |
| `pnpm test:theme` | 检查主题的安装、同名更新和改名，以及全局 CSS 的格式化、保存和跨窗口生效。 |
| `pnpm test:shortcuts` | 窗口分派、焦点导航、按名称或键位查找、录入和保存。在编辑器中改键后，验证新旧组合、清除和恢复默认。 |
| `pnpm test:capacity` | 扫描包含大量真实文件的目录，并在 WebKit 中检查文件树分页、多个标签和长文滚动。 |
| `pnpm test:filing` | 用回环 HTTP 服务检查分类、超时、错误和确认后移动文件。WebKit 检查手动触发与取消，并模拟不同地址的密钥状态、更换和移除。无需服务密钥。 |
| `pnpm test:filing:live` | 可选的真实服务检查。在项目 `.env` 设置 `JEV_API_KEY`，命令会发三份合成便签到 Jev 官方接口，核对两个候选目录与无匹配结果。自定义端点可成对设置 `JOT_JEV_ENDPOINT` 和 `JOT_JEV_API_KEY`。 |
| `python3 qa/document-windows.py --history-compat` | 在临时产物中模拟缺少历史快照方法，验证已保存的正文在切标签时还能打开。故障注入依赖输出代码形状，作为升级诊断单跑。 |

单独运行上述分组前先用 `pnpm build` 生成 `dist/`。`pnpm test:web` 会构建一次并跑编辑器和界面两组。检查文件树或读写时，报告中注明使用了 IPC 夹具。系统焦点、实际文件权限和跨进程行为用真实桌面验证。

## Windows 桌面回放

在安装了 Microsoft Edge 和 WebView2 Runtime 的 Windows 桌面运行 `pnpm build`，然后运行 `node qa/windows-smoke.mjs`。回放使用独立的 Jotiva 和 Edge 配置，以及含 350 行中文的 UTF-8 CRLF 文本文件。保持桌面已解锁，让回放控制测试窗口的焦点。

脚本用系统按键切换标签，再用鼠标把一个标签拖到 Edge。它会检查标题栏按钮的响应范围、拖动和置顶，并用 Ctrl+End、Ctrl+Home、Ctrl+Z、Ctrl+Y、Ctrl+S 检查长文导航和编辑。折叠便签后，它会在 Edge 中按全局 Ctrl+Shift+Space，确认打开了一张空白便签，并核对原便签仍保留同步过的正文。随后，脚本在主窗口和挂载窗口分别修改同一文档，检查两边是否同步、宿主关闭后是否解除挂载，以及文件正文和换行是否正确。它也会单独检查原生菜单、文档窗口的最大化、还原与关闭，以及资源管理器再次挂载和选中文件的行为。

推送到 `verify/**` 分支会运行三端 CI。

## Linux 桌面回放

Ubuntu 24.04 和 WSL Ubuntu 可使用同一套命令。在 Linux 构建依赖之外安装 `xvfb xauth dbus-x11 openbox xdotool zenity weston thunar fonts-noto-cjk`。用 `pnpm build` 生成前端后运行

```sh
GDK_BACKEND=x11 WEBKIT_DISABLE_DMABUF_RENDERER=1 LIBGL_ALWAYS_SOFTWARE=1 \
  xvfb-run -a dbus-run-session -- python3 qa/linux-smoke.py
WEBKIT_DISABLE_DMABUF_RENDERER=1 LIBGL_ALWAYS_SOFTWARE=1 \
  xvfb-run -a dbus-run-session -- python3 qa/linux-smoke.py --wayland
```

回放在临时资料目录中用 WebKitGTK 打开 Markdown，修改并验证磁盘内容，打开设置与独立文档，用 Openbox 和一个临时外部应用验证 X11 挂载、关闭解绑、自绘标题栏的拖动和原生应用菜单。Wayland 用 Weston 验证独立便签与窗口控制。两种会话都检查发给 Thunar 的文件定位请求及目标路径。Windows 和 Linux 共用 WebView 内的断言。Debian 包生成后，用 `xvfb-run -a dbus-run-session -- python3 qa/linux-package.py 路径.deb` 验证安装包的文件关联。该用例通过 GIO 打开两份路径带空格的中文文档，再核对两份正文。

## macOS 桌面回放

需要已登录且未锁屏的 macOS 桌面。涉及窗口拖放和切换的回放会发送真实按键和鼠标事件，运行期间请暂停手动操作，并按系统提示授予测试程序所需的权限。快捷键录入和文件存储测试使用独立的 Jotiva 实例，可单独运行。原生脚本共享 `qa/.cache/target` 中的测试二进制，在同一台 Mac 上依次运行。

| 场景 | 命令 |
| --- | --- |
| 日常原生验收，含焦点、拖放、文件恢复、关闭保存 | `pnpm test:macos:smoke`。若默认热键被占用，运行 `python3 qa/macos/run.py --smoke --quick-key u`。 |
| 文件落盘、外部改动、移动失败和重启 | `pnpm test:macos:storage`。 |
| 快捷键录制、菜单响应、全局注册冲突和重启 | `pnpm test:macos:shortcuts`。在独立测试应用里把按键事件发给测试窗口，可分别核对菜单显示的键位和实际响应。 |
| 只确认隔离测试版能编译 | `python3 qa/macos/run.py --build-only`。 |
| 两张挂载便签的层级、编辑和关闭 | `python3 qa/macos/sibling-windows.py`，可加 `--main-open`。 |
| 从速记到搜索、从标签到独立窗口 | `python3 qa/scenarios/window_flows.py capture documents`。 |
| 双屏、宿主全屏、普通桌面切换 | `python3 qa/scenarios/window_flows.py displays fullscreen`，以及 `python3 qa/scenarios/window_flows.py spaces --space-display 1`。编号从 0 开始，选有两个普通桌面的屏幕。 |
| 速记冷、热唤起和回收后重建 | `python3 qa/macos/run.py --quick-key u`，记录机器、系统、样本数、计时起止点及原始范围。 |

排查原生子系统时可单编译 `qa/macos/context-menu.m` 或 `host-focus.m`，分别和生产端 `platform/macos/objc/native_ui.m` 或 `platform/macos/objc/identity.m` 链接。
