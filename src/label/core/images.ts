// Bringing bitmaps in. Imported art becomes a data URL so a saved document is
// self-contained and a reopened project restores byte-identical artwork.

import type { Bitmap } from "./render";

export const ACCEPTED_TYPES: readonly string[] = ["image/png", "image/jpeg", "image/webp"];
export const ACCEPT_ATTRIBUTE = ".png,.jpg,.jpeg,.webp,image/png,image/jpeg,image/webp";

export function isAcceptedType(type: string, name = ""): boolean {
  if (ACCEPTED_TYPES.includes(type.toLowerCase())) return true;
  return /\.(png|jpe?g|webp)$/i.test(name);
}

export function extensionType(name: string): string | null {
  const lower = name.toLowerCase();
  if (lower.endsWith(".png")) return "image/png";
  if (lower.endsWith(".jpg") || lower.endsWith(".jpeg")) return "image/jpeg";
  if (lower.endsWith(".webp")) return "image/webp";
  return null;
}

export function blobToDataUrl(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(new Error("could not read the image file"));
    reader.onload = () => {
      const result = reader.result;
      if (typeof result === "string") resolve(result);
      else reject(new Error("the image file did not decode to a data URL"));
    };
    reader.readAsDataURL(blob);
  });
}

export function loadBitmap(source: string): Promise<Bitmap> {
  return new Promise((resolve, reject) => {
    const image = new Image();
    image.onload = () => {
      resolve({ image, width: image.naturalWidth, height: image.naturalHeight });
    };
    image.onerror = () => reject(new Error("the image could not be decoded"));
    image.src = source;
  });
}

export async function fetchAsDataUrl(url: string): Promise<string> {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`could not read ${url} (${response.status})`);
  return blobToDataUrl(await response.blob());
}

/** Decoded bitmaps, keyed by their data URL. Misses trigger a load and a redraw. */
export class BitmapCache {
  private readonly ready = new Map<string, Bitmap>();
  private readonly pending = new Set<string>();
  private readonly failed = new Map<string, string>();

  constructor(private readonly onReady: () => void) {}

  get(source: string): Bitmap | null {
    const found = this.ready.get(source);
    if (found) return found;
    if (!this.pending.has(source) && !this.failed.has(source) && source.length > 0) {
      this.pending.add(source);
      loadBitmap(source)
        .then((bitmap) => {
          this.ready.set(source, bitmap);
          this.pending.delete(source);
          this.onReady();
        })
        .catch((problem: unknown) => {
          this.pending.delete(source);
          this.failed.set(source, problem instanceof Error ? problem.message : String(problem));
          this.onReady();
        });
    }
    return null;
  }

  errorFor(source: string): string | null {
    return this.failed.get(source) ?? null;
  }

  forget(source: string): void {
    this.ready.delete(source);
    this.failed.delete(source);
  }

  resolver(): (source: string) => Bitmap | null {
    return (source: string) => this.get(source);
  }

  /** Wait until every source has been decoded or has failed; used before an export. */
  async warm(sources: readonly string[]): Promise<void> {
    await Promise.all(
      sources.map(async (source) => {
        if (source.length === 0 || this.ready.has(source)) return;
        try {
          this.ready.set(source, await loadBitmap(source));
          this.failed.delete(source);
        } catch (problem) {
          this.failed.set(source, problem instanceof Error ? problem.message : String(problem));
        }
      }),
    );
  }
}
