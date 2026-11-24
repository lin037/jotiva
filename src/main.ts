import { bindNewDocumentDialog } from "./desktop/window-actions";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { bindAppearance } from "./appearance/preferences";
import { bindFileDrop } from "./desktop/file-drop";
import { loadLocalShortcuts } from "./desktop/shortcuts";
import { bindFocusRegions } from "./desktop/focus-regions";
import { report } from "./ui/dom";
import { bindTooltips } from "./ui/tooltip";
import "./styles/index.css";

const views = {
  main: () => import("./views/workspace/view"),
  quick: () => import("./views/quick/view"),
  settings: () => import("./views/settings/view"),
} as const;

const label = getCurrentWindow().label;
const quick = label === "quick" || /^quick-[1-9]\d*$/.test(label);
const id = /^document-([1-9]\d*)$/.exec(label)?.[1];
void bindNewDocumentDialog().catch(report);
bindTooltips();
bindFocusRegions();
if (label === "main" || quick || id) bindFileDrop();
if (id) {
  void Promise.all([import("./views/document/view"), bindAppearance(), loadLocalShortcuts()])
    .then(([view]) => view.mount(Number(id))).catch(report);
} else if (quick || label in views) {
  void Promise.all([views[quick ? "quick" : label as keyof typeof views](), bindAppearance(), loadLocalShortcuts()])
    .then(([view]) => view.mount()).catch(report);
}
