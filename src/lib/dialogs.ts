// Native pickers. In a plain browser dev session there is no native layer, so
// the same functions fall back to a prompt rather than silently doing nothing.

import { open, save } from "@tauri-apps/plugin-dialog";
import { USING_FIXTURES } from "./backend";

export interface SaveOptions {
  title: string;
  defaultPath: string;
  extension: string;
  extensionName: string;
}

export async function pickDirectory(title: string, defaultPath?: string): Promise<string | null> {
  if (USING_FIXTURES) return promptPath(`${title}\n\nEnter a folder path.`, defaultPath ?? "/Users/dev/Carts");
  const chosen = await open({ directory: true, multiple: false, title, defaultPath });
  if (typeof chosen === "string") return chosen;
  return null;
}

export async function pickSavePath(options: SaveOptions): Promise<string | null> {
  if (USING_FIXTURES) return promptPath(`${options.title}\n\nEnter a destination path.`, options.defaultPath);
  const chosen = await save({
    title: options.title,
    defaultPath: options.defaultPath,
    filters: [{ name: options.extensionName, extensions: [options.extension] }],
  });
  return chosen ?? null;
}

function promptPath(message: string, initial: string): string | null {
  const answer = window.prompt(message, initial);
  if (answer === null) return null;
  const trimmed = answer.trim();
  return trimmed.length > 0 ? trimmed : null;
}

export async function copyToClipboard(text: string): Promise<boolean> {
  try {
    if (navigator.clipboard && window.isSecureContext) {
      await navigator.clipboard.writeText(text);
      return true;
    }
  } catch {
    // Fall through to the selection-based path below.
  }
  try {
    const holder = document.createElement("textarea");
    holder.value = text;
    holder.setAttribute("readonly", "");
    holder.style.position = "fixed";
    holder.style.opacity = "0";
    document.body.appendChild(holder);
    holder.select();
    const ok = document.execCommand("copy");
    document.body.removeChild(holder);
    return ok;
  } catch {
    return false;
  }
}

/** Paths dropped onto the window arrive differently in Tauri and in a browser. */
export function droppedPaths(event: DragEvent): string[] {
  const files = event.dataTransfer?.files;
  if (!files) return [];
  const out: string[] = [];
  for (let index = 0; index < files.length; index += 1) {
    const file = files.item(index);
    if (!file) continue;
    const withPath = file as File & { path?: string };
    out.push(typeof withPath.path === "string" && withPath.path.length > 0 ? withPath.path : file.name);
  }
  return out;
}
