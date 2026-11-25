import { desktopPlatform } from "../desktop/platform";
import type { MarkdownEditor } from "@jot-md/editor";
import { commands, onLocalShortcuts, type LocalBindings } from "../desktop/shortcuts";

const componentKeys: Record<string, string> = {
  Control: "Ctrl", Super: "Meta", Space: "Space", Comma: ",", Period: ".",
  Slash: "/", Backslash: "\\", Minus: "-", Equal: "=", BracketLeft: "[",
  BracketRight: "]", Backquote: "`", Semicolon: ";", Quote: "'",
};

function editorChord(chord: string) {
  return chord.split("+").map(part => part.startsWith("Key")
    ? part.slice(3).toLowerCase() : componentKeys[part] ?? part.replace(/^Digit/, "")).join("-");
}

// Scope bindings to this editor's registry. Optional plugins may add or remove
// their commands while the document is open.
export function bindEditorShortcuts(editor: MarkdownEditor) {
  // The editor supplies Mod-Shift-Z. The desktop Edit menu also exposes Ctrl+Y.
  if (desktopPlatform.editor === "windows") {
    editor.commands.bind({ id: "history.redo.ctrl-y", command: "history.redo", key: "Ctrl-y" });
  }
  let preferences: LocalBindings = {};
  let hadChanges = false;
  function refresh() {
    const slots = new Set(editor.commands.shortcutSettings.map(slot => slot.id));
    const changes: LocalBindings = {};
    // Desktop menus, the shortcut recorder and the editor share the same accelerators.
    if (desktopPlatform.editor === "windows") {
      for (const command of commands) {
        if (command.editor && slots.has(command.id)) {
          changes[command.id] = command.key ? editorChord(command.key) : null;
        }
      }
    }
    for (const [id, chord] of Object.entries(preferences)) {
      if (slots.has(id)) changes[id] = chord === null ? null : editorChord(chord);
    }
    const changed = Object.keys(changes).length > 0;
    if (changed || hadChanges) {
      const defaults = Object.fromEntries(editor.commands.defaultShortcuts.map(slot => [slot.id, slot.key]));
      editor.commands.setShortcutDefaults(defaults, changes, desktopPlatform.editor);
    }
    hadChanges = changed;
  }
  const stop = onLocalShortcuts(bindings => { preferences = bindings; refresh(); });
  editor.disposeSignal.addEventListener("abort", stop, { once: true });
  return refresh;
}
