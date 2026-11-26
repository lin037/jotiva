# Verification & Releasing Guide

[Developer Guide](developer-guide.md) · [Architecture](architecture.md) · [简体中文](../verification.md)

This guide covers baseline checks, the Playwright WebKit test suite, native desktop smoke runs on macOS/Windows/Linux, and signed release configuration for maintainers. Run `pnpm install --frozen-lockfile` before running these commands.

---

## 1. Baseline Checks (Every Change)

| Check | Command | What it verifies |
| --- | --- | --- |
| Typecheck & bundle | `pnpm build` | TypeScript compilation and Vite production build. |
| Frontend unit tests | `pnpm test:unit` | Accent contrast, button text legibility, and search highlights. |
| Rust lints | `cargo fmt --check --manifest-path src-tauri/Cargo.toml` and `cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` | Formatting and zero-warning Clippy checks. |
| Rust test suite | `pnpm test:rust` | Atomic writes, conflicts/rollbacks, migrations, window state, and shortcuts. |

GitHub Actions runs frontend builds, Rust checks across all three platforms, macOS WebKit integration tests, and Ubuntu X11 / Wayland smoke runs on pull requests and pushes to `main` or `verify/**`. [CI Workflow](https://github.com/lin037/jotiva/actions/workflows/ci.yml)

---

## 2. WebKit Integration Suite

Set up the Python environment and Playwright WebKit once:

```sh
python3 -m venv .venv
source .venv/bin/activate
python3 -m pip install -r qa/requirements.txt
python3 -m playwright install webkit
pnpm test:web
```

`pnpm test:web` runs `pnpm build` followed by `test:web:editor` and `test:web:ui`. Build `dist/` with `pnpm build` before running individual suites below (set `WEBKIT_EXECUTABLE` if pointing to a custom WebKit binary):

| Command | Coverage |
| --- | --- |
| `pnpm test:web:editor` | Cross-window merge & conflict recovery, tab undo history, quick-note resume, and worker reclamation (`python3 qa/document-windows.py --sync` / `--history` / `--resume`). |
| `pnpm test:web:ui` | File tree, window picker, focus navigation, and Accessibility grant/revoke/retry flows. |
| `pnpm test:web:visual` | Screenshot checks for heading carets and scrollbars (best run on a fixed display scale). |
| `pnpm test:theme` | Theme import, update, rename, and global CSS formatting/sync across windows. |
| `pnpm test:shortcuts` | Window dispatch, focus navigation, shortcut search/recording/save, and reset to defaults. |
| `pnpm test:capacity` | Large-folder scans, tree pagination, multiple tabs, and long-document scrolling. |
| `pnpm test:filing` | Loopback HTTP test for Jev filing, timeouts, errors, API key management, and confirmation moves (no live API key required). |
| `pnpm test:filing:live` | Optional live check: set `JEV_API_KEY` in `.env`, or export `JOT_JEV_ENDPOINT` and `JOT_JEV_API_KEY` for a custom endpoint. |
| `python3 qa/document-windows.py --history-compat` | Upgrade diagnostic checking tab switches when history snapshots are unavailable. |

---

## 3. Native Desktop Smoke Tests

### Windows
On an unlocked Windows desktop with Microsoft Edge and WebView2 Runtime, run `pnpm build` followed by `node qa/windows-smoke.mjs`. The script uses isolated profiles and a 350-line UTF-8 CRLF fixture to test drag-to-attach, long-document shortcuts, cross-window sync, host close detachment, and Explorer file reveal.

### Linux
On Ubuntu 24.04 or WSL Ubuntu, install `xvfb xauth dbus-x11 openbox xdotool zenity weston thunar fonts-noto-cjk` in addition to build prerequisites, run `pnpm build`, then run:

```sh
GDK_BACKEND=x11 WEBKIT_DISABLE_DMABUF_RENDERER=1 LIBGL_ALWAYS_SOFTWARE=1 \
  xvfb-run -a dbus-run-session -- python3 qa/linux-smoke.py
WEBKIT_DISABLE_DMABUF_RENDERER=1 LIBGL_ALWAYS_SOFTWARE=1 \
  xvfb-run -a dbus-run-session -- python3 qa/linux-smoke.py --wayland
```

To verify `.deb` file associations, run `xvfb-run -a dbus-run-session -- python3 qa/linux-package.py <path-to.deb>`.

### macOS
Run on an unlocked, logged-in macOS desktop and avoid touching the keyboard/mouse during automated input runs (the shared test binary builds into `qa/.cache/target`):

| Scenario | Command |
| --- | --- |
| Standard native smoke run | `pnpm test:macos:smoke` (or `python3 qa/macos/run.py --smoke --quick-key u` if the default shortcut is taken) |
| Disk persistence, conflicts & restart recovery | `pnpm test:macos:storage` |
| Shortcut recording, menus & global conflicts | `pnpm test:macos:shortcuts` |
| Build isolated test bundle only | `python3 qa/macos/run.py --build-only` |
| Multi-window z-order & document flows | `python3 qa/macos/sibling-windows.py` (`--main-open`), `python3 qa/scenarios/window_flows.py capture documents` |
| Multi-display, fullscreen host & Spaces | `python3 qa/scenarios/window_flows.py displays fullscreen`, `python3 qa/scenarios/window_flows.py spaces --space-display 1` |
| Cold/warm quick-note launch & WebView rebuild | `python3 qa/macos/run.py --quick-key u` |

---

## 4. Signed Releases & Auto-Updater (Maintainers)

Releases are driven by [Release packages](../../.github/workflows/release.yml):

1. Tag a release commit on `main` as `vX.Y.Z` (matching `package.json`, `Cargo.toml`, and `tauri.conf.json`).
2. Manually trigger `Release packages` on that tag with `publish` enabled (to test signed pre-release artifacts on a `build/vX.Y.Z` branch, set repository variable `JOTIVA_SIGNED_RELEASE=true`).
3. After checks pass, the workflow publishes macOS Universal installers (`.dmg`, `.pkg`, `.zip`), Windows x64 NSIS, Linux x86_64 (`.deb`, `.AppImage`), source archives, `latest.json`, `SHA256SUMS`, and `HelloJotiva.zip` to GitHub Releases. Extract the sample archive to open either language version and its image offline.

### Required GitHub Actions Variables & Secrets

| Purpose | Configuration |
| --- | --- |
| **Updater signing** | Variable `JOTIVA_UPDATER_PUBLIC_KEY`; Secrets `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (back up the private key offline and keep it consistent across releases). |
| **macOS signing & notarization** | Secrets `JOTIVA_MAC_CERTIFICATES` (Base64 `.p12` containing Developer ID Application & Installer identities) and `JOTIVA_MAC_CERTIFICATE_PASSWORD`; Variables `APPLE_SIGNING_IDENTITY`, `JOT_INSTALLER_IDENTITY`, `APPLE_TEAM_ID`; Secrets `APPLE_ID` and `APPLE_PASSWORD` (app-specific password). |
| **Windows Authenticode** | Secrets `JOTIVA_WINDOWS_CERTIFICATE` (Base64 `.pfx`) and `JOTIVA_WINDOWS_CERTIFICATE_PASSWORD` (replace the import step if using hardware or cloud signing). |

> To verify macOS Accessibility permission continuity across upgrades, upgrade in place between builds signed with the same Developer ID team, bundle identifier, and install path.
