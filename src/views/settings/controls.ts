import type { Label } from "../../i18n";
import { element, report } from "../../ui/dom";

export function field(name: Label, controls: HTMLElement | HTMLElement[], id?: string) {
  const row = element("div", "settings-field");
  const label = id ? element("label", undefined, name) : element("span", undefined, name);
  const items = element("div", "settings-field-controls");
  if (id) label.setAttribute("for", id);
  items.append(...(Array.isArray(controls) ? controls : [controls]));
  row.append(label, items);
  return row;
}

export function select<T extends string>(id: string, options: readonly (readonly [T, Label])[], changed?: (value: T) => unknown) {
  const input = element("select");
  input.id = id;
  for (const [value, text] of options) {
    const option = element("option", undefined, text);
    option.value = value;
    input.append(option);
  }
  if (changed) input.addEventListener("change", () => { void Promise.resolve().then(() => changed(input.value as T)).catch(report); });
  return input;
}
