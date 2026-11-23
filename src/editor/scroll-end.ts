import type { MarkdownEditor } from "@jot-md/editor";

// Live Preview replaces estimated rows as the worker returns Markdown blocks.
// At the very end the user's anchor is the page edge, even if a replacement
// changes the height of the first visible row.
export function retainScrollEnd(view: MarkdownEditor["input"]["view"], signal: AbortSignal) {
  const viewport = view.scrollDOM;
  let end: typeof view.state.doc | undefined;
  let extent = viewport.scrollHeight;
  const release = () => {
    if (end) viewport.removeEventListener("wheel", depart);
    end = undefined;
  };
  viewport.addEventListener("scroll", () => {
    const height = viewport.scrollHeight;
    if (viewport.scrollTop + viewport.clientHeight >= height - 1 && viewport.scrollTop > 0) {
      if (!end) viewport.addEventListener("wheel", depart, { passive: false });
      end = view.state.doc;
    } else if (height === extent) release();
    extent = height;
  }, { passive: true, signal });
  // WebKit's compositor can move before the wheel reaches JS. During the
  // last projection, layout can then restore the old bottom over this motion.
  // Resolve the departure from the anchored edge in the new geometry.
  function depart(event: WheelEvent) {
    if (event.deltaY >= 0 || event.ctrlKey || event.shiftKey) return;
    release();
    if (Math.abs(event.deltaX) > -event.deltaY) return;
    for (const node of event.composedPath()) {
      if (node === viewport) break;
      if (node instanceof HTMLElement && node.scrollTop > 0) return;
    }
    const unit = event.deltaMode === WheelEvent.DOM_DELTA_LINE ? view.defaultLineHeight
      : event.deltaMode === WheelEvent.DOM_DELTA_PAGE ? viewport.clientHeight : 1;
    if (event.cancelable) event.preventDefault();
    viewport.scrollTo({
      top: viewport.scrollHeight - viewport.clientHeight + event.deltaY * unit,
      left: viewport.scrollLeft + event.deltaX * unit,
    });
  }
  viewport.ownerDocument.addEventListener("pointerdown", release, { capture: true, signal });
  viewport.addEventListener("keydown", release, { capture: true, signal });
  const sizes = new ResizeObserver(() => {
    const height = viewport.scrollHeight;
    if (end !== view.state.doc) release();
    else if (viewport.scrollTop + viewport.clientHeight < height - 1) viewport.scrollTop = height;
    extent = height;
  });
  sizes.observe(view.contentDOM);
  signal.addEventListener("abort", () => { release(); sizes.disconnect(); }, { once: true });
}
