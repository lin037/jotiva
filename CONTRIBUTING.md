# 参与贡献（Contributing）

感谢你关注并参与 Jotiva！无论是修正文档、补充测试用例、修复 Bug，还是提交功能改进，我们都非常欢迎。

[English](docs/en/contributing.md) · [开发者指南](docs/developer-guide.md) · [测试与发布验证](docs/verification.md)

## 开始之前

- 建议先浏览现有的 [Issues](https://github.com/lin037/jotiva/issues) 与 [Discussions](https://github.com/lin037/jotiva/discussions)，确认是否已有相关讨论。
- 涉及文件格式、保存/合并逻辑或跨平台窗口行为的改动，请在 Issue 或 PR 中先说明具体使用场景与预期表现。

## 提交 Pull Request

1. 从最新的 `main` 分支拉取工作分支（外部贡献者请先 Fork 仓库）。分支命名建议：`feat/<主题>`、`fix/<主题>` 或 `docs/<主题>`。
2. 保持每次提交聚焦、可独立审查：
   - 如果改动影响了用户可见的操作、快捷键或设置项，请同步更新 [使用手册](docs/user-guide.md)（及对应英文文档）。
   - 如果改动更新了 `package.json` 或 `Cargo.toml` 依赖，请一并提交锁文件，并运行 `python3 scripts/third-party-notices.py` 更新第三方许可声明。
3. 在合并前确认本地基础检查通过（`pnpm build`、`pnpm test:unit`、`pnpm test:rust`），并在 PR 描述中写清**改动动机、验证环境与实际测试结果**；涉及界面改动请附截图，涉及文件写入请说明异常中断或重启后的恢复情况。

## 报告问题

提交 Issue 时，请尽量提供：
- Jotiva 版本、操作系统版本与架构（macOS / Windows / Linux；Linux 请注明 X11 或 Wayland）。
- 可公开分享的最小复现步骤或最小 Markdown 示例。
- 涉及窗口挂载问题时，请附上目标宿主应用名称、是否多屏以及是否全屏。

> [!NOTE]
> 上传截图或日志前，请先移除个人目录路径、敏感标题、正文内容与 API 密钥。如发现安全漏洞，请勿公开提交 Issue，而是参考 [安全政策](SECURITY.md) 私下报告。
