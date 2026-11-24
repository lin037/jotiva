// These IPC fixtures replay macOS key events. CodeMirror also consults navigator.platform.
window.__jotPlatform = { os: "macos", hostWindows: true };
Object.defineProperty(navigator, "platform", { configurable: true, value: "MacIntel" });
