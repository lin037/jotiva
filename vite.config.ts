import { defineConfig } from "vite";
import editorRelease from "./node_modules/@jot-md/editor/package.json" with { type: "json" };

export default defineConfig({
  clearScreen: false,
  cacheDir: `node_modules/.vite/editor-${editorRelease.version}`,
  // The worker entry is discovered after the first editor opens in development.
  optimizeDeps: { include: ["@jot-md/editor/worker", "@jot-md/editor/wasm"] },
  build: {
    // Ship KaTeX fonts as same-origin assets under the desktop CSP.
    assetsInlineLimit: path => /\.(woff2?|ttf)$/.test(path) ? false : undefined,
  },
  server: {
    host: "127.0.0.1",
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  worker: { format: "es" },
});
