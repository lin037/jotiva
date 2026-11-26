import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import type { MarkdownEditorOptions } from "@jot-md/editor";
import { t } from "../i18n";

// PNG animation is declared before the first IDAT. Canvas would flatten it to a single frame.
function animatedPng(bytes: ArrayBuffer) {
  const view = new DataView(bytes);
  for (let offset = 8; offset + 12 <= view.byteLength;) {
    const type = view.getUint32(offset + 4);
    if (type === 0x6163544c) return true; // acTL
    if (type === 0x49444154) return false; // IDAT
    offset += 12 + view.getUint32(offset);
  }
  return true;
}

async function compressImage(file: File, bytes: ArrayBuffer) {
  const header = new DataView(bytes);
  if (header.byteLength < 8) return bytes;
  const jpeg = header.getUint16(0) === 0xffd8 && header.getUint8(2) === 0xff;
  const png = header.getUint32(0) === 0x89504e47 && header.getUint32(4) === 0x0d0a1a0a;
  if (!jpeg && (!png || animatedPng(bytes))) return bytes;
  const image = new Image();
  const src = URL.createObjectURL(file);
  try {
    image.src = src;
    await image.decode();
    if (image.naturalWidth * image.naturalHeight > 32_000_000) return bytes;
    const canvas = document.createElement("canvas");
    canvas.width = image.naturalWidth;
    canvas.height = image.naturalHeight;
    const context = canvas.getContext("2d");
    if (!context) return bytes;
    context.drawImage(image, 0, 0);
    const encoded = await new Promise<Blob | null>(resolve => canvas.toBlob(resolve, jpeg ? "image/jpeg" : "image/webp", 0.84));
    // WebKit versions without WebP encoding may return a PNG here.
    return encoded && encoded.size < file.size ? await encoded.arrayBuffer() : bytes;
  } finally {
    URL.revokeObjectURL(src);
  }
}

export function noteImages(id: number): Pick<MarkdownEditorOptions, "saveImage" | "resolveImageSrc"> {
  return {
    async saveImage(file, { signal }) {
      if (file.size > 32 * 1024 * 1024) throw new Error(t("图片超过 32 MiB"));
      const { compress, revision, mode } = await invoke<{compress: boolean; revision: number; mode: string}>("image_options", { id });
      const original = await file.arrayBuffer();
      const bytes = compress ? await compressImage(file, original) : original;
      signal.throwIfAborted();
      const destination = await invoke<string>("store_note_image", bytes, { headers: {
        "x-jot-mode": mode, "x-jot-note": String(id), "x-jot-name": encodeURIComponent(file.name), "x-jot-revision": String(revision),
      } });
      return { destination, alt: file.name.replace(/\.[^.]+$/, "") };
    },
    resolveImageSrc(destination) {
      if (/^(?:https?:\/\/|data:image\/|blob:)/iu.test(destination)) return destination;
      if (/^(?:[a-z][a-z0-9+.-]*:|\/\/|\\\\)/iu.test(destination) && !destination.startsWith("file:///")) return undefined;
      return convertFileSrc(id + "/" + destination);
    },
  };
}
