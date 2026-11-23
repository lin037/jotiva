import { bootstrapEditorWorker } from "@jot-md/editor/worker";

void bootstrapEditorWorker({
  endpoint: self,
  loadWasm: () => import("@jot-md/editor/wasm"),
});
