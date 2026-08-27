// Rendering the document to a PNG data URL, at the native size or a whole multiple.

import type { LabelDoc } from "../../lib/types";
import type { ExportSettings } from "./exportGuard";
import { drawDoc, type ImageResolver } from "./render";

export function makeCanvas(width: number, height: number): HTMLCanvasElement {
  const canvas = document.createElement("canvas");
  canvas.width = Math.max(1, Math.round(width));
  canvas.height = Math.max(1, Math.round(height));
  return canvas;
}

export function contextOf(canvas: HTMLCanvasElement): CanvasRenderingContext2D {
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("this system has no 2D canvas context");
  return ctx;
}

/** Posterise to `levels` per channel; fewer distinct colours compress much smaller. */
export function quantizeCanvas(canvas: HTMLCanvasElement, levels: number): void {
  if (levels < 2 || levels >= 256) return;
  const ctx = contextOf(canvas);
  const frame = ctx.getImageData(0, 0, canvas.width, canvas.height);
  const data = frame.data;
  const step = 255 / (levels - 1);
  for (let index = 0; index < data.length; index += 4) {
    data[index] = Math.round(Math.round((data[index] ?? 0) / step) * step);
    data[index + 1] = Math.round(Math.round((data[index + 1] ?? 0) / step) * step);
    data[index + 2] = Math.round(Math.round((data[index + 2] ?? 0) / step) * step);
  }
  ctx.putImageData(frame, 0, 0);
}

export function renderDoc(
  doc: LabelDoc,
  resolve: ImageResolver,
  settings: ExportSettings,
): HTMLCanvasElement {
  const multiple = Math.max(1, Math.round(settings.multiple));
  const canvas = makeCanvas(doc.width * multiple, doc.height * multiple);
  const ctx = contextOf(canvas);
  ctx.imageSmoothingEnabled = true;
  ctx.imageSmoothingQuality = "high";
  ctx.scale(multiple, multiple);
  drawDoc(ctx, doc, resolve);
  if (settings.quantize !== null) quantizeCanvas(canvas, settings.quantize);
  return canvas;
}

export function exportDataUrl(
  doc: LabelDoc,
  resolve: ImageResolver,
  settings: ExportSettings,
): string {
  return renderDoc(doc, resolve, settings).toDataURL("image/png");
}
