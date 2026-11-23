export const accentPresets = [
  { name: "雾蓝", light: "#456993", dark: "#9cbce4" },
  { name: "松绿", light: "#3e705f", dark: "#99c3af" },
  { name: "藤紫", light: "#785e8b", dark: "#c4afd6" },
  { name: "陶土", light: "#915e48", dark: "#d7ab93" },
] as const;

function channels(hex: string) {
  return [1, 3, 5].map(start => Number.parseInt(hex.slice(start, start + 2), 16));
}

function luminance(rgb: number[]) {
  const linear = rgb.map(byte => {
    const value = byte / 255;
    return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2];
}

function contrast(a: number, b: number) {
  return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
}

export function accentPreset(preference: string | null) {
  const light = preference?.toLowerCase() ?? accentPresets[0].light;
  return accentPresets.find(item => item.light === light);
}

export function accentColors(preference: string | null, dark: boolean) {
  const preset = accentPreset(preference);
  const fill = preset ? preset[dark ? "dark" : "light"] : preference!;
  const rgb = channels(fill);
  const light = luminance(rgb);
  const onFill = contrast(light, 1) >= contrast(light, 0) ? "#ffffff" : "#000000";

  // Check against a surface beyond the strongest built-in selection tint.
  const surface = luminance(channels(dark ? "#55585d" : "#d4d4d4"));
  const endpoint = dark ? 255 : 0;
  let ink = rgb;
  for (let step = 1; contrast(luminance(ink), surface) < 4.5; step++) {
    ink = rgb.map(byte => Math.round(byte + (endpoint - byte) * step / 20));
  }
  return { fill, onFill, ink: "#" + ink.map(byte => byte.toString(16).padStart(2, "0")).join("") };
}
