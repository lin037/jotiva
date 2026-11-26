import { invoke } from "@tauri-apps/api/core";

export const events = {
  quickView: "desktop:quick-view",
  mainNote: "desktop:main-note",
  notesChanged: "desktop:notes-changed",
  shortcutRecorded: "desktop:shortcut-recorded",
  foldersChanged: "desktop:folders-changed",
  appearanceChanged: "desktop:appearance-changed",
  imageSettingsChanged: "desktop:image-settings-changed",
  settingsSection: "desktop:settings-section",
  sidebarToggle: "desktop:sidebar-toggle",
  editorAction: "desktop:editor-action",
} as const;

export type NoteId = number;

export const openLibraryFile = (path: string) => invoke<NoteId>("open_library_file", { path });

export interface Card {
  id: NoteId;
  version: number;
  title: string;
  excerpt: string;
  source: string;
  attached: boolean;
  folder: number | null;
  archivePath?: string;
  file?: string;
}

export interface Note {
  id: NoteId;
  fileUnavailable?: boolean;
  filingTicket?: number;
  source: string;
  markdown: string;
  version: number;
  location?: { folder: number | null; path: string };
  file?: string;
}

export interface NotePage {
  items: Card[];
  more: boolean;
}

export interface SearchPage<Item = Card> {
  current: Item[];
  attached: Item[];
  documents: Item[];
  more: boolean;
}

export interface EditorCursor {
  anchor: number;
  head: number;
  top: number;
  mode: import("@jot-md/editor").MarkdownEditor["mode"];
}

export type QuickView = { epoch: number } & (
  | { kind: "hidden" }
  | { kind: "summary"; note: Card; docked: boolean; rail: (NotePage & { host: number }) | null }
  | { kind: "edit"; id: NoteId; pinned: boolean; standalone: boolean; source: string | null; cursor: EditorCursor | null }
  | { kind: "search"; tiled: boolean; host: number | null }
);

export type Scope =
  | { kind: "folder"; id: number | null }
  | { kind: "source"; key: string }
  | { kind: "host"; id: number };

export interface Folder {
  id: number;
  name: string;
  description: string;
  path: string;
  classify: boolean;
}

export interface SourceGroup {
  app: string;
  windows: { key: string; title: string; attached: boolean }[];
}

export interface SourceChoice {
  choice: number;
  app: string;
  title: string;
}

export const folders = {
  list: () => invoke<Folder[]>("list_folders"),
  inbox: () => invoke<string>("inbox_directory"),
  choose: () => invoke<string | null>("choose_folder"),
  save: (input: Omit<Folder, "id"> & { id: number | null }) => invoke<Folder>("save_folder", { input }),
  move: (id: NoteId, folder: number | null) => invoke<{ path: string | null; warning: string | null }>("move_note", { id, folder }),
};

export type ImageMode = "relative" | "fixed" | "http";
export interface ImageRule { path: string; mode: ImageMode; subfolder: string; directory: string }
export interface ImageUpload {
  endpoint: string;
  field: string;
  responsePointer: string;
  bearer: boolean;
}
export interface ImageConfig extends ImageUpload {
  defaultMode: ImageMode;
  rules: ImageRule[];
  subfolder: string;
  directory: string;
  groupByDocument: boolean;
  compress: boolean;
}
interface ImageFolder {
  path: string;
  effectiveRule: ImageRule | null;
  inheritedRule: ImageRule | null;
}
interface ImageSettings {
  config: ImageConfig;
  revision: number;
  folder: ImageFolder | null;
}

export const imageSettings = {
  read: (folder?: string) => invoke<ImageSettings>("read_image_settings", { folder }),
  save: (config: ImageConfig, revision: number) => invoke<ImageSettings>("save_image_settings", { config, revision }),
  saveFolder: (folder: string, rule: ImageRule | null, upload: ImageUpload | null, revision: number) =>
    invoke<void>("save_image_folder", { folder, rule, upload, revision }),
  key: (address: string, action: "check" | "set" | "remove", folder?: string) =>
    invoke<boolean>("image_key", { address, action, folder }),
};

export const sources = {
  list: () => invoke<SourceGroup[]>("list_sources"),
  choices: () => invoke<SourceChoice[] | null>("source_choices"),
  add: (choice: number) => invoke<string>("register_source", { choice }),
  clearChoices: () => invoke("clear_source_choices"),
};

export const notes = {
  search: (query: string, host: number | null, page: number) => invoke<SearchPage>("search_notes", { query, host, page }),
  searchFiles: (query: string) => invoke<string[]>("search_document_files", { query }),
  list: (query: string, offset: number, scope: Scope | null = null) => invoke<NotePage>("list_notes", { query, offset, scope }),
  read: (id: NoteId) => invoke<Note>("read_note", { id }),
  card: (id: NoteId) => invoke<Card>("read_card", { id }),
  rename: (id: NoteId, name: string) => invoke("note_action", { id, action: { kind: "rename", name } }),
  create: () => invoke<NoteId>("new_note"),
  save: (id: NoteId, version: number, markdown: string) => invoke<number>("save_note", { id, version, markdown }),
  attach: (id: NoteId, choice: number) => invoke<string>("attach_note", { id, choice }),
  release: (id: NoteId) => invoke("release_note", { id }),
};
