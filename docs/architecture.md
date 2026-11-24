# Jotiva 实现说明

[开发者指南](developer-guide.md)

## 桌面平台边界

一次编辑有三个独立的结果：正文以同一版本写回磁盘、窗口以系统习惯展示、便签能够找到正确的宿主。三者分别由文档库、Tauri 窗口与菜单、平台宿主适配器负责。文档 ID 与窗口句柄独立，因此可以关闭宿主、手动重新挂载，或在另一个窗口继续编辑同一个文件。

| 能力 | macOS | Windows | Linux |
| --- | --- | --- | --- |
| 主窗、设置、独立文档 | AppKit 标题栏，主窗叠加系统红绿灯 | 自绘标题栏、右侧窗口控制、应用菜单、WebView2 | 自绘标题栏、应用菜单、WebKitGTK |
| 快捷键、设置与退出 | ⌘ 主键，应用菜单含设置与退出 | Ctrl 主键，工具菜单含设置，文件菜单含退出 | 同 Windows，全局快捷键依赖桌面会话 |
| 浮窗与宿主 | NSPanel、Spaces、Accessibility 及原有身份恢复 | Win32 枚举、选择、监听前台和关闭事件；同一运行期间按窗口关联 | X11 通过 EWMH 枚举、关联与监听外部窗口；原生 Wayland 会话使用独立便签 |
| 在文件管理器中定位 | Finder 选中文件 | Windows Shell 定位并选中文件 | Tauri opener 调用桌面文件管理服务 |
| 保存时检验被替换的版本 | Darwin 文件交换 | ReplaceFileW 保留被替换的文件 | Linux renameat2 文件交换 |
| Jev 凭据 | 钥匙串 | Windows 凭据管理器 | 使用 Secret Service、secret-tool 和 zenity |

业务代码调用 [platform](../src-tauri/src/platform/mod.rs) 导出的操作。[HostBackend 和 FileManager](../src-tauri/src/platform/contract.rs) 分别定义宿主和文件操作的契约。macos、windows、linux 为同级模块，shared 只容纳复用的 Tauri 实现。编译目标选择实现，窗口构建时把平台交给 [前端平台配置](../src/desktop/platform.ts)。添加新能力时，先判断 Tauri 是否已提供窗口、菜单、对话框或文件管理器接口。涉及操作系统特有语义时，在适配器内实现，业务层继续使用相同的命令和数据格式。

## 窗口与编辑器

TypeScript 根据 Tauri 的窗口标签加载视图。主窗 `main`、设置 `settings`、速记 `quick` 和 `quick-*`、独立文档 `document-*` 共用同一个 HTML 入口。主窗同时只运行当前标签的编辑器和 Worker，切走时保存正文，再保留该标签的撤销快照、选区、阅读位置和编辑模式。打开另一扇独立文档窗会多出一份独立编辑会话。

速记由 `desktop.rs` 协调，额外速记窗在 `desktop/note_windows.rs` 管理。Rust 用递增的 `epoch` 标记视图切换，WebView 在准备好编辑、摘要或搜索界面后用同一个编号确认。旧确认会被丢弃。折叠便签后可回收 Worker，隐藏窗口经过空闲期后可销毁 WebView。仅菜单栏常驻时仍可以用热键唤起。 [窗口事件定义](../src-tauri/src/events.rs)

macOS 宿主观察使用 AppKit 和 Accessibility，Windows 通过 Win32 观察窗口，Linux 使用 X11/EWMH。macOS 能从宿主取得足够明确的身份时可恢复绑定。当前 Windows 和 Linux 的窗口标识用于当前会话，目标关闭后可从「待重新挂载」重新选择。标题可用于列表筛选。被动切回宿主会恢复便签的显示，键盘焦点仍留在宿主。 [宿主代码](../src-tauri/src/host.rs)

## 文档怎样保存

1. 输入先到编辑器，延时保存会等待 Worker 确认正文。切换、关闭和退出也会先处理待保存的输入。
2. Rust 用文档 ID 找到记录，检查版本及磁盘内容。替换 Markdown 前在同目录写入临时文件，检查旧内容后替换目标。记录提交失败时尝试恢复原文件。
3. Jotiva 的其他窗口收到更新后可以读取新版本。若本地和另一窗口修改了不同范围，前端会尝试合并。遇到重叠、外部编辑产生的保存冲突或目标无法读取时，界面可以把尚未保存的输入另存到「其他」。
4. 便签从「其他」移到用户目录时保留文档 ID。文档绑定与文件位置独立，解除挂载后仍能打开原文。

[编辑会话](../src/editor/note-editor.ts) · [文本合并](../src/editor/document-sync.ts) · [文件写入](../src-tauri/src/storage.rs)

## 本地数据

应用数据目录由 Tauri 的 `app_data_dir()` 解析。macOS 使用 `~/Library/Application Support/io.github.lin037.jot.desktop/`，Windows 使用 `%APPDATA%\io.github.lin037.jot.desktop`，Linux 默认使用 `$XDG_DATA_HOME/io.github.lin037.jot.desktop`（通常位于 `~/.local/share`）。

| 路径 | 内容 |
| --- | --- |
| `notes/其他/*.md` | 默认归档中的真实文档。新建的无名速记在首次保存正文后写入。 |
| `notes/<id>.json` | 便签正文快照、修订号、标题、归档路径和来源。 |
| `notes/window-bindings.json`、`notes/sources.json` | 窗口身份与手动登记的来源。 |
| `notes/closing-sources/` | 解除来源时的中间状态，用于下次启动继续处理。 |
| `workspace.json`、`folders.json` | 工作区目录，归档名称、路径、说明和分类选择。 |
| `appearance.json`、`settings.json` | 外观和快捷键。快捷键设置保存三项全局键位及用户改过的应用内键位。 |
| `filing.json` | Jev 归档开关、服务地址和模型名称。密钥由各平台的系统凭据存储管理。 |

归档设置页按表单里的地址查询密钥状态。WebView 只收到是否已配置或查询错误。输入和更换密钥通过原生密码框完成，保存归档偏好可以单独进行。

从系统文件管理器或工作区打开的 Markdown 和 UTF-8 文本文件留在原位置。保存时沿用原文件的 CRLF 或 LF 换行。注册工作区只是把路径存起来，展开目录时才读取那一级的子项。 [便签库](../src-tauri/src/library/notes.rs) · [工作区](../src-tauri/src/library/workspace.rs)

应用内默认键位由 `src/desktop/shortcut-commands.json` 和 `shortcut-editor.json` 定义。设置页、应用菜单和编辑器共用这两份清单。保存后，各窗口收到更新事件，已打开的文档也会使用新键位。系统编辑操作、方向键选择和特定控件的导航操作列在 `shortcut-reference.json`。 [快捷键说明](shortcuts.md)

## Jev 归档

```mermaid
sequenceDiagram
    actor U as 用户
    participant E as 编辑器
    participant R as 本机归档服务
    participant J as Jev
    U->>E: 在「其他」中选择「用 Jev 归档」
    E->>R: 保存便签，提交文档 ID 与修订号
    R->>R: 检查开关、正文长度和参与分类的目录
    R->>J: 正文摘录、目录名称与说明
    J-->>R: 候选目录编号或无匹配
    R-->>E: 显示建议
    U->>E: 确认移动
    E->>R: 应用所选建议
    R->>R: 核对便签与目标，移动 Markdown
```

归档对接 Jev 协议的 `choice` 类型。每次请求只有一道单选题 `questions.destination`。每个参与分类的目录占一个选项，键为 `folder_<id>`，说明包含目录名称和用途。服务在 `answers.destination` 返回 `type: "choice"` 和选中的 `choice`，例如 `folder_7`。选择 `other` 表示没有匹配。Jotiva 校验返回值，用本地目录记录查找磁盘路径。 [请求与响应处理](../src-tauri/src/settings/filing.rs)

自建服务可以使用路径前缀。例如基础地址 `https://example.org/team` 对应的 POST 地址是 `https://example.org/team/v1/systemone`。基础地址以 `/v1` 结尾时只追加 `/systemone`。「完整接口地址」直接使用填写的请求路径。远程接口必须使用 HTTPS。HTTP 仅接受本机回环地址 `127.0.0.1` 和 `[::1]`，可以指定端口。Jotiva 会直接连接这两种地址上的服务。密钥与最终的请求地址绑定，平台适配器从系统凭据存储读取密钥并设置 Bearer 认证，请求不跟随重定向。 [macOS 请求与钥匙串](../src-tauri/src/platform/macos/objc/filing.m) · [Windows 与 Linux 请求](../src-tauri/src/platform/shared/filing.rs)

每次网络请求限时 25 秒。连接中断、超时或 HTTP 502、503、504 会在 750 毫秒后重试一次，发送前再次核对请求是否有效。认证错误、限流和格式异常会在编辑器中提示。用户可以修改设置或稍后再发起一次。模拟服务的测试入口是 `pnpm test:filing`。 [测试说明](verification.md)
