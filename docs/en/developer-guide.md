# Jotiva developer guide

[Project home](../../README.en.md) · [User guide](user-guide.md) · [中文](../developer-guide.md)

Jotiva is a Tauri 2 app. The TypeScript side runs the interface and editing sessions. Rust handles disk writes, revisions, and window behavior. The Markdown editor comes from a pinned release of Jot Markdown Engine. [Jot Markdown Engine](https://github.com/lin037/jot-md-engine)

## Get a local build running

Install Rust through rustup, Node.js 24, and pnpm 11.11.0. Rustup picks up Rust 1.100.0 from the repository's toolchain file. On macOS, install the Xcode Command Line Tools. On Windows, install the MSVC desktop C++ tools and WebView2. On Linux, install the development packages for GTK, WebKitGTK 4.1, AppIndicator, OpenSSL, and libxdo. [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)

Run these from the checkout.

```sh
pnpm install --frozen-lockfile
pnpm tauri dev
```

Vite serves the interface at `127.0.0.1:1420`. Use `pnpm tauri dev` when you need file dialogs, native menus, or window attachment. If you work in both Windows and WSL, install the frontend dependencies separately in each environment.

## Where to start reading

| If you're changing… | Start here |
| --- | --- |
| The workspace, settings, or editor | `src/views/` for each window's UI. `src/editor/` holds the session, worker, and save queue. |
| Calls between the frontend and Rust | `src/desktop/` for frontend calls and types. `src-tauri/src/main.rs` for Tauri commands and setup. |
| When a quick note appears or hides | `src-tauri/src/desktop.rs` for the decisions. `windows.rs` for presentation and focus. |
| What happens to a file on save | Follow the save queue in `src/editor/note-editor.ts` through the Rust `save_note` command to `Notes::save`. File operations live in `src-tauri/src/library/` and `storage.rs`. |
| A feature that depends on the operating system | `src-tauri/src/platform/` for host windows, file managers, storage operations, and credentials. |
| Tests | `qa/` for WebKit checks, native desktop scenarios, and package checks. |

The host-window and file-manager interfaces live in `platform/contract.rs`. Look for a shared Tauri API first. When a feature needs a native call, put it in the matching platform module. Keep the behavior that all three platforms share in the existing window or document code. [Architecture notes (中文)](../architecture.md)

## Test your changes

Run `pnpm build`, `pnpm test:unit`, and `pnpm test:rust`. If you touch file handling, check what happens after a normal save, an interrupted save, and a conflict. If you change Jev filing, run `pnpm test:filing` against the local mock service. The test guide covers WebKit and the native checks for macOS, Windows, X11, and Wayland. [Test guide (中文)](../verification.md)

Use strict TypeScript and run rustfmt on Rust changes. Include both lockfiles when they change. In a bug report, give the Jotiva version, operating system, architecture, and the smallest sample that reproduces the issue. For window attachment, include the other app and your display setup. On Linux, say whether you're using X11 or Wayland. [Contributing](../../CONTRIBUTING.md)

## Branches and releases

`main` is the only long-lived branch. Start each change from the latest `main`, open a pull request against `main`, and wait for CI before merging. Delete the remote working branch after merge. Run `git fetch --prune origin` to remove stale remote-tracking refs locally.

| Branch | Purpose |
| --- | --- |
| `feat/<topic>`, `fix/<topic>` | Features, fixes, and routine maintenance. Name the change, for example `fix/windows-window-focus`. |
| `verify/<topic>` | Run cross-platform CI on push. Delete the branch after verification once any changes worth keeping have landed on `main`. |
| `release/X.Y.Z` | Prepare version updates and release notes together when needed. Delete the branch after merging the release PR. |
| `build/vX.Y.Z` | Build installers for verification. A push starts `Release packages`. Delete the branch once the packages have been checked and any changes worth keeping have landed on `main`. |

For a formal release, tag the release commit on `main` as `vX.Y.Z`. Manually run `Release packages` on that tag and select `publish`. The workflow checks that the source versions match the tag, runs CI, and publishes the packages to GitHub Releases after the checks pass. Use version tags to find past releases. [CI](https://github.com/lin037/jotiva/actions/workflows/ci.yml) · [Release packages](https://github.com/lin037/jotiva/actions/workflows/release.yml)
