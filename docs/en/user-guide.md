# Jotiva User Guide

[Project home](../../README.md) · [简体中文](../user-guide.md)

Jotiva is a Markdown editor for macOS. Open a file and write where it already lives, or keep a note beside a meeting, browser, or coding window. Your notes are regular Markdown files you can open in another editor.

## Get started

Jotiva requires macOS 14 or later. The `universal` downloads work on Apple silicon and Intel Macs. Downloads marked `aarch64` are for Apple silicon. Choose one of the following options. [Download Jotiva](https://github.com/lin037/jotiva/releases/latest)

- **Disk image (.dmg)**. Open the image, drag Jotiva along the arrow into Applications and eject the image once the copy finishes.
- **Installer package (.pkg)**. Double-click the package and choose whether to install for yourself or everyone on the Mac. A personal installation puts Jotiva in the Applications folder inside your home directory.
- **ZIP archive (.zip)**. Extract the archive and move `Jotiva.app` into Applications.

Open Jotiva from Applications. Use `⌘O` to open Markdown, or choose Jotiva from **Open With** in Finder. Jotiva saves changes to the original file after you pause typing. Press `⌘S` to save immediately.

Press `⌘/` to switch between editable live preview and Markdown source. Drop several Markdown files onto the main window to open them in tabs. Press `⌃Tab` to return to the last document, or `⌥⌘O` to open the current document in a separate window. Use the outline to jump between headings, `⌘F` to search the current document, and `⌘K` to find notes and files.

## Take a note beside another app

1. Focus the window you want the note to follow and press `Control+Shift+Space`. Jotiva uses macOS Accessibility permission to identify windows and respond when you switch between them. You can grant access in advance from **Settings → Window Association** (`设置 → 窗口关联`).
2. Write, then press `⌘Return` to give the other app focus again. Press `Esc` to collapse your note. Pin it to keep it visible when you switch apps.
3. When the other app closes, find the note in the detached section (`待重新挂载`) or with `⌘K`. Closing the note itself saves it and removes its association.

You can also right-click an existing Markdown file and choose `挂载到窗口` to attach it. If Jotiva needs permission, click `打开辅助功能设置`, enable Jotiva in macOS Accessibility settings, then return to Jotiva to pick a window. The file stays at its original path. If the default shortcut is in use, change it in Jotiva’s shortcut settings. Regular Markdown editing works with Accessibility turned off.

**Jotiva still asks for permission after an update.** Quit with `⌘Q` and reopen the installed Jotiva from Applications. Expand `已打开开关，仍提示未授权？` in the permission prompt and choose `在 Finder 中显示这份 Jotiva` to locate the running copy. If the macOS Accessibility list still has the old copy, select it, click the minus button, then use the plus button to add and enable the app you located in Finder.

## Organize files and recover edits

Add folders to the workspace to browse files in place. Select a folder and press `⌘N` to make a document there. Saved quick notes go to the built-in archive folder (`其他`). Use the path in the status bar to locate a note in Finder or move it to another archive folder.

If you run into a save conflict, choose **Save My Edits as a Copy and Reload** (`另存编辑并重载`). Jotiva saves your edits to the built-in archive before trying to reopen the original file.

If a file has moved or its drive is disconnected, reopening a previously saved document from Jotiva can show its last saved text with a message that the original is unavailable. Reconnect the drive or use `另存编辑并重载` to keep that text as a new note in `其他`.

To find the built-in archive in Finder, press `⇧⌘G` and enter `~/Library/Application Support/io.github.lin037.jot.desktop/notes/其他/`. This folder persists across restarts and system temporary-file cleanup. Preferences, workspace roots, and archive settings live under `~/Library/Application Support/io.github.lin037.jot.desktop/`. Put your own archive folders where you normally back up your documents. Jotiva rejects system temporary directories as archive destinations.

## Get a folder suggestion with Jev

Jev filing is off by default. You can use a provider’s service or a self-hosted service that implements the Jev protocol. Remote services require HTTPS. For a service running on the same Mac, you can use `http://127.0.0.1:8080` or `http://[::1]:8080`. Replace `8080` with the port your service uses.

1. Open **Settings → Filing** (`设置 → 归档`). If your provider gives you a base URL, choose `基础地址` and enter that URL, including any path prefix. If you have the full request URL, choose `完整接口地址` and enter it as supplied. The model name defaults to `jev-latest`. For a self-hosted service, enter the name it accepts.
2. Under **API 密钥**, check whether the address above has a key. Choose `配置密钥…`, verify the request URL in the dialog and enter the key. The key stays in the macOS Keychain. You can replace or remove it here. Select `允许 Jev 归档` and click `保存设置` to save the filing switch, address and model. You can also save those preferences first and add a key later.
3. Add or edit an archive folder and select `参与 Jev 分类`. Describe what belongs in it to help Jev distinguish similar folders.
4. Open a saved note in `其他`, then choose `用 Jev 归档` below the editor or in the File menu. This sends up to the first 6,000 characters of the note, along with the names and descriptions of eligible folders, to the endpoint you entered. Creating, editing and saving notes are local operations. A suggestion needs at least 20 letters or digits. Shorter notes can be moved by hand.
5. Review the suggested folder and confirm the move or cancel. The note stays in `其他` if you cancel, no folder matches, or the service cannot be reached.

## Customize Jotiva

Open Settings with `⌘,`. Under **快捷键**, search for an action, click its current key, press your new combination, then choose **保存快捷键**. The settings cover global shortcuts, window commands and editor actions. The reference rows also list the standard editing and navigation keys. [Shortcut reference](../shortcuts.md) · [Sample documents](../../showcase/README.md)
