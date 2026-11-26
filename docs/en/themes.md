# Appearance, Typography & Themes

[Project Home](../../README.en.md) · [User Guide](user-guide.md) · [中文](../themes.md)

Jotiva keeps **themes** separate from **personal typography preferences**: switch or share full color themes anytime while keeping your font size, writing font, heading scale, and custom CSS settings. Themes can still change the default body text scale.

---

## 1. Personal Typography & Built-in Themes

Open **Settings → Appearance** and **Settings → Editor** to customize your setup (open windows and terminals update immediately):

- **Appearance mode & accent**: Choose **Light**, **Dark**, or **System**, pick from **Mist blue**, **Pine green**, **Wisteria**, or **Terracotta**, or set a **Custom…** button fill (Jotiva automatically adjusts contrast for readable labels and links).
- **Interface theme**: Choose between **Default**, **Ink and Steel**, **Material**, and **Blueprint**—each with light and dark modes.
- **Typography independent of themes**:
  - **Base font size** (`11–18 px`) scales window controls, sidebars, dialogs, and quick notes together.
  - **Writing font** (System font, Serif, or Monospace) and **Heading sizes** (`80%–240%` of body text for H1–H6).
  - Toggles for **Ruled paper**, **Wrap lines in code blocks**, **Show heading fold controls**, and **Show main editor toolbar**.
  - **Global CSS tweaks**: Paste or write CSS directly, click **Format** in the bottom-left to tidy it, and click **Save** in the bottom-right. Your tweaks persist across theme switches; clear the box and save to remove them.

---

## 2. Importing & Sharing CSS Themes

An imported Jotiva theme is a single, non-empty UTF-8 `.css` file (up to 128 KB), making it easy to version, share, and update:

1. Under **Settings → Appearance → My themes**, click **Import theme…** and select a `.css` file. Jotiva copies it into its theme folder and uses the filename (without `.css`) as the theme name.
2. Select the imported theme from **Interface theme** to apply it immediately.
3. **Update or rename**: Importing a file with an existing name lets you update the installed theme or save a copy under a new name. Updates to the active theme apply immediately. Renaming a theme in **My themes** renames the installed file automatically.

---

## 3. Writing Custom Themes (Advanced)

### Quick start from Ink and Steel
The built-in [ink-and-steel.css](../assets/ink-and-steel.css) and [blueprint.css](../assets/blueprint.css) themes define compact color tables at the top of the file. Copy one under a new filename and adjust its top-level variables:

| Target | Variables in Ink and Steel |
| --- | --- |
| Accent & buttons | `--ink-accent` (links, flowchart borders, Gantt bars) and `--ink-on-accent` (text on buttons and task bars) |
| Paper & sidebar | `--ink-paper` (editor background) and `--ink-panel` (sidebar background) |
| Body & muted text | `--ink-text` (body and diagram text) and `--ink-muted` (secondary labels and connectors) |

### Standard CSS tokens (`--jot-*`)
For themes or **Global CSS tweaks** that work reliably across presets, override the public `--jot-*` tokens defined in [tokens.css](../../src/styles/tokens.css):

```css
:root {
  --jot-color-accent: #6760ad;
  --jot-color-accent-fill: #6760ad;
  --jot-color-on-accent: #ffffff;
  --jot-color-sidebar: #f5f2fa;
  --jot-editor-scale: 1.15;
  --jot-quick-scale: 1.08;
  --jot-sidebar-width: clamp(12rem, 24%, 17rem);
}

:root[data-jot-scheme="dark"] {
  --jot-color-sidebar: #262231;
  --jot-color-accent: #c6b8fd;
  --jot-color-accent-fill: #c6b8fd;
  --jot-color-on-accent: #1c1826;
}
```

| Scope | Key tokens / selectors |
| --- | --- |
| **Core palette** | `--jot-color-bg`, `--jot-color-text`, `--jot-color-muted`, `--jot-color-hover`, `--jot-color-active`, `--jot-color-line`, `--jot-color-backdrop` |
| **Accent & focus** | `--jot-color-accent`, `--jot-color-accent-fill`, `--jot-color-on-accent`, `--jot-color-focus`, `--jot-focus-ring` |
| **Layout & scale** | `--jot-color-chrome`, `--jot-color-sidebar`, `--jot-sidebar-width`, `--jot-outline-width`, `--jot-font-base`, `--jot-editor-scale`, `--jot-quick-scale` |
| **Editor & paper** | `--jot-editor-font-family`, `--jot-editor-font-size`, `--jot-quick-font-size`, `--jot-editor-pad-inline`, `--jot-color-rule-paper`, `--jot-editor-heading-ratio-1`–`6` |
| **Code & quotes** | `--jot-color-surface` (inline code background), `--jot-color-quote`, `--jot-color-line-strong` |
| **Mermaid diagrams** | Flowcharts and sequence diagrams map `--jot-color-surface`, `--jot-color-accent`, `--jot-color-text`, and `--jot-color-muted`; multi-series charts (pie, git, mindmap, timeline) use `--jot-color-series-1` through `12` |
| **Windows & contexts** | `.jot-workspace`, `.jot-document-window`, `.jot-quick`, `.jot-titlebar`, `.jot-sidebar`, `.jot-settings`, `.jot-editor[data-jot-context="workspace"]`, `.jot-editor[data-jot-context="quick"]` |

> [!NOTE]
> Dragging a pane divider saves a custom pane width that overrides theme defaults; double-click the divider to reset it. Rely on the public tokens above rather than internal editor DOM class names, which may change between engine versions.
