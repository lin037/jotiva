# Jotiva user guide

[Project home](../../README.md) · [中文](../user-guide.md)

Jotiva is an open-source desktop editor for Markdown and UTF-8 text. Open files where they already live, browse local folders, or keep a note beside the window you're working in. Your documents remain plain files you can open with another editor.

## Install

Choose an installer for your system from [GitHub Releases](https://github.com/lin037/jotiva/releases).

| System | How to install |
| --- | --- |
| macOS 14 or later | The universal packages work on Apple silicon and Intel Macs. Drag Jotiva to Applications from the DMG, run the PKG installer, or unzip the app and move it to Applications. |
| Windows x64 | Run the `-setup.exe` installer. Jotiva uses Microsoft Edge WebView2. |
| Linux x86_64 | On Ubuntu 24.04, install the Debian package with `sudo apt install ./filename.deb`. For the AppImage, run `chmod +x ./filename.AppImage` and open it. |

The macOS package is not notarized, and the Windows installer is unsigned. Your system may ask you to verify the publisher.

## Open, edit, and save

1. Download [HelloJotiva.md](../HelloJotiva.md) to a folder on your computer. Press `Cmd+O` on Mac or `Ctrl+O` on Windows and Linux to open it.
2. Check off a task or edit a table cell in Live Preview. Use `Cmd+/` or `Ctrl+/` to switch to Markdown source and back.
3. Changes save automatically after a pause. Press `Cmd+S` or `Ctrl+S` to save immediately. The status bar shows the file path and save status.
4. Add local folders to the workspace, use tabs to switch between files, or open documents in separate windows to compare them.

## Keep a note beside a window

Focus a browser, meeting, or other app and press `Control+Shift+Space`. Write your note, then use `Cmd+Enter` on Mac or `Ctrl+Enter` on Windows and Linux to return focus to the attached window. Press Esc to collapse the note. You can also attach an existing document from its context menu.

- **macOS** asks for Accessibility permission before it can identify other apps' windows.
- **Windows** supports choosing another window or dragging a document to it.
- **Linux** supports attachment in compatible EWMH X11 sessions. Standalone notes work on native Wayland. Global shortcuts depend on the desktop session.

If the target window closes, find the note under **Pending reattachment** (「待重新挂载」) or open search with `Cmd+K` or `Ctrl+K`.

## Your files and settings

Jotiva saves existing documents back to their original location. New quick notes appear under **Archive → Other**. If another app changes the same file, use **Save edits separately and reload** to keep your edits in Other and reopen the original.

| System | Default quick-note location |
| --- | --- |
| macOS | `~/Library/Application Support/io.github.lin037.jot.desktop/notes/其他/` |
| Windows | `%APPDATA%\io.github.lin037.jot.desktop\notes\其他\` |
| Linux | `~/.local/share/io.github.lin037.jot.desktop/notes/其他/`, or the corresponding path under `XDG_DATA_HOME`. |

Optional Jev filing can suggest an archive folder. It sends a note excerpt and candidate folder descriptions to your configured service when you ask for a suggestion. The note moves after you confirm. API keys stay in your operating system's credential store. Linux uses Secret Service, `secret-tool`, and `zenity`.

Open Settings with `Cmd+,` or `Ctrl+,`. Use **Appearance** (「外观」) to change the theme or import CSS and **Shortcuts** (「快捷键」) to remap keys. [Three-platform shortcut reference (中文)](../shortcuts.md) · [Theme customization (中文)](../themes.md)
