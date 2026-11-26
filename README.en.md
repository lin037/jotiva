<p align="center">
  <img src="src/assets/jot.svg" alt="Jotiva" width="76" height="76">
</p>
<h1 align="center">Jotiva</h1>
<p align="center"><strong>A Markdown scratchpad beside your work window. Jot down an idea, then let a model help you file it.</strong></p>
<p align="center">A free, open-source desktop Markdown note-taking and writing app for macOS, Windows, and Linux.</p>
<p align="center">
  <a href="https://github.com/lin037/jotiva/releases"><strong>Download</strong></a> ·
  <a href="docs/en/HelloJotiva.md">Sample Note</a> ·
  <a href="docs/en/user-guide.md">User Guide</a> ·
  <a href="docs/en/shortcuts.md">Shortcuts</a> ·
  <a href="docs/en/themes.md">Themes</a> ·
  <a href="README.md">简体中文</a>
</p>
<p align="center">
  <a href="docs/en/user-guide.md#installation"><img src="https://img.shields.io/badge/macOS-14%2B-343940?logo=apple&amp;logoColor=white" alt="macOS 14+"></a>
  <a href="docs/en/user-guide.md#installation"><img src="https://img.shields.io/badge/Windows-x64-2B579A" alt="Windows x64"></a>
  <a href="docs/en/user-guide.md#installation"><img src="https://img.shields.io/badge/Linux-x86__64-83561B?logo=linux&amp;logoColor=white" alt="Linux x86_64"></a>
  <a href="https://github.com/lin037/jotiva/actions/workflows/ci.yml"><img src="https://github.com/lin037/jotiva/actions/workflows/ci.yml/badge.svg?branch=main" alt="CI"></a>
  <a href="https://v2.tauri.app/"><img src="https://img.shields.io/badge/Tauri-2-14788A?logo=tauri&amp;logoColor=white" alt="Tauri 2"></a>
  <a href="https://rust-lang.org/"><img src="https://img.shields.io/badge/Rust-B5411A?logo=rust&amp;logoColor=white" alt="Rust"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-426c5a" alt="MIT License"></a>
</p>

<p align="center">
  <img src="docs/assets/screenshots/macos-themes.webp" alt="Jotiva on macOS showing Default Light, Ink and Steel Light, and Blueprint Dark alongside a quick note" width="960">
</p>

## Why Jotiva: More Than Just an Editor

Standard Markdown editors only care about what happens *after* you create a file. Real work starts earlier: you are debugging in a browser or reading API docs and need to jot down a few lines right there; quick notes pile up because you do not want to stop and organize folders mid-thought; reaching for the mouse breaks your typing flow; and pasting screenshots into different projects always requires fixing image paths by hand.

Jotiva is built around five core capabilities that solve these exact friction points:

| Core Pillar | How It Works in Jotiva | Problem Solved |
| --- | --- | --- |
| **1. Window-Attached Quick Notes** | Press `Control+Shift+Space` to pin a note beside your active window. It hides when you switch away, reappears when you return, and lets you jump back to that window anytime. | No constant `Alt+Tab` / `Cmd+Tab` switching, and no always-on-top windows blocking unrelated apps. |
| **2. Jev Smart Filing** | Quick notes save to disk immediately. When ready, click **File with Jev** to get folder suggestions based on your note's content and your folder descriptions. | Zero organizational overhead while capturing thoughts; fast, structured filing when you finish. |
| **3. Mouse-Free Workflow** | Summon notes, return focus to the host app, search, switch UI regions (`F6`), edit tables, and open the integrated terminal—all from the keyboard. | Keep your hands on the keyboard without breaking focus while coding, reading, or writing. |
| **4. Instant Live Rendering** | Powered by an incremental Rust/Wasm engine. Syntax markers appear only at the cursor and fold away elsewhere; tables, LaTeX math, and Mermaid diagrams render in place. | No split-pane clutter. Long documents open fast, typing stays responsive, and you edit what you see. |
| **5. Per-Folder Image Management** | Set image storage rules per directory (subfolder beside the doc, shared assets folder, or HTTP upload) with automatic subfolder inheritance and optional compression. | Lab notes, blogs, and project docs each follow their own image workflow automatically on paste. |

---

## Five Core Capabilities in Detail

### 1. Quick Notes: Attached to Your Active Window

Say you have two browser windows open—one for an error report, another for API documentation. Press `Control+Shift+Space` in each window to attach a dedicated note beside it:

- **Follows the window, stays out of the way**: Different windows of the same app can hold separate notes, and one window can hold several. Unpinned notes hide when you leave the window and reappear when you come back. Press `Esc` to collapse a note into a summary bar, click the pin icon to keep it above all windows, or attach any existing sidebar document beside a reference window.
- **Jump from a note back to its window**: Press `Cmd+K` / `Ctrl+K` to search across **Current window**, **Notes in other windows**, and **Documents**. Selecting an attached note brings you straight back to its host window. If that window was closed, the note waits safely under **Awaiting reattachment**.
- **Always a real local `.md` file**: Choose **Edit in main window** from the note menu to continue expanding those quick lines in the main workspace—editing the exact same Markdown file on disk.

### 2. Jev Filing & Local Workspace: Capture Without Friction, File with Order

Jotiva edits standard `.md`, `.markdown`, and UTF-8 `.txt` files directly on disk, separating **Workspace** (active project folders) from **Archive** (incoming quick notes and filed references):

- **Write first, file with Jev**: New quick notes land in **Archive → Inbox** so you never have to pick a folder before typing. With optional **Jev filing** enabled (off by default), just add a short description to your archive folders. Clicking **File with Jev** sends an excerpt and candidate folder descriptions to suggest the best destination, moving the file only after you confirm.
- **Zero-lock-in project folders**: Add codebases, blogs, or note directories to the Workspace sidebar to create, rename, edit, or drag-move files in place. Removing a workspace folder only removes its sidebar shortcut—never your files.
- **Multi-window & conflict-safe saving**: Open multiple standalone document windows side by side. Non-overlapping edits across windows merge automatically; if an external change or conflict occurs, **Keep a copy and reload** protects your unsaved work.

### 3. Mouse-Free Workflow: Full Keyboard Control

Every step from jotting a thought to running build scripts is reachable without touching the mouse:

- **Capture and return focus in one keystroke**: Press `Control+Shift+Space` to jot a note, then press `Cmd+Enter` (macOS) or `Ctrl+Enter` (Windows / Linux) to hand keyboard focus right back to the host window.
- **Region & table navigation**: Search notes and files with `Cmd+K` / `Ctrl+K`; jump between the editor, file tree, archive, and outline with `F6` (or `Cmd+1` / `2` / `3`); navigate and insert/delete table rows and columns directly via keyboard shortcuts.
- **Integrated terminal at the active document's path**: Press `` Ctrl+Shift+` `` to open a terminal in the current file's folder for blog previews or Git commands. Sessions stay alive across tab switches, and scripts can use `JOTIVA_INBOX_DIR` to drop generated `.md` files straight into your Inbox.
- **Command palette & customizable bindings**: Run editor actions via `Ctrl+Shift+P` and rebind common shortcuts under **Settings → Shortcuts**.

### 4. Instant Live Rendering: In-Place Editing at Rust/Wasm Speed

Quick notes, workspace tabs, and standalone document windows share the same editor core: CodeMirror handles typing, IME input, and undo history, while Jotiva's Rust/Wasm engine parses Markdown incrementally in a background worker.

- **Distraction-free Live Preview**: Markdown syntax reveals only inside the active block around your cursor and hides when you move away. Click directly into table cells to edit or tick checkboxes to update the file. Toggle full Markdown source anytime with `Cmd+/` or `Ctrl+/`.
- **Complete technical writing toolkit**: Write inline and block LaTeX math, code blocks (wrapped or horizontally scrollable), theme-aware Mermaid diagrams (flowcharts, sequence diagrams, Gantt charts), collapsible headings, and a live document outline in one `.md` file.
- **Viewport-first speed & state preservation**: Keystrokes appear immediately while the background worker parses affected text first. Switching tabs preserves your exact scroll position, selection, and undo history.

### 5. Image Management: Per-Folder Rules on Paste

Different folders need different asset layouts. Configure image behavior globally or per folder, and subfolders automatically inherit the nearest parent folder's rule:

- **Beside the document (default)**: Save pasted screenshots into a relative subfolder beside the note (e.g., `_assets` or a per-document subfolder) so notes and images stay portable together.
- **Shared directory**: Store all images for a blog or project in a central folder (such as `../assets`) using relative paths.
- **HTTP upload**: Automatically upload pasted images to your image host or gateway and insert the returned URL.
- **Automatic size optimization**: Automatically numbers duplicate filenames and optionally compresses static PNG/JPEG images without resizing (keeping the result only when smaller, leaving animations untouched).

---

## Shareable Themes & Independent Typography

Jotiva separates **color themes** from **personal typography settings** so switching themes never ruins your layout:

- **Single-file CSS themes**: Switch between **Default**, **Ink and Steel**, **Material**, and **Blueprint** (light and dark modes), or import any `.css` theme file with live updates across open windows and terminals.
- **Persistent reading & writing habits**: Base font size, writing font, H1–H6 heading scales, ruled paper, accent color, and **Global CSS tweaks** stay intact across any theme change.

---

## Platform Support & Getting Started

| Platform | Requirements & Window Attachment |
| --- | --- |
| **macOS** | macOS 14+ (Universal for Apple silicon & Intel). Grant Accessibility permission under **Settings → Window attachments** to attach notes to other windows. |
| **Windows** | Windows x64 (requires Microsoft Edge WebView2). Pick a target window from the list or drag a document onto it. |
| **Linux** | Linux x86_64. Window attachment works on EWMH-compatible X11 sessions; native Wayland supports standalone notes and main windows. |

1. Download the installer for your platform from [GitHub Releases](https://github.com/lin037/jotiva/releases).
2. [Preview the HelloJotiva sample](docs/en/HelloJotiva.md). Open `docs/en/HelloJotiva.md` from the source archive in Jotiva to edit the table, check off tasks, and try formulas, Mermaid, and local images.
3. Explore the docs: [User Guide](docs/en/user-guide.md) · [Shortcuts](docs/en/shortcuts.md) · [Themes](docs/en/themes.md)

## Contributing & License

Want to build from source, explore the internals, or contribute?
- [Developer Guide](docs/en/developer-guide.md) · [Architecture](docs/en/architecture.md) · [Verification & Releasing](docs/en/verification.md) · [Contributing](docs/en/contributing.md)
- If Jotiva helps your workflow, consider giving it a **Star** or sharing ideas in [GitHub Discussions](https://github.com/lin037/jotiva/discussions).

Jotiva is open-source software licensed under the [MIT License](LICENSE).
