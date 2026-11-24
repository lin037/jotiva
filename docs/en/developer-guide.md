# Jotiva developer guide

[Project home](../../README.md) · [User guide](user-guide.md) · [中文](../developer-guide.md)

Jotiva is a Tauri 2 app for macOS, Windows and Linux. TypeScript manages the views and editing sessions. Rust manages document history, disk writes, and coordination between windows. The Markdown editor is a pinned component dependency. [Jot Markdown Engine](https://github.com/lin037/jot-md-engine)

## Run from source

Install Rust through rustup, Node.js 24, and pnpm 11.11.0. The repository pins Rust 1.100.0. macOS needs Xcode Command Line Tools. Windows needs the MSVC desktop C++ toolchain and WebView2. On Linux, install the GTK and WebKitGTK 4.1 development packages, plus AppIndicator, OpenSSL and libxdo. [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)

```sh
pnpm install --frozen-lockfile
pnpm tauri dev
```

The development server listens on `127.0.0.1:1420`. Run through Tauri to exercise native interactions. Windows and WSL should each install their own frontend dependencies.

## Find the code by feature

| Path | What it owns |
| --- | --- |
| `src/views/`, `src/editor/` | Workspace, settings, document and quick-note views. The editor layer owns session state, the worker, and save coordination. |
| `src/desktop/`, `src-tauri/src/main.rs` | IPC types and calls, Tauri commands and platform setup. |
| `src-tauri/src/desktop.rs`, `windows.rs` | Note presentation, window lifecycle and focus transitions. |
| `src-tauri/src/library/`, `storage.rs` | Shared document revisions, archives, file writes and recovery. |
| `src-tauri/src/platform/` | Host windows, file managers, storage operations and credentials on each desktop. |
| `qa/` | WebKit tests, native desktop replays, and package checks. |

Use `platform/contract.rs` for the host and file-manager interfaces. Add system operations under `platform/macos/`, `platform/windows/`, or `platform/linux/`. Check for a shared Tauri implementation before adding a new platform implementation. Window and document rules belong in their existing shared modules. [Architecture notes (中文)](../architecture.md)

## Test and contribute

Run `pnpm build`, `pnpm test:unit`, and `pnpm test:rust`. The [test guide](../verification.md) lists the WebKit, native macOS, interactive Windows, and X11 and Wayland Linux checks. File changes should cover both normal saves and interruptions or conflicts. For Jev changes, run `pnpm test:filing` with the local mock endpoint.

Use strict TypeScript and rustfmt. Keep both lockfiles in sync with dependency changes. When reporting a desktop issue, include the app version, OS, architecture, and a small sample document. Include the host application and display setup for an attachment issue, or the session type if you use Linux. [Contributing](../../CONTRIBUTING.md)
