<p align="center">
  <img src="src/assets/jot.svg" alt="Jotiva" width="76" height="76">
</p>
<h1 align="center">Jotiva</h1>
<p align="center"><strong>把便签留在你正在用的窗口旁。</strong></p>
<p align="center">在 macOS、Windows 和 Linux 上编辑本地 Markdown 和纯文本。</p>
<p align="center">
  <a href="https://github.com/lin037/jotiva/releases"><strong>下载安装</strong></a> ·
  <a href="docs/HelloJotiva.md">打开示例</a> ·
  <a href="docs/user-guide.md">使用手册</a> ·
  <a href="docs/developer-guide.md">开发者指南</a> ·
  <a href="README.en.md">English</a>
</p>
<p align="center">
  <a href="docs/user-guide.md#安装"><img src="https://img.shields.io/badge/macOS-14%2B-343940?logo=apple&amp;logoColor=white" alt="macOS 14+"></a>
  <a href="docs/user-guide.md#安装"><img src="https://img.shields.io/badge/Windows-x64-2B579A" alt="Windows x64"></a>
  <a href="docs/user-guide.md#安装"><img src="https://img.shields.io/badge/Linux-x86__64-83561B?logo=linux&amp;logoColor=white" alt="Linux x86_64"></a>
  <a href="https://github.com/lin037/jotiva/actions/workflows/ci.yml"><img src="https://github.com/lin037/jotiva/actions/workflows/ci.yml/badge.svg?branch=main" alt="CI"></a>
  <a href="https://v2.tauri.app/"><img src="https://img.shields.io/badge/Tauri-2-14788A?logo=tauri&amp;logoColor=white" alt="Tauri 2"></a>
  <a href="https://rust-lang.org/"><img src="https://img.shields.io/badge/Rust-B5411A?logo=rust&amp;logoColor=white" alt="Rust"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-426c5a" alt="MIT 许可证"></a>
</p>

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs/assets/jot-dark.webp">
    <img src="docs/assets/jot-light.webp" alt="Jotiva 正在编辑 HelloJotiva，左边是本地工作区，正文有待办和表格，右边是章节大纲" width="960">
  </picture>
</p>

## 简介

- **文档编辑。** 在电脑上打开 Markdown 或纯文本，改动会存回原文件。可以直接勾待办、改表格，或切到 Markdown 源码。
- **窗口便签。** 查资料时，把便签挂在浏览器旁。切到别的应用后，再回到浏览器就能接着记。窗口挂载支持 macOS、Windows 和兼容的 Linux X11 会话。
- **工作区。** 把常用文件夹放在侧栏。可以用标签切换文档，从大纲跳到要看的段落，或把两份文档各开一个窗口来对照。界面有浅色和深色，也可以导入自己的 CSS。

## 操作演示

<details>
<summary>窗口便签</summary>
<picture>
  <source media="(prefers-reduced-motion: reduce)" srcset="docs/assets/window-notes.webp">
  <img src="docs/assets/window-notes.gif" alt="在路线窗口旁打开便签。切到消息窗口时便签隐藏，回到路线窗口后再次显示。" width="840">
</picture>
</details>

<details>
<summary>勾待办、给表格加一行，再切换到 Markdown 源码</summary>
<img src="docs/assets/edit-markdown.gif" alt="在 HelloJotiva 勾掉待办、插入表格行，再查看 Markdown 源码" width="840">
</details>

<details>
<summary>搜索本地便签和文档</summary>
<img src="docs/assets/search-notes.gif" alt="打开全局搜索，输入关键词并打开匹配的 Markdown 文档" width="840">
</details>

下载这份示例笔记，用 Jotiva 打开。勾掉一项待办，或把表格里的路线换成你自己的。 [HelloJotiva.md](docs/HelloJotiva.md)

用得顺手，欢迎点个 **Star**。想分享你的用法，或者提个建议，可以到讨论区聊聊。 [讨论区](https://github.com/lin037/jotiva/discussions) · [贡献指南](CONTRIBUTING.md)
