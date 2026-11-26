# Jotiva User Guide

[Project Home](../../README.en.md) · [Shortcuts](shortcuts.md) · [Themes](themes.md) · [简体中文](../user-guide.md)

Jotiva edits local Markdown (`.md`, `.markdown`) and UTF-8 plain text (`.txt`) files directly on your disk. It is built around five core capabilities for everyday technical and note-taking workflows:
- **Window-Attached Quick Notes**: Pin notes right beside the app window you are working in—they hide when you switch away and reappear when you return;
- **Jev Smart Filing**: Capture thoughts immediately without picking a folder, then get content-based folder suggestions when you are ready to organize;
- **Mouse-Free Workflow**: Summon notes, return focus to the host app, search, switch UI regions, and edit tables entirely from the keyboard;
- **Instant Live Rendering**: Syntax markers reveal only at the cursor while tables, LaTeX math, and Mermaid diagrams render and edit in place;
- **Per-Folder Image Management**: Configure image storage rules per directory (subfolder beside the doc, shared folder, or HTTP upload) with automatic subfolder inheritance.

Every note is a plain file on your disk, ready for Git or any other editor.

---

## Installation

Download the installer for your platform from [GitHub Releases](https://github.com/lin037/jotiva/releases):

| Platform | How to install |
| --- | --- |
| **macOS 14+** | The `universal` build runs on Apple silicon and Intel. Open the `.dmg` and drag Jotiva into Applications, run the `.pkg` installer, or unzip and move the app into Applications. |
| **Windows x64** | Run the `-setup.exe` installer and follow the prompts (requires Microsoft Edge WebView2). |
| **Linux x86_64** | On Debian / Ubuntu, run `sudo apt install ./filename.deb`. For AppImage, run `chmod +x ./filename.AppImage` and launch it. |

> [!NOTE]
> If a release package is unsigned or unnotarized, your operating system may show a security prompt the first time you open it. This guide follows the current source. Features depend on the installer version.

---

## 1. Quick Notes Beside Other Windows

Whenever you are reading documentation, attending a meeting, or debugging in another app, you can attach a note directly to that window without constant window switching:

1. **Attach to the active window**: Click the reference window you want the note to follow, then press **`Control+Shift+Space`** to open a new quick note. Different windows of the same app can keep separate notes, and one window can hold multiple notes.
2. **Return focus without the mouse**: When you finish jotting something down in an attached note, press **`Cmd+Enter`** (macOS) or **`Ctrl+Enter`** (Windows / Linux) to hand keyboard focus straight back to the host window.
3. **Auto-hide, collapse, or pin**: Press **`Esc`** to collapse a note into a compact summary bar, or click the pin icon to **Keep above other windows**. When you switch to another app, unpinned attached notes hide automatically and reappear when you return.

### Attach an existing file or expand in the main window
- **Attach an existing document**: Right-click any file in the sidebar and choose **Attach to window…**, then switch to the target window or click **Choose from list…**. On Windows, you can also drag a document onto a window.
- **Continue in the main workspace**: Click **Edit in main window** inside a quick note to open the exact same local file in the main workspace with the outline, sidebar, and terminal.

### Jump back to the original window from your notes
In Jotiva, press **`Cmd+K`** (macOS) or **`Ctrl+K`** to open search:
- Results are grouped into **Current window**, **Notes in other windows**, and **Documents** (or **Attached notes** when searching from the main window). Selecting an attached note brings you straight back to its associated host window.
- Registered notes match by body content; unregistered files in the workspace or archive folders match by filename.
- If the original window has been closed, the note remains safe under **Awaiting reattachment** or via search, ready to be attached to a new window anytime.

<details>
<summary>Platform attachment details & macOS Accessibility troubleshooting</summary>

- **macOS**: Grant **Accessibility** permission under **Settings → Window attachments** so Jotiva can identify other apps' windows. If macOS asks for permission again after an update, request it there. If Accessibility is enabled in System Settings but Jotiva still asks for permission, quit Jotiva and reopen it from Applications. If the prompt persists, re-add the current app under **System Settings → Privacy & Security → Accessibility**.
- **Windows**: Choose a window from the list or drag a document onto the target window.
- **Linux**: Window attachment works in EWMH-compatible X11 sessions. Native Wayland supports standalone notes. Global shortcut availability depends on your desktop session.

</details>

---

## 2. Instant Live Rendering & Long-Form Writing

Press **`Cmd+O`** / **`Ctrl+O`** to open a local Markdown or UTF-8 `.txt` file, or start with [HelloJotiva.md](HelloJotiva.md).

### In-place Live Preview & full source toggle
- **Edit what you see**: In Live Preview, Markdown syntax markers appear only around your cursor in regular blocks and hide when you move away. Click directly into table cells to edit them, or tick checkboxes to update the file on disk.
- **Built for technical writing**: Combine inline and block LaTeX math, code blocks (with line wrapping or horizontal scrolling), theme-matched Mermaid diagrams (flowcharts, sequence diagrams, Gantt charts), tables, task lists, and images in a single `.md` file.
- **Instant source toggle**: Press **`Cmd+/`** / **`Ctrl+/`** anytime to switch to full Markdown source, and press again to return to Live Preview.

### Mouse-free navigation, tabs, and multiple windows
- **Keyboard region switching**: Press **`F6`** / **`Shift+F6`** (or `Cmd+1` editor, `Cmd+2` workspace, `Cmd+3` archive) to move focus between the editor, file tree, and outline. Navigate and edit tables directly with `Tab`, `Enter`, and table shortcuts (see [Shortcut Reference](shortcuts.md)).
- **Workspace sidebar**: Click **Add workspace folder…** to add codebases, blogs, or note directories. Creating a new document writes an empty file immediately and lets you type its name inline (adding a numeric suffix if the name is taken).
- **Outline & section folding**: Jump through long documents using the right-hand **Document outline**, or fold/unfold sections with `Control+Shift+O` / `Ctrl+Shift+O`.
- **Preview tabs & side-by-side windows**: Single-clicking a file in the sidebar opens it in an italic **preview tab** that gets reused as you browse. Start editing, double-click the file/tab, or choose **Keep tab** to keep it open (or disable **Preview files on single click** in **Settings → Editor**). Right-click a file or tab and select **Open in new window** to compare two documents side by side.

### Auto-save & conflict-safe merging
Jotiva auto-saves shortly after you pause typing, or immediately when you press **`Cmd+S`** / **`Ctrl+S`**. If the same file is edited across multiple windows, non-overlapping changes merge automatically. If an external change or overlap occurs, choose **Keep a copy and reload** to save your unsaved edits into **Archive → Inbox** before reloading the file from disk.

---

## 3. Jev Smart Filing & Archive Organization

Jotiva separates active project directories from incoming quick thoughts so you can write first and organize later:
- **Workspace**: Folders you actively maintain (codebases, blogs, vaults). Removing a folder from the workspace **only removes its sidebar entry—it never deletes files on disk**. Dragging a file to another folder moves it on disk.
- **Archive**: New quick notes start in **Archive → Inbox**. When you are ready to organize them, move them manually from the status bar path menu or context menu (**Move to archive folder…**), or let **Jev filing** suggest the right folder.

### Smart folder suggestions with Jev (optional)
Jev filing is **off by default**. Even when enabled, data is sent only when you explicitly click **File with Jev**:

1. In **Settings → Filing**, choose **Base URL** or **Endpoint URL**, and enter your service URL and model name (remote endpoints require HTTPS; local development endpoints can use `127.0.0.1` or `[::1]`).
2. Click **Set up API key…** to save the token in your system credential store (macOS Keychain, Windows Credential Manager, or Linux system keyring). Enable **Allow Jev filing** and save.
3. Right-click an imported archive folder in the sidebar, open **Archive folder settings…**, give it a clear display name and **Description** (for example, "Backend incident reports and SQL query debugging notes"), and check **Include in Jev filing**.
4. Open a saved note in Inbox (with at least 20 letters or digits) and click **File with Jev**. Jotiva trims leading/trailing whitespace and sends up to the first 6,000 characters along with eligible folder names and descriptions. When a folder matches, Jotiva moves the file **only after you confirm**.

<details>
<summary>Default Inbox folder paths on disk</summary>

Use the status bar or context menu to reveal an Inbox note in your file manager, or browse directly:
- **macOS**: `~/Library/Application Support/io.github.lin037.jot.desktop/notes/其他/`
- **Windows**: `%APPDATA%\io.github.lin037.jot.desktop\notes\其他\`
- **Linux**: `~/.local/share/io.github.lin037.jot.desktop/notes/其他/` (or replace `~/.local/share` with `$XDG_DATA_HOME`)

</details>

---

## 4. Per-Folder Image Management

When you paste a screenshot or insert an image from the toolbar, Jotiva stores it according to **Settings → Images** or the folder's **Image settings…** context menu:

| Storage method | Best for | Example setup |
| --- | --- | --- |
| **Beside the document** (default) | Keeping images portable alongside each Markdown file (default `_assets` subfolder). Move or copy the images along with the document. | Use `images` for lab notes and `media/screenshots` for meeting notes. |
| **Shared directory** | Sharing one asset folder across an entire project or blog, optionally using **Relative to this folder**. | On a blog root, open **Image settings…**, select **Shared directory → Relative to this folder**, and enter `../assets` to generate relative links across posts. |
| **HTTP upload** | Uploading pasted images to your image host or upload gateway and inserting the returned URL directly. | Ideal for published blog posts or shared documents. |

- **Automatic folder inheritance**: Subfolders inherit the nearest parent folder rule, falling back to the global default. In the folder's **Image settings…**, choose **Folder rules → Restore inheritance** and apply it to remove a folder-specific override.
- **Deduplication & size compression**: Local modes can group images into subfolders by document name and append a suffix to duplicate filenames. Optional **Automatically compress still images** optimizes static PNG/JPEG files without resizing when smaller, while preserving animated images (32 MiB limit per file).

<details>
<summary>HTTP upload configuration & macOS local picker tip</summary>

- **HTTP upload fields**: The endpoint must accept `POST multipart/form-data` and return JSON. For example, if the form field is `file` and the response is `{"data":{"url":"https://cdn.example.com/photo.png"}}`, set **Form file field** to `file` and **Image URL JSON Pointer** to `/data/url`. If authentication is required, enable **Use a bearer token** and set up the token in your system credential store.

</details>

---

## 5. Integrated Terminal

Run blog previews, link checks, or Git commands right from the active document's folder without leaving the keyboard:
- **Open at the active file's path**: Press **`` Ctrl+Shift+` ``** in the main window to return to a terminal session for the current location or start a new one. You can also right-click a folder or tab and choose **New terminal here**.
- **Persistent sessions**: Switching or closing documents keeps running shells alive. Click **`+`** to start additional sessions, switch sessions from the right-hand list, or rename them via context menu / `F2`.
- **Drag paths & `JOTIVA_INBOX_DIR`**: Drag a file or document tab into the terminal to insert its escaped path. Scripts can write generated `.md` files into `JOTIVA_INBOX_DIR` (`"$JOTIVA_INBOX_DIR"` in Bash/Zsh, `$env:JOTIVA_INBOX_DIR` in PowerShell) to place them straight in **Archive → Inbox**.

---

## 6. Appearance, Language, Shortcuts & Updates

- **Interface language**: Press **`Cmd+,`** / **`Ctrl+,`** to open Settings, then switch between **English** and **简体中文** under **Settings → Appearance → Interface language**. Open windows update immediately.
- **Themes & personal typography**: Color themes are separated from your personal font size, writing font, heading scales, ruled paper, and global CSS tweaks. See the [Themes Guide](themes.md).
- **Shortcuts**: Customize supported shortcuts under **Settings → Shortcuts**, or browse defaults in the [Shortcut Reference](shortcuts.md).
- **Software updates**: Update-enabled builds check automatically at startup and every six hours, or manually under **Settings → Software updates** (for Debian `.deb` packages, download and install the new version). Jotiva saves open editors before restarting and prompts if terminal sessions are active.
