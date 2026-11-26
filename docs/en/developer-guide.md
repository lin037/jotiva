# Jotiva Developer Guide

[Project Home](../../README.en.md) · [User Guide](user-guide.md) · [Architecture](architecture.md) · [Verification & Releasing](verification.md) · [简体中文](../developer-guide.md)

Jotiva is a Tauri 2 desktop application. The TypeScript frontend manages window views, editing sessions, and shortcut dispatch, while the Rust backend handles disk persistence, library indexing, host-window tracking, the integrated terminal, and OS integration. Core Markdown parsing and editing are powered by a pinned release of [Jot Markdown Engine](https://github.com/lin037/jot-md-engine).

---

## Local Setup

### Prerequisites
- **Rust** via `rustup` (pinned to `1.100.0` by the repository toolchain file).
- **Node.js 24** and **pnpm 11.11.0**.
- **Platform dependencies** ([Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)):
  - **macOS**: Xcode Command Line Tools
  - **Windows**: MSVC desktop C++ build tools and Microsoft Edge WebView2 Runtime
  - **Linux**: Development packages for WebKitGTK 4.1, GTK, AppIndicator, OpenSSL, and `libxdo`

> If you develop across both Windows and WSL, install frontend dependencies separately in each environment (keep your WSL checkout inside the Linux filesystem).

### Start in development mode

```sh
pnpm install --frozen-lockfile
pnpm tauri dev
```

`pnpm dev` starts the Vite server at `127.0.0.1:1420`. The frontend needs Tauri's window APIs, so use `pnpm tauri dev` to run the app locally. Keep `pnpm-lock.yaml` and `src-tauri/Cargo.lock` in sync when updating dependencies.

---

## Repository Map

| Location | Responsibility |
| --- | --- |
| `src/main.ts` | Mounts the workspace, quick-note, document, or settings view based on window label. |
| `src/views/` | Entry views for `workspace` (tree, tabs, terminal), `quick`, `document`, and `settings`. |
| `src/editor/` | Editing session, worker bridge, save queue, revision merge, images, and outline (`note-editor.ts` is shared across all three editor windows). |
| `src/desktop/` | Frontend IPC wrappers, shortcut definitions, and focus/window helpers. |
| `src/appearance/`, `src/ui/`, `src/styles/` | Theme resolution, shared controls/lists, and CSS design tokens. |
| `src-tauri/src/main.rs`, `events.rs` | Tauri command registration, shared app state, and frontend/backend events. |
| `src-tauri/src/desktop.rs`, `desktop/note_windows.rs`, `windows.rs` | Window state transitions, host binding, quick-note panels, and standalone document windows. |
| `src-tauri/src/library/`, `storage.rs` | Notes library, workspace/archive indexing, file actions, and atomic writes. |
| `src-tauri/src/images/`, `terminal.rs`, `updates.rs` | Image storage/compression/upload, PTY terminal sessions, and app updates. |
| `src-tauri/src/host.rs`, `platform/{macos,windows,linux}/`, `menus.rs` | Platform traits, OS adapters, and native application menus. |
| `src-tauri/src/settings/` | Appearance, shortcuts, Jev filing settings, and credential lookup. |
| `qa/`, `scripts/` | Unit tests, Playwright WebKit suite, native smoke tests, and packaging scripts. |

See [Architecture](architecture.md) for data flow and platform boundaries.

---

## Building & Testing

Run the baseline checks from the repository root:

```sh
pnpm build          # TypeScript check and production Vite bundle
pnpm test:unit      # Frontend unit tests
pnpm test:rust      # Rust test suite
```

Before submitting Rust changes, verify formatting and lints:

```sh
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

- For Playwright WebKit suites, macOS/Windows/Linux desktop smoke tests, and release signing setup, see [Verification & Releasing](verification.md).
- After upgrading `@jot-md/editor` or changing lockfiles, run `python3 scripts/third-party-notices.py` (Python 3.10+) and commit the updated [THIRD-PARTY-NOTICES.txt](../../src-tauri/notices/THIRD-PARTY-NOTICES.txt).

---

## Conventions

- **TypeScript**: Keep strict type checking clean. Keep view-specific DOM/CSS inside `src/views/`; move logic to `ui/` or `editor/` only when shared across windows.
- **IPC boundaries**: Update Rust commands and TypeScript wrappers together (payload shape, event scope, and error messages), and keep window IDs separate from document IDs.
- **Rust & persistence**: Format with `rustfmt`, reuse `library/` and `storage.rs` for disk mutations, and verify failure/rollback and restart recovery.
- **Platform code**: Keep OS-specific calls behind `src-tauri/src/platform/contract.rs`, prefer built-in Tauri APIs when available, and return actionable error messages to users.

---

## Branches & Contributing

`main` is the only long-lived branch. Branch from `main`, open a pull request, wait for CI to pass, and delete the working branch after merge.

| Branch | Purpose |
| --- | --- |
| `feat/<topic>`, `fix/<topic>`, `docs/<topic>` | Features, bug fixes, docs, and routine maintenance. |
| `verify/<topic>` | Runs cross-platform CI on push; delete once verified changes land on `main`. |
| `release/X.Y.Z` | Prepares version bumps and release notes; delete after merging. |
| `build/vX.Y.Z` | Triggers `Release packages` pre-release installers; delete after verification. |

See [Contributing](contributing.md) for PR and issue guidelines. Jotiva is licensed under the [MIT License](../../LICENSE).
