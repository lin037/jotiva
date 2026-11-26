<p align="center">
  <img src="src/assets/jot.svg" alt="Jotiva" width="76" height="76">
</p>
<h1 align="center">Jotiva</h1>
<p align="center"><strong>贴在工作窗口旁的 Markdown 速记本——先顺手记下灵感，再让模型帮你整理归档</strong></p>
<p align="center">免费、开源的跨平台桌面 Markdown 笔记与写作软件（macOS · Windows · Linux）</p>
<p align="center">
  <a href="https://github.com/lin037/jotiva/releases"><strong>下载安装</strong></a> ·
  <a href="docs/HelloJotiva.md">示例笔记</a> ·
  <a href="docs/user-guide.md">使用手册</a> ·
  <a href="docs/shortcuts.md">快捷键</a> ·
  <a href="docs/themes.md">主题定制</a> ·
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
  <img src="docs/assets/screenshots/macos-themes.webp" alt="macOS 上的 Jotiva 英文界面，展示默认浅色、墨与钢浅色、蓝晒深色和一张窗口便签" width="960">
</p>

## 为什么做 Jotiva：不只是一个普通编辑器

普通的 Markdown 编辑器只解决「打开文件之后怎么排版」，但真实的工作往往从**还没建文件的时候**就开始了：你在浏览器查报错、在文档里翻接口，突然要记几行线索；随手记了一堆零散笔记，事后懒得一个个建文件夹分类；写长文时频繁在键盘和鼠标之间来回切，粘贴截图还要手动整理图片路径。

Jotiva 围绕真实的高频痛点，把五个核心体验做成了闭环：

| 核心特点 | 在 Jotiva 里的体验 | 解决什么痛点 |
| --- | --- | --- |
| **1. 便签速记** | 按 `Control+Shift+Space` 在当前窗口旁贴一张便签，切走自动隐藏，切回原窗口自动浮现；从便签还能直接跳回当时的窗口。 | 不用反复切屏找笔记窗口，也不用忍受全局置顶浮窗挡住视线。 |
| **2. Jev 智能管理** | 随手记下的便签先自动落盘，写完点一下「用 Jev 归档」，根据正文内容与你的目录说明自动推荐归属文件夹。 | 记笔记时零分类负担，整理时不用手动翻目录找去处。 |
| **3. 全程脱离鼠标** | 从唤起便签、交还窗口焦点、搜索跳转、切换面板焦点（`F6`）到增删表格行列、调出内置终端，全流程纯键盘完成。 | 双手不离键盘，写代码、查资料与记笔记的节奏不会被打断。 |
| **4. 即时排版渲染** | Rust/Wasm 引擎增量解析，光标所在处展开语法，移开即恢复排版；表格、LaTeX 公式、Mermaid 图表直接原位渲染与编辑。 | 告别左右分栏占屏幕，长文秒开、打字不卡顿，所见即所得。 |
| **5. 目录级图片管理** | 按文件夹配置存图策略（跟随文档子目录、共用图片目录或 HTTP 图床上传），子目录自动继承，支持静态图压缩。 | 实验笔记、个人博客、项目文档各用各的存图规则，粘贴截图即插即用。 |

---

## 五大核心能力详解

### 1. 便签速记：贴在正在用的窗口旁，记住内容也记住现场

比如你同时开了两个浏览器窗口，一个查报错，一个看接口文档。在各自窗口按下 `Control+Shift+Space`，就能给两个窗口分别贴上专属便签：

- **跟窗显隐，不挡视线**：同一软件的不同窗口可以挂不同便签，一个窗口也能挂多张。切到别处时，未置顶的便签自动藏起；切回原窗口时自动现身。按 `Esc` 可折叠成一行摘要，点图钉可跨窗置顶；侧栏里的已有文档也能直接挂到窗口旁对照看。
- **从便签找回当时的窗口**：按 `⌘K` / `Ctrl+K` 搜索时，结果按「当前窗口 / 其他窗口便签 / 文档」清晰分组。选中一张已挂载便签，Jotiva 会直接带你回到它绑定的那个窗口；就算原窗口关了，也能在「待重新挂载」里找回。
- **天然就是本地 `.md` 文件**：在便签菜单选「在主窗口编辑」，刚才随手记的几句直接变成主窗口里的文档继续深写，不用复制粘贴。

### 2. Jev 管理与本地工作区：随手记零负担，归档有条理

Jotiva 直接读写本机硬盘上的 `.md`、`.markdown` 和 `.txt` 文件，并把「常驻项目（工作区）」和「临时速记（归档）」分开管理：

- **先记下来，再让 Jev 帮你分拣**：新建便签默认保存在「归档 → 其他」，不用在动笔前纠结存哪儿。开启可选的 **Jev 归档**（默认关闭）后，只需给你的归档文件夹写一句用途说明；点击「用 Jev 归档」时，Jotiva 会根据笔记正文与目录说明推荐最匹配的文件夹，经你确认后一键移入。
- **零侵入管理本地项目**：把常用的代码库、博客或笔记目录加到「工作区」即可直接增删改或拖拽移动文件；从工作区移除目录只会撤掉侧栏入口，绝不删硬盘文件，无缝配合 Git 使用。
- **多窗口防冲突保存**：支持开启多个独立文档窗并排对照。多窗口改同一文件的不同位置会自动合并；若遇外部修改或冲突，可一键「另存编辑并重载」，绝不丢稿。

### 3. 脱离鼠标：双手不离键盘的流畅流转

从记下第一笔到整理归档，所有高频操作都针对纯键盘流做了专门设计：

- **秒进秒出不抢焦点**：`Control+Shift+Space` 唤出便签，写完按 `⌘Enter`（macOS）或 `Ctrl+Enter`（Windows / Linux），焦点立刻回到原工作窗口，继续在浏览器或 IDE 里操作。
- **无死角面板与编辑导航**：按 `⌘K` / `Ctrl+K` 全局搜索便签与文件；按 `F6`（或 `⌘1` / `⌘2` / `⌘3`）在正文、工作区文件树、归档与大纲之间快速切换焦点；在表格内直接用 `Tab` / `Enter` 与快捷键移动和增删行列。
- **按键随手打开内置终端**：需要预览博客或跑脚本时，按 `` Ctrl+Shift+` `` 直接在当前文档所在目录呼出终端，切换文档不断连；自动化脚本还能通过环境变量 `JOTIVA_INBOX_DIR` 把生成的 Markdown 直接送进收件箱。
- **命令面板与自由改键**：按 `⌃⇧P` / `Ctrl+Shift+P` 调出编辑器命令面板，常用快捷键均可在「设置 → 快捷键」中按个人习惯改键。

### 4. 即时渲染：原位编辑，长文秒开

不管是小便签还是主窗口，底层都采用同一套自研编辑器：由 CodeMirror 处理输入与撤销，由 Rust/Wasm 引擎在后台线程做增量解析。

- **干净的原位编辑（Live Preview）**：不用开左右分栏。只有光标移进的段落才展开 Markdown 标记，移开立刻恢复排版；直接点击表格改单元格、勾选方框完成待办。想看完整源码时，按 `⌘/` 或 `Ctrl+/` 一键切换。
- **完整支持技术写作**：原生支持行内与块级 LaTeX 数学公式、代码块（可切自动换行或横向滚动）、自动跟随浅/深色主题换色的 Mermaid 图表（流程图、时序图、甘特图等）、章节折叠与右侧文档大纲。
- **视口优先渲染与状态保留**：打字先上屏，后台只增量解析改动部分并优先渲染眼前这一屏。切换标签再回来时，之前的阅读位置、选中文本和撤销记录全部原样保留。

### 5. 图片管理：按目录定制规则，粘贴即存好

不同文档对图片的存放要求往往不同。Jotiva 支持为不同文件夹单独设置图片规则，子目录自动继承最近一层上级目录的配置：

- **跟随文档（默认）**：粘贴截图或插入图片时，自动存进文档旁的相对子目录（如 `_assets` 或按文档名建子目录），移动文档时带着目录一起走即可。
- **固定共用目录**：适合博客或大型工程，把所有图片集中存到指定目录（如 `../assets`），自动生成相对路径引用。
- **HTTP 图床上传**：粘贴图片时自动上传到你的图床或自定义接口，直接插入返回的在线链接。
- **自动体积优化**：本地存图支持同名自动编号，并可开启静态 PNG / JPEG 自动无损尺寸压缩（仅在体积更小时采用，动图保持原样）。

---

## 主题定制与个人排版解耦

换主题时，最怕把调好的字号和行距冲掉。Jotiva 把「主题配色」与「个人排版习惯」分开保存：

- **可分享的 `.css` 主题**：内置**默认**、**墨与钢**、**Material** 与**蓝晒**四套明暗主题，也支持导入单文件 `.css` 主题；同名重新导入即可热更新，所有打开的窗口与终端即时换色。
- **排版习惯跨主题保留**：基础字号、正文字体、H1–H6 标题比例、横线稿纸、强调色以及「全局样式微调（Custom CSS）」独立保存，怎么换主题都不影响你的阅读习惯。

---

## 平台支持与快速开始

| 平台 | 支持版本与窗口挂载说明 |
| --- | --- |
| **macOS** | macOS 14+（Universal 通用版，支持 Apple 芯片与 Intel）。在「设置 → 窗口关联」授权辅助功能权限后即可挂载便签。 |
| **Windows** | Windows x64（需 Microsoft Edge WebView2）。支持从列表选择目标窗口，或直接把文档拖到目标窗口上挂载。 |
| **Linux** | Linux x86_64。在兼容 EWMH 的 X11 会话中支持窗口挂载；原生 Wayland 会话可使用独立便签与主窗口。 |

1. 前往 [GitHub Releases](https://github.com/lin037/jotiva/releases) 下载对应系统的安装包（功能以下载版本为准）。
2. [预览示例笔记 HelloJotiva.md](docs/HelloJotiva.md)。在 Jotiva 打开源码包里的 `docs/HelloJotiva.md`，可以直接改表格、勾待办，试试公式、Mermaid 和本地配图。
3. 查阅详细文档：[使用手册](docs/user-guide.md) · [快捷键参考](docs/shortcuts.md) · [外观与主题](docs/themes.md)

## 参与贡献与许可证

如果你想从源码构建、了解技术架构或参与贡献，欢迎阅读：
- [开发者指南](docs/developer-guide.md) · [架构与设计说明](docs/architecture.md) · [测试与发布验证](docs/verification.md) · [贡献指南](CONTRIBUTING.md)
- 如果 Jotiva 对你的日常记录或写作有帮助，欢迎点亮右上角的 **Star**，或到 [GitHub Discussions](https://github.com/lin037/jotiva/discussions) 聊聊你的用法。

Jotiva 基于 [MIT 许可证](LICENSE) 开源。
