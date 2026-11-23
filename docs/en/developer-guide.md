# Jotiva Developer Guide

[Project home](../../README.md) · [User guide](user-guide.md) · [简体中文](../developer-guide.md)

Jotiva is a standalone Tauri 2 application. TypeScript implements the window views and editor session, Rust coordinates documents and windows, and Objective-C handles macOS integration. The Markdown editor comes from a pinned [component release](https://github.com/lin037/jot-md-engine/releases/tag/v0.3.1).

## Run from source

Install the Xcode Command Line Tools, Rustup, Node.js 24, and pnpm 11.11.0. The repository pins Rust 1.100.0. Run these commands from the directory containing Jotiva’s `package.json`.

```sh
pnpm install --frozen-lockfile
pnpm tauri dev
```

Vite serves the development UI at `127.0.0.1:1420`. Use the Tauri command to test native windows and file operations. Keep both lockfiles in version control.

## Where to make a change

| Directory | Responsibility |
| --- | --- |
| `src/views/` | Main, quick note, settings, and independent document views. Keep window-specific components near the view they serve. |
| `src/editor/` | Editor sessions, revision sync, saving, worker integration, and editor layout. |
| `src/desktop/`, `src/ui/`, `src/appearance/`, `src/styles/` | Frontend IPC and window actions, reusable controls, and appearance. |
| `src-tauri/src/main.rs`, `events.rs` | Commands, shared state, and event contracts. |
| `src-tauri/src/desktop.rs`, `windows.rs`, `host/` | Window lifecycle, host observation, positioning, and focus. |
| `src-tauri/src/library/`, `storage.rs`, `settings/` | Notes and file operations, persistence, and preferences. |
| `qa/` | WebKit and native macOS regression scripts. |

Use strict TypeScript and format Rust with rustfmt. For IPC changes, check the frontend caller, the Rust handler, and which windows receive the event. Use existing file operations when changing storage and verify what happens on failure and restart. The [architecture notes](../architecture.md) describe the save path and window lifecycle.

## Verify and contribute

Run `pnpm build`, `pnpm test:unit`, and `pnpm test:rust`. The [testing guide](../verification.md) covers WebKit setup and native or visual tests. Native tests need a logged-in macOS desktop for keyboard, focus, and drag operations.

When filing an issue, include the Jotiva version, macOS version, the smallest sample document, and expected and actual behavior. For a floating note, include the host app and display setup. Fork the repository and open a pull request for a change that can be verified on its own. Describe the checks you ran and update the user guide when a workflow changes.

For filing changes, run `pnpm test:filing` against the local mock service. Use `pnpm test:macos:storage` to exercise saving and recovery on disk. [Jev request and response format](../architecture.md#jev-归档)

## License

Jotiva's source is available under the MIT License. Dependencies and fonts keep their respective licenses. After updating dependencies, run `python3 scripts/third-party-notices.py` (Python 3.10+) and include the updated notices in your change. [MIT License](../../LICENSE) · [Third-party notices](../../src-tauri/notices/THIRD-PARTY-NOTICES.txt)
