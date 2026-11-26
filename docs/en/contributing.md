# Contributing to Jotiva

Thanks for your interest in contributing to Jotiva! Documentation fixes, test cases, bug fixes, and workflow improvements are all welcome.

[中文](../../CONTRIBUTING.md) · [Developer Guide](developer-guide.md) · [Verification Guide](verification.md)

## Before You Start

- Check existing [Issues](https://github.com/lin037/jotiva/issues) and [Discussions](https://github.com/lin037/jotiva/discussions) to see if a similar topic is already active.
- For changes affecting file formats, save/merge behavior, or platform-specific window handling, describe the user scenario and expected behavior first.

## Opening a Pull Request

1. Branch from the latest `main` (fork the repository if you do not have write access). Suggested prefixes: `feat/<topic>`, `fix/<topic>`, or `docs/<topic>`.
2. Keep each PR focused and self-contained:
   - Update the [User Guide](user-guide.md) (and [Chinese user guide](../user-guide.md)) if user-facing behavior, shortcuts, or settings change.
   - Commit updated lockfiles and run `python3 scripts/third-party-notices.py` if dependencies in `package.json` or `Cargo.toml` change.
3. Verify baseline checks (`pnpm build`, `pnpm test:unit`, `pnpm test:rust`), and include your OS, test commands, and results in the PR description (plus screenshots for UI changes, or recovery notes for file-write changes).

## Reporting Bugs

Include your Jotiva version, OS and architecture (and X11 vs. Wayland on Linux), minimal reproduction steps, and expected vs. actual behavior. For window attachment issues, mention the host application, display setup, and fullscreen state.

> [!NOTE]
> Remove personal folder paths, sensitive titles, note contents, and API keys before sharing screenshots or logs. For security vulnerabilities, report privately via [Security Policy](../../SECURITY.md) instead of opening a public issue.
