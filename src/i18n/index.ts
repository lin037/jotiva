import { environment } from "../desktop/environment";
import english from "./en.json";

export type Language = "zh-CN" | "en";
type Message = keyof typeof english;
export const language: Language = environment.language ?? "zh-CN";
document.documentElement.lang = language;

// Catalog keys are the source-language UI strings. User text never passes through this lookup.
export function message(source: string): string {
  return language === "en" && Object.prototype.hasOwnProperty.call(english, source) ? english[source as Message] : source;
}

export function t(source: Message, values?: Record<string, string | number>) {
  const pattern = message(source);
  return values ? pattern.replace(/\{(\w+)\}/g, (token, key: string) => String(values[key] ?? token)) : pattern;
}
