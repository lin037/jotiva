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

[Watch a note follow its window](../../README.en.md#demos)

## Insert and store images

Paste a screenshot or use the image button to choose a file or enter an existing URL. **Settings → Images** controls where pasted and picked files go.

On macOS, if clicking the local file option closes the menu, reopen it, focus that option with the keyboard and press Enter to open the file chooser.

- **Beside the document** stores files in `_assets` by default. Move that directory along with the Markdown file to keep relative links working. Unsaved notes use the inbox directory.
- **Shared directory** writes a local file URL. Either local mode can group images by document name. Duplicate filenames get a numeric suffix.
- **HTTP upload** sends a multipart POST. Configure the form field and the JSON Pointer for the returned HTTP or HTTPS image URL. For example, use `file` and `/data/url` for `{"data":{"url":"https://cdn.example.com/photo.png"}}`. An optional bearer token is stored in the system credential store for that endpoint. Object storage can be reached through an upload gateway.

Right-click a workspace or archive folder and choose **Image settings…** to see the current method. To set up a folder rule, choose **Folder rules**, then copy a saved rule or customize a new one, and click **Apply**. For example, use **Beside the document** with `images` for blog posts and `media/screenshots` for meeting notes. Subfolders use the closest matching rule. Folders without a rule use the global default. Choose **Restore inheritance** to remove the rule for the selected folder.

To share an image directory across a blog, choose **Folder rules → Customize now** on the blog root. Set the method to **Shared directory**, choose **Relative to this folder**, and enter `../access`. Images from `blog/posts/entry.md` go into `access` next to `blog`, and the document links to `../../access/photo.png`. An absolute shared directory uses local file URLs.

The limit is 32 MiB per file. Optional compression keeps still PNG and JPEG images at their original dimensions and uses the result only when it is smaller. Animated images are preserved.

## Open a file and start writing

Use **Cmd+O** on Mac or **Ctrl+O** on Windows and Linux. Start with a file you already have, or download the sample note and try checking off a task or editing the table. [HelloJotiva.md](../HelloJotiva.md)

In Live Preview, you can edit the formatted document directly. Use **Cmd+/** or **Ctrl+/** to switch to Markdown source and back. If you stop typing for a moment, Jotiva saves to the original file. Use **Cmd+S** or **Ctrl+S** to save right away. The bar at the bottom shows the path and save status.

Add folders you use often to the workspace sidebar. Keep related files open in tabs, or open two files in separate windows to compare them. For a long document, click a heading in the outline to jump straight to it.

Creating a document saves an empty file immediately and selects its name in the file tree. Type a name and click elsewhere or press Enter. If that name exists, Jotiva adds a number before the extension.

Right-click a tab to copy its text, duplicate the document, open it in a new window, or close tabs to its left, right, or on both sides.

## Use the integrated terminal

While editing, press **Ctrl + Shift + Backquote** to return to the most recently used live terminal opened for that document. If it has none, a new shell starts in its directory. Choose **New Terminal Here** from the context menu on a workspace entry, archive entry, or document tab to pick the starting location. With no saved document, a new shell starts in the default archive directory.

The title bar terminal button hides the panel or brings back your last session. Use **+** to start another shell, the session list on the right to switch between shells, and the panel’s top edge to resize it. Switching or closing a document keeps its shell and running commands alive.

A session takes its name from the file or folder where it was opened. Numbers distinguish sessions with the same name. Right-click a session and choose **Rename terminal**, or focus its tab and press F2. Clear the name and save to restore the default.

Drag a file or a document tab into the terminal to insert its quoted path at the cursor. Press Enter when the command is ready. Scripts can read `JOTIVA_INBOX_DIR` to place generated or synced documents in the default archive. Use `$env:JOTIVA_INBOX_DIR` in PowerShell or `"$JOTIVA_INBOX_DIR"` in Bash or Zsh. **Terminate terminal** closes the selected session. If shells are running when you close the main window or quit the app, Jotiva asks for confirmation.

## Find your notes

Scroll or press **Down** to load more search results. Press **Up** to revisit earlier results.


New quick notes appear under **Archive → Inbox** (「归档 → 其他」). If another app changes the same file, choose 「另存编辑并重载」 to save your edits separately in Other and reload the original.

| Platform | Where quick notes are saved |
| --- | --- |
| macOS | `~/Library/Application Support/io.github.lin037.jot.desktop/notes/其他/` |
| Windows | `%APPDATA%\io.github.lin037.jot.desktop\notes\其他\` |
| Linux | `~/.local/share/io.github.lin037.jot.desktop/notes/其他/`. If you set `XDG_DATA_HOME`, use that in place of `~/.local/share`. |

Right-click an imported archive folder and choose **Archive folder settings…** to edit its display name, description, and participation in Jev filing. New suggestions use the saved details. Renaming the entry keeps its disk location.

Jev filing is off by default. If you turn it on and ask for a suggestion, Jotiva sends up to the first 6,000 characters of the note, along with the candidate folders' names and descriptions, to the service you set up. The file moves when you confirm. Your system's credential store holds the API key. On Linux, this uses Secret Service, `secret-tool`, and `zenity`.

## Change the theme or shortcuts

Choose **English** or **简体中文** under **Settings → Appearance → Interface language** to update open windows immediately. Press **Cmd+,** or **Ctrl+,** to open Settings. In Appearance (「界面外观」), choose a theme, set the font size, or edit the global CSS. In Shortcuts (「快捷键」), find an action by name or key combination and set the keys you want. [Shortcut reference (中文)](../shortcuts.md) · [Theme options (中文)](../themes.md)


Single-click a file in the sidebar to reuse the italic preview tab. Double-click the file or tab, choose **Keep tab** from the tab menu, or start editing to keep it. Disable **Preview files on single click** in **Settings → Editor** to keep every file. Each press of the global quick-note shortcut creates a fresh note, as does **More → New note** in a note window.

Choose a built-in or installed theme in **Settings → Appearance → Interface theme**. Under **My themes**, select **Import theme…** and choose a CSS file. Jotiva uses the filename as the theme name. When a name is already taken, you can update the installed theme or import a copy with a different name. Open documents and terminals update immediately. Type or paste CSS under **Global CSS tweaks** and choose **Save** to apply it across themes. Choose **Format** to tidy the CSS, or clear the text and save to remove it. [Theme guide (中文)](../themes.md)

## Software updates

Signed update-enabled builds check on startup and every six hours. Open **Settings → Software updates** to check manually or install a verified update. Jotiva saves open editors before restarting and asks before closing a running terminal. Install the first update from 0.1.1 manually. Debian installations update through the package manager.
