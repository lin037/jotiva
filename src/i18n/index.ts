import { environment } from "../desktop/environment";
import english from "./en.json";

export type Language = "zh-CN" | "en";
type Message = keyof typeof english;
export let language: Language = environment.language ?? "zh-CN";
document.documentElement.lang = language;

export type Label = string | (() => string);
export const localized = (source: Message) => () => t(source);

type Binding = () => void;
const bindings = new WeakMap<Element, Map<string, Binding>>();

export function setLanguage(next: Language) {
  if (!next || next === language) return;
  language = next;
  document.documentElement.lang = next;
  for (const node of document.querySelectorAll("[data-i18n]")) {
    bindings.get(node)?.forEach(update => update());
  }
  window.dispatchEvent(new Event("jot:language"));
}

/** Only application-owned labels are bound. Document text and input values remain owned by the user. */
export function bindLabel(node: Element, attribute: string, value: Label) {
  let slots = bindings.get(node);
  const apply = (text: string) => {
    if (attribute === "text") node.textContent = text;
    else node.setAttribute(attribute, text);
  };
  if (typeof value === "string") {
    slots?.delete(attribute);
    apply(value);
  } else {
    if (!slots) {
      bindings.set(node, slots = new Map());
      node.setAttribute("data-i18n", "");
    }
    let previous: string | undefined;
    const update = () => {
      const current = attribute === "text" ? node.textContent : node.getAttribute(attribute);
      if (previous !== undefined && current !== previous) { slots?.delete(attribute); return; }
      apply(previous = value());
    };
    slots.set(attribute, update);
    update();
  }
}

// Catalog keys are the source-language UI strings. User text never passes through this lookup.
export function message(source: string): string {
  return language === "en" && Object.prototype.hasOwnProperty.call(english, source) ? english[source as Message] : source;
}

export function t(source: Message, values?: Record<string, string | number>) {
  const pattern = message(source);
  return values ? pattern.replace(/\{(\w+)\}/g, (token, key: string) => String(values[key] ?? token)) : pattern;
}
