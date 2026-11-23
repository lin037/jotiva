import type { MarkdownEditor } from "@jot-md/editor";
import { onLocalShortcuts, type LocalBindings } from "../desktop/shortcuts";

const componentKeys: Record<string, string> = {
  Control: "Ctrl", Super: "Meta", Space: "Space", Comma: ",", Period: ".",
  Slash: "/", Backslash: "\\", Minus: "-", Equal: "=", BracketLeft: "[",
  BracketRight: "]", Backquote: "`", Semicolon: ";", Quote: "'",
};

// Scope bindings to this editor's registry. Optional plugins may add or remove
// their commands while the document is open.
export function bindEditorShortcuts(editor: MarkdownEditor) {
  let preferences: LocalBindings = {};
  let hadChanges = false;
  function refresh() {
    const slots = new Set(editor.commands.shortcutSettings.map(slot => slot.id));
    const changes: LocalBindings = {};
    for (const [id, chord] of Object.entries(preferences)) {
      if (slots.has(id)) {
        changes[id] = chord === null ? null : chord.split("+")
          .map(part => part.startsWith("Key") ? part.slice(3).toLowerCase()
            : componentKeys[part] ?? part.replace(/^Digit/, ""))
          .join("-");
      }
    }
    const changed = Object.keys(changes).length > 0;
    if (changed || hadChanges) {
      // Rebuild from defaults in one step so clearing an override restores the
      // original binding. Resolve Mod for macOS before checking for collisions.
      const defaults = Object.fromEntries(editor.commands.defaultShortcuts.map(slot => [slot.id, slot.key]));
      editor.commands.setShortcutDefaults(defaults, changes, "mac");
    }
    hadChanges = changed;
  }
  const stop = onLocalShortcuts(bindings => { preferences = bindings; refresh(); });
  editor.disposeSignal.addEventListener("abort", stop, { once: true });
  return refresh;
}
