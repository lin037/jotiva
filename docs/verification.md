# 运行测试

[开发者指南](developer-guide.md)

所有命令从桌面项目根目录运行。首次运行先用 `pnpm install --frozen-lockfile` 安装依赖。

## 每次改动

| 场景 | 命令 | 证据 |
| --- | --- | --- |
| 类型与前端生产构建 | `pnpm build` | TypeScript、前端打包及依赖解析。 |
| 前端单元测试 | `pnpm test:unit` | 强调色、按钮文字和搜索高亮的可读性。 |
| Rust 代码检查 | `cargo fmt --check --manifest-path src-tauri/Cargo.toml` 与 `cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` | 格式和静态检查。 |
| Rust 行为测试 | `pnpm test:rust` | 文件操作、版本冲突、迁移和回滚，以及窗口状态和快捷键。 |

Rust 构建会编译 macOS 的 Objective-C 代码，CI 要安排在 macOS。Rust 测试只在终端运行。

GitHub Actions 在推送与 Pull Request 上运行前端构建、macOS Rust 检查及 WebKit 集成测试。原生桌面回放可在本机单独运行。 [CI 结果](https://github.com/lin037/jotiva/actions/workflows/ci.yml)

## WebKit 集成

```sh
python3 -m venv .venv
source .venv/bin/activate
python3 -m pip install -r qa/requirements.txt
python3 -m playwright install webkit
pnpm test:web
```

浏览器脚本打开本机回环页面，使用真实的前端和发布版 Worker/Wasm，文件和桌面调用使用测试数据。每个文档分组重新创建独立的浏览器上下文和夹具。Playwright 与安装的浏览器版本需要匹配。定位问题时可以用 `WEBKIT_EXECUTABLE` 指定兼容的 WebKit。

| 命令 | 主要检查 |
| --- | --- |
| `pnpm test:web:editor` | 多窗口合并和冲突保留、标签撤销历史、速记恢复和 Worker 回收。可单跑 `python3 qa/document-windows.py --sync`、`--history` 或 `--resume`，也可组合。 |
| `pnpm test:web:ui` | 文件树、窗口选择器和焦点导航。窗口测试模拟未授权、取消、授权后继续、使用中撤权和列表读取失败后重试。 |
| `pnpm test:web:visual` | 截图核对标题光标和滚动条。光标截图要在闪烁前捕捉，适合固定系统、缩放和负载的机器。 |
| `pnpm test:theme` | 导入示例 CSS，核对编辑器、各窗口和弹窗遮罩在深浅色下的外观，验证重开和移除主题后的效果。 |
| `pnpm test:shortcuts` | 窗口分派、焦点导航、按名称或键位查找、录入和保存。在编辑器中改键后，验证新旧组合、清除和恢复默认。 |
| `pnpm test:capacity` | 扫描包含大量真实文件的目录，并在 WebKit 中检查文件树分页、多个标签和长文滚动。 |
| `pnpm test:filing` | 用回环 HTTP 服务检查分类、超时、错误和确认后移动文件。WebKit 检查手动触发与取消，并模拟不同地址的密钥状态、更换和移除。无需服务密钥。 |
| `pnpm test:filing:live` | 可选的真实服务检查。手动设置完整接口地址 `JOT_JEV_ENDPOINT` 和密钥 `JOT_JEV_API_KEY` 后单独运行，用三份合成便签核对两个候选目录与无匹配结果。 |
| `python3 qa/document-windows.py --history-compat` | 在临时产物中模拟缺少历史快照方法，验证已保存的正文在切标签时还能打开。故障注入依赖输出代码形状，作为升级诊断单跑。 |

单独运行上述分组前先用 `pnpm build` 生成 `dist/`。`pnpm test:web` 会构建一次并跑编辑器和界面两组。检查文件树或读写时，报告中注明使用了 IPC 夹具。系统焦点、实际文件权限和跨进程行为用真实桌面验证。

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

排查原生子系统时可单编译 `qa/macos/context-menu.m` 或 `host-focus.m`，分别和生产端 `native_ui.m` 或 `host/identity.m` 链接。
