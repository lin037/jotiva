# Jotiva Shortcut Reference

[Project Home](../../README.en.md) · [User Guide](user-guide.md) · [中文](../shortcuts.md)

A **mouse-free workflow** is a core part of Jotiva. Below are all default shortcuts—you can search by action or key chord and customize bindings under **Settings → Shortcuts**. On macOS, `⌘`, `⌥`, `⌃`, and `⇧` stand for `Command`, `Option`, `Control`, and `Shift`. Windows and Linux share the same in-app defaults.

> [!TIP]
> **Typical Mouse-Free Workflow**
> 1. **Capture beside a window**: Press `Control+Shift+Space` beside your active window to jot a note → press `Cmd+Enter` / `Ctrl+Enter` to hand keyboard focus straight back to the host app.
> 2. **Jump back to context**: Press `Cmd+K` / `Ctrl+K` in Jotiva to search notes, press `Enter` to return to the host window, or choose **Edit in main window** to expand the draft.
> 3. **Navigate & edit**: Press `F6` (or `Cmd+1` / `2` / `3`) to jump between the editor, file tree, archive, and outline → `Cmd+/` / `Ctrl+/` to toggle Live Preview and full source → `Tab` / `Enter` and table shortcuts to edit tables → `` Ctrl+Shift+` `` to open a terminal in the active document's folder.

---

## 1. Windows & Quick Notes

| Action | macOS | Windows / Linux |
| --- | --- | --- |
| Open Quick Note (global) | `Control+Shift+Space` | `Ctrl+Shift+Space` |
| Find notes and documents (and jump to host window) | `⌘K` | `Ctrl+K` |
| Return focus to host window | `⌘Enter` | `Ctrl+Enter` |
| Return to main window | `` ⌘` `` | `` Ctrl+` `` |
| Tile notes | `⇧⌘O` | `Ctrl+Shift+F8` |
| Toggle always on top | `⌥⌘P` | `Ctrl+Alt+P` |
| Tile beside main window | `⌘\` | `Ctrl+\` |
| Open current document in new window | `⌥⌘O` | `Ctrl+Alt+O` |
| Collapse note or close search | `Esc` | `Esc` |

> On Linux, global shortcuts, window attachment, and side-by-side tiling depend on your desktop session (EWMH-compatible X11 sessions support attachment; native Wayland supports standalone notes).

---

## 2. Files, Workspace & Integrated Terminal

| Action | macOS | Windows / Linux |
| --- | --- | --- |
| New document / New Document Window | `⌘N` / `⇧⌘N` | `Ctrl+N` / `Ctrl+Shift+N` |
| Open local document | `⌘O` | `Ctrl+O` |
| Save immediately | `⌘S` | `Ctrl+S` |
| Close tab or window | `⌘W` | `Ctrl+W` |
| Switch to previous document | `⌃Tab` | `Ctrl+Tab` |
| Focus directly: Editor / Workspace / Archive | `⌘1` / `⌘2` / `⌘3` | `Ctrl+1` / `Ctrl+2` / `Ctrl+3` |
| Cycle focus across regions (editor / tree / outline) | `F6` / `⇧F6` | `F6` / `Shift+F6` |
| Choose focus region from list | `⌥F6` | `Alt+F6` |
| Toggle left sidebar | `⇧⌘L` | `Ctrl+Alt+L` |
| Open terminal at current document's path | `` ⌃⇧` `` | `` Ctrl+Shift+` `` |
| Open settings / Quit Jotiva | `⌘,` / `⌘Q` | `Ctrl+,` / `Ctrl+Q` |

**Sidebar file tree keys**: Use arrow keys to move and expand/collapse folders, `Enter` to open a file, `F2` to rename inline, and `⌘Backspace` (macOS) or `Delete` (Windows / Linux) to move items to the Trash / Recycle Bin.

---

## 3. Live Rendering, Navigation & Table Editing

| Action | macOS | Windows / Linux |
| --- | --- | --- |
| Toggle Live Preview and full Markdown source | `⌘/` | `Ctrl+/` |
| Open editor command palette | `⌃⇧P` | `Ctrl+Shift+P` |
| Find in document / Find and replace | `⌘F` / `⌃⇧H` | `Ctrl+F` / `Ctrl+Shift+H` |
| Next / previous match | `⌃G` / `⌃⇧G` | `Ctrl+G` / `Ctrl+Shift+G` |
| Go to line | `⌃⇧L` | `Ctrl+Shift+L` |
| Toggle current heading fold | `⌃⇧O` | `Ctrl+Shift+O` |
| Bold / Italic | `⌘B` / `⌘I` | `Ctrl+B` / `Ctrl+I` |
| Inline code / Insert or edit link | `⌃⇧C` / `⌃⇧K` | `Ctrl+Shift+C` / `Ctrl+Shift+K` |
| Heading 1–6 | `⌃⇧1` – `⌃⇧6` | `Ctrl+Shift+1` – `Ctrl+Shift+6` |
| Toggle task list / Toggle task completion | `⌃⇧9` / `⌘⌥Enter` | `Ctrl+Shift+9` / `Ctrl+Alt+Enter` |
| Undo / Redo | `⌘Z` / `⇧⌘Z` | `Ctrl+Z` / `Ctrl+Y` |
| Line start/end / Document start/end | `⌘←` `⌘→` / `⌘↑` `⌘↓` | `Home` `End` / `Ctrl+Home` `Ctrl+End` |

### Keyboard Table Editing

| Action | macOS | Windows / Linux |
| --- | --- | --- |
| Next / previous cell | `Tab` / `⇧Tab` | `Tab` / `Shift+Tab` |
| Cell below / above | `Enter` / `⇧Enter` (or `⌥↓` / `⌥↑`) | `Enter` / `Shift+Enter` (or `Alt+↓` / `Alt+↑`) |
| Insert row below / column to the right | `⌃⇧↓` / `⌃⇧→` | `Ctrl+Shift+↓` / `Ctrl+Shift+→` |
| Delete table row / Delete table column | `⌘⌥⌫` / `⌘⌥⇧⌫` | `Ctrl+Alt+Backspace` / `Ctrl+Alt+Shift+Backspace` |
| Clear table cell | `⌥⇧⌫` | `Alt+Shift+Backspace` |

---

## 4. Customizing Shortcuts

In **Settings → Shortcuts**, filter by action name or key combination. Click a configurable shortcut, press the new key chord, and click **Save shortcuts** (press `Esc` to cancel recording). Actions marked **Unassigned** (such as **Renumber ordered list**, **Expand all sections**, or global **Find notes and documents** and **Dock right or restore**) can also be assigned keys here.
