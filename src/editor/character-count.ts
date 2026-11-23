import type { MarkdownEditor } from "@jot-md/editor";

function countCharacters(text: string) {
  let count = 0;
  for (const character of text) {
    if (/\S/u.test(character)) count++;
  }
  return count;
}

export function trackCharacterCount(editor: MarkdownEditor, counter: HTMLElement) {
  let previous = editor.confirmedValue;
  let total = countCharacters(previous);
  const render = () => { counter.textContent = total.toLocaleString() + " 字符"; };
  counter.title = "Markdown 源文的字符数，包含标点，不含空格和换行";
  render();
  return editor.subscribe(({ appliedEdits }) => {
    if (!appliedEdits?.length) return;
    let delta = 0;
    for (const { range, insert } of appliedEdits) {
      delta += countCharacters(insert) - countCharacters(previous.slice(range.from, range.to));
    }
    previous = editor.confirmedValue;
    if (delta) { total += delta; render(); }
  });
}
