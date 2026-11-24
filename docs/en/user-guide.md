# Jotiva user guide

[Project home](../../README.en.md) · [中文](../user-guide.md)

Say you're comparing two routes and want to remember where to turn. Open a quick note beside the map and jot it down. When you switch away, the note goes out of view. Come back to the map and it's there again. You can also use Jotiva to edit the Markdown and UTF-8 text files you already keep on your computer.

## Install Jotiva

Download the package for your computer. [GitHub Releases](https://github.com/lin037/jotiva/releases)

| Platform | Installation |
| --- | --- |
| macOS 14 or later | The universal build works on Apple silicon and Intel. Open the DMG and drag Jotiva into Applications, run the PKG installer, or unzip the app and move it into Applications. |
| Windows x64 | Run the file ending in `-setup.exe` and follow the prompts. Jotiva needs Microsoft Edge WebView2. |
| Linux x86_64 | On Ubuntu 24.04, run `sudo apt install ./filename.deb`. If you downloaded the AppImage, run `chmod +x ./filename.AppImage` from its folder, then open it. |

The Mac package is not notarized, and the Windows installer is unsigned. You may see a warning when you open them.

## Take a note beside another app

1. Click the window you want the note to follow, then press **Control+Shift+Space**.
2. Write your note. Press **Esc** to fold it down to a short preview. You can also leave it open and pass the keyboard back to the other app with **Cmd+Enter** on Mac or **Ctrl+Enter** on Windows and Linux.
3. Switch to a different window, then back to the original one. The note comes back with its window. If you close the original window, look for the note under 「待重新挂载」 or open search with **Cmd+K** or **Ctrl+K**.

If you already have a document open, right-click it in the sidebar and choose 「挂载到窗口…」, then switch to the window you want to attach it to. You can pin the note if you want it to stay visible while you work in other apps.

- **macOS** needs Accessibility permission to identify other apps' windows.
- **Windows** lets you pick a window or drag a document onto it.
- **Linux** supports attachment in EWMH-compatible X11 sessions. Standalone notes work on native Wayland. Global shortcuts depend on your desktop session.

[Watch a note follow its window](../../README.en.md#see-it-in-action)

## Open a file and start writing

Use **Cmd+O** on Mac or **Ctrl+O** on Windows and Linux. Start with a file you already have, or download the sample note and try checking off a task or editing the table. [HelloJotiva.md](../HelloJotiva.md)

In Live Preview, you can edit the formatted document directly. Use **Cmd+/** or **Ctrl+/** to switch to Markdown source and back. If you stop typing for a moment, Jotiva saves to the original file. Use **Cmd+S** or **Ctrl+S** to save right away. The bar at the bottom shows the path and save status.

Add folders you use often to the workspace sidebar. Keep related files open in tabs, or open two files in separate windows to compare them. For a long document, click a heading in the outline to jump straight to it.

## Find your notes

New quick notes appear under **Archive → Other** (「归档 → 其他」). If another app changes the same file, choose 「另存编辑并重载」 to save your edits separately in Other and reload the original.

| Platform | Where quick notes are saved |
| --- | --- |
| macOS | `~/Library/Application Support/io.github.lin037.jot.desktop/notes/其他/` |
| Windows | `%APPDATA%\io.github.lin037.jot.desktop\notes\其他\` |
| Linux | `~/.local/share/io.github.lin037.jot.desktop/notes/其他/`. If you set `XDG_DATA_HOME`, use that in place of `~/.local/share`. |

Jev filing is off by default. If you turn it on and ask for a suggestion, Jotiva sends up to the first 6,000 characters of the note, along with the candidate folders' names and descriptions, to the service you set up. The file moves when you confirm. Your system's credential store holds the API key. On Linux, this uses Secret Service, `secret-tool`, and `zenity`.

## Change the theme or shortcuts

Press **Cmd+,** or **Ctrl+,** to open Settings. In Appearance (「外观」), choose a theme, set the font size, or import CSS. In Shortcuts (「快捷键」), find an action by name or key combination and set the keys you want. [Shortcut reference (中文)](../shortcuts.md) · [Theme options (中文)](../themes.md)
