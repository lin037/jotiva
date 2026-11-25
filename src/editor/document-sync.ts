import { t } from "../i18n";
import type { MarkdownEditor } from "@jot-md/editor";
import { range, singleTextEdit } from "@jot-md/editor/adapter";

/** Rebase an incoming edit onto a source which may have local, unsaved input. */
function incomingChange(base: string, local: string, incoming: string) {
  const [remote] = singleTextEdit(base, incoming);
  let from = Number(remote.range.from);
  let to = Number(remote.range.to);
  const [own] = singleTextEdit(base, local);
  if (own) {
    const start = Number(own.range.from);
    const end = Number(own.range.to);
    if (from === to && start === end && from === start) return null;
    if (end <= from) {
      const shift = own.insert.length - (end - start);
      from += shift;
      to += shift;
    } else if (start < to) {
      return null;
    }
  }
  return { range: range(from, to), insert: remote.insert };
}

export async function applyIncoming(editor: MarkdownEditor, base: string, incoming: string) {
  if (base === incoming) return true;
  const inputMoved = Symbol("queued input changed the source");
  const view = editor.input.view;
  for (;;) {
    await editor.input.settled();
    if (view.composing) return false;
    const before = view.state.doc;
    const current = editor.value;
    if (current === incoming) return true;
    const currentRevision = editor.revision;
    let expected = current;
    try {
      await editor.transact(({ source, revision }) => {
        if (revision !== currentRevision || view.state.doc !== before) throw inputMoved;
        if (source !== current) throw new Error(t("编辑器尚未确认本地输入，请保留编辑并重新加载"));
        const patch = incomingChange(base, source, incoming);
        if (!patch) throw new Error(t("两个窗口修改了同一处内容，可将本窗口的编辑另存为副本"));
        expected = source.slice(0, patch.range.from) + patch.insert + source.slice(patch.range.to);
        return {
          origin: "system",
          history: { type: "ignore" },
          edits: [patch],
          viewport: { visibleRanges: view.visibleRanges.map(({ from, to }) => range(from, to)) },
        };
      });
    } catch (error) {
      if (error === inputMoved) continue;
      throw error;
    }
    if (editor.value === expected) return true;
    // The source host keeps an optimistic keystroke that arrived during the worker request.
    // Wait for that input to commit and rebase the remote change against its confirmed source.
    if (view.state.doc === before) throw new Error(t("编辑器没有应用新版本，请保留编辑并重新加载"));
  }
}
