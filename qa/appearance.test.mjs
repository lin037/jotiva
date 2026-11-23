import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { accentColors, accentPresets } from "../src/appearance/accent.ts";

function luminance(hex) {
  const [r, g, b] = hex.slice(1).match(/../g).map(pair => {
    const value = Number.parseInt(pair, 16) / 255;
    return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

function verifyContrast(foreground, background) {
  const [lo, hi] = [luminance(foreground), luminance(background)].sort((a, b) => a - b);
  const ratio = (hi + 0.05) / (lo + 0.05);
  assert.ok(ratio >= 4.5, `${foreground} on ${background}: ${ratio}`);
}

test("presets pair light and dark fills and the default resets to blue", () => {
  for (const dark of [false, true]) {
    for (const preset of accentPresets) {
      assert.equal(accentColors(preset.light, dark).fill, dark ? preset.dark : preset.light);
    }
    assert.deepEqual(accentColors(null, dark), accentColors(accentPresets[0].light, dark));
  }
});

test("custom button labels and accent text stay readable in both appearances", () => {
  const colors = ["#ffffff", "#000000", "#ffff00", "#240f36", "#777777", "#00ffff", ...accentPresets.map(item => item.light)];
  for (const color of colors) {
    for (const dark of [false, true]) {
      const { fill, ink, onFill } = accentColors(color, dark);
      verifyContrast(onFill, fill);
      for (const surface of dark ? ["#222428", "#292c32", "#383e48"] : ["#ffffff", "#f6f7f8", "#e7eaf0"]) {
        verifyContrast(ink, surface);
      }
    }
  }
  assert.equal(accentColors("#240f36", true).onFill, "#ffffff");
  assert.equal(accentColors("#ffff00", false).onFill, "#000000");
});

test("search highlights preserve the contrast of body text and muted text", () => {
  const css = readFileSync(new URL("../src/styles/tokens.css", import.meta.url), "utf8");
  for (const theme of css.split(':root[data-jot-scheme="dark"]')) {
    const color = name => theme.match(new RegExp("--jot-color-" + name + ": (#[a-f0-9]{6});"))[1];
    for (const foreground of ["text", "muted"]) {
      for (const background of ["match", "match-current"]) verifyContrast(color(foreground), color(background));
    }
  }
});
