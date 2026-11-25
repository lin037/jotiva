import { bindLabel, type Label, localized } from "../../i18n";
import { invoke } from "@tauri-apps/api/core";
import { currentAppearance } from "../../appearance/preferences";
import { element, report } from "../../ui/dom";

const minimumWidth = 144;
const maximumWidth = 440;

export function workspaceDividers(workspace: HTMLElement, sidebar: HTMLElement, outline: HTMLElement, remeasure: () => void) {
  const layout = { ...currentAppearance().workspace };
  addDivider("sidebar", sidebar, localized("调整侧栏宽度"));
  addDivider("outline", outline, localized("调整大纲宽度"));

  function addDivider(side: keyof typeof layout, pane: HTMLElement, label: Label) {
    const divider = element("div", "workspace-divider");
    divider.tabIndex = 0;
    bindLabel(divider, "title", localized("拖动调整宽度，双击恢复默认"));
    divider.setAttribute("role", "separator");
    divider.setAttribute("aria-orientation", "vertical");
    divider.setAttribute("aria-controls", pane.id);
    bindLabel(divider, "aria-label", label);
    divider.setAttribute("aria-valuemin", String(minimumWidth));
    divider.setAttribute("aria-valuemax", String(maximumWidth));
    pane.append(divider);
    const direction = side === "sidebar" ? 1 : -1;
    const variable = `--workspace-preferred-${side}`;
    let drag: { pointer: number; start: number; width: number; next: number } | undefined;
    let frame = 0;

    function paint(width: number | null) {
      if (width === null) workspace.style.removeProperty(variable);
      else workspace.style.setProperty(variable, `${width}px`);
    }

    function save(width: number | null) {
      paint(width);
      if (width === layout[side]) return;
      layout[side] = width;
      remeasure();
      void invoke("save_workspace", { layout: { ...layout } }).catch(report);
    }

    function bound(width: number) {
      return Math.max(minimumWidth, Math.min(maximumWidth, Math.round(width)));
    }

    paint(layout[side]);
    new ResizeObserver(() => {
      if (!pane.hidden) divider.setAttribute("aria-valuenow", String(pane.offsetWidth));
    }).observe(pane);

    divider.addEventListener("pointerdown", event => {
      if (!event.isPrimary || event.button !== 0) return;
      event.preventDefault();
      drag = { pointer: event.pointerId, start: event.clientX, width: pane.offsetWidth, next: pane.offsetWidth };
      divider.setPointerCapture(event.pointerId);
      workspace.classList.add("resizing-pane");
    });
    divider.addEventListener("pointermove", event => {
      if (drag?.pointer !== event.pointerId) return;
      drag.next = bound(drag.width + direction * (event.clientX - drag.start));
      if (!frame) frame = requestAnimationFrame(() => {
        frame = 0;
        if (drag) paint(drag.next);
      });
    });
    divider.addEventListener("pointerup", event => {
      if (drag?.pointer !== event.pointerId) return;
      const start = drag.width;
      paint(bound(start + direction * (event.clientX - drag.start)));
      const width = pane.offsetWidth;
      finish();
      if (width === start) paint(layout[side]);
      else save(bound(width));
    });
    divider.addEventListener("lostpointercapture", () => {
      if (!drag) return;
      finish();
      paint(layout[side]);
      remeasure();
    });

    function finish() {
      drag = undefined;
      cancelAnimationFrame(frame);
      frame = 0;
      workspace.classList.remove("resizing-pane");
    }

    const reset = () => save(null);

    divider.addEventListener("dblclick", reset);
    divider.addEventListener("keydown", event => {
      if (event.key === "Home") {
        event.preventDefault();
        reset();
        return;
      }
      if (!["ArrowLeft", "ArrowRight"].includes(event.key)) return;
      event.preventDefault();
      if (event.repeat) return;
      const start = pane.offsetWidth;
      const step = event.key === "ArrowRight" ? 16 : -16;
      paint(bound(start + step * direction));
      if (pane.offsetWidth === start) paint(layout[side]);
      else save(bound(pane.offsetWidth));
    });
  }
}
