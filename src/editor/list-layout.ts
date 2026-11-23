import type { AdapterEffect } from "@jot-md/editor/adapter";
import type { MarkdownEditor } from "@jot-md/editor";

type ListNode = NonNullable<AdapterEffect["renderPatch"]>["operations"][number]["inserted"][number];

// Follow the engine's projected list markers. Retain the column while this note is
// open so scrolling a wide number out of view never moves the text back.
export function trackListMarkers(editor: MarkdownEditor, root: HTMLElement, remeasure: () => void) {
  let widest = 0;
  let widestTask = 0;
  function visit(node: ListNode) {
    if (node.kind !== "list-item" && node.kind !== "block-quote") return;
    const children = node.content.type === "structured" ? node.content.children : [];
    const number = node.attributes.find(attribute => attribute.name === "number")?.value;
    if (number?.type === "number") {
      const columns = String(number.value).length + 1;
      widest = Math.max(widest, columns);
      const task = children.some(child => child.kind === "task-marker" || (child.kind === "paragraph" &&
        child.content.type === "structured" && child.content.children.some(inline => inline.kind === "task-marker")));
      if (task) widestTask = Math.max(widestTask, columns);
    }
    children.forEach(visit);
  }
  editor.subscribe(({ renderPatch }) => {
    if (!renderPatch) return;
    const previous = widest, previousTask = widestTask;
    for (const operation of renderPatch.operations) operation.inserted.forEach(visit);
    if (widest === previous && widestTask === previousTask) return;
    root.style.setProperty("--jot-ordered-width", `calc(${widest}ch + var(--jot-list-marker-gap))`);
    if (widestTask) root.style.setProperty("--jot-ordered-task-width",
      `calc(${widestTask}ch + 2 * var(--jot-list-marker-gap) + var(--jot-task-box-size))`);
    remeasure();
  });
}
