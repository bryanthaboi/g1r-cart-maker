// Image import through the native dialog. The dialog returns paths and the bytes
// come back through a command: the asset protocol would need a filesystem scope
// and a wider CSP to reach from the window.

import { open } from "@tauri-apps/plugin-dialog";
import { api } from "../lib/backend";

export interface ImportedImage {
  name: string;
  dataUrl: string;
}

export type PickResult =
  | { ok: true; images: ImportedImage[] }
  | { ok: false; fallback: boolean; error: string };

function baseName(path: string): string {
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 1] ?? path;
}

function messageOf(problem: unknown): string {
  if (problem instanceof Error) return problem.message;
  return String(problem);
}

export async function pickImages(multiple: boolean): Promise<PickResult> {
  let chosen: string | string[] | null;
  try {
    chosen = await open({
      multiple,
      directory: false,
      filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "webp"] }],
    });
  } catch (problem) {
    return { ok: false, fallback: true, error: messageOf(problem) };
  }
  if (chosen === null) return { ok: true, images: [] };
  const paths = Array.isArray(chosen) ? chosen : [chosen];
  const images: ImportedImage[] = [];
  for (const path of paths) {
    try {
      const dataUrl = await api.label.readImage(path);
      images.push({ name: baseName(path), dataUrl });
    } catch (problem) {
      return { ok: false, fallback: true, error: `${baseName(path)}: ${messageOf(problem)}` };
    }
  }
  return { ok: true, images };
}
