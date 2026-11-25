import { notes, type Card, type SearchPage } from "./ipc";
import { fileName } from "./platform";

export type SearchHit = Card | {
  id: null;
  file: string;
  title: string;
  attached: false;
  source: "";
  excerpt: "";
  archivePath?: never;
};

export const hitKey = (hit: SearchHit) => hit.id ?? hit.file;

/** One query owns its file snapshot, including late replies after the user changes the query. */
export function documentSearch(query: string, host: number | null) {
  const batchSize = 12;
  let files: Promise<string[]> | undefined;
  let firstFilePage = Infinity;
  let firstFileCount = 0;
  return async (page: number): Promise<SearchPage<SearchHit>> => {
    let result: SearchPage<SearchHit> = { current: [], attached: [], documents: [], more: false };
    if (page <= firstFilePage) {
      result = await notes.search(query, host, page);
      if (result.more) return result;
      firstFilePage = page;
      firstFileCount = Math.max(0, batchSize - result.current.length - result.attached.length - result.documents.length);
    }
    files ??= notes.searchFiles(query).catch(error => { files = undefined; throw error; });
    const paths = await files;
    const first = page === firstFilePage;
    const start = first ? 0 : firstFileCount + (page - firstFilePage - 1) * batchSize;
    const end = start + (first ? firstFileCount : batchSize);
    for (const file of paths.slice(start, end)) {
      result.documents.push({ id: null, file, title: fileName(file), attached: false, source: "", excerpt: "" });
    }
    result.more = paths.length > end;
    return result;
  };
}
