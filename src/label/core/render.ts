// The canvas engine. One function draws a document; the editor, the cart preview and
// the PNG export all call it, so what you see is what is written.

import type { LabelDoc, Layer } from "../../lib/types";
import { fitPlacement, type Rect } from "./geometry";
import type { ImageLayer, RectLayer, TextLayer } from "./doc";

export interface Bitmap {
  image: CanvasImageSource;
  width: number;
  height: number;
}

export type ImageResolver = (source: string) => Bitmap | null;

export interface DrawOptions {
  /** Layers to leave out, for drag previews. */
  skip?: ReadonlySet<string>;
  /** Draw hidden layers too, for the layer thumbnail strip. */
  includeHidden?: boolean;
}

export function fontStringFor(layer: TextLayer): string {
  const weight = layer.weight && layer.weight.length > 0 ? layer.weight : "400";
  return `${weight} ${layer.size}px ${layer.font}`;
}

export function textLines(layer: TextLayer): string[] {
  return layer.text.split("\n");
}

export function lineHeightOf(layer: TextLayer): number {
  const factor = layer.line_height ?? 1.2;
  return layer.size * (factor > 0 ? factor : 1.2);
}

function measureWithSpacing(ctx: CanvasRenderingContext2D, text: string, spacing: number): number {
  if (spacing === 0) return ctx.measureText(text).width;
  let total = 0;
  for (const character of text) total += ctx.measureText(character).width + spacing;
  return total > 0 ? total - spacing : 0;
}

function drawSpacedText(
  ctx: CanvasRenderingContext2D,
  text: string,
  x: number,
  y: number,
  spacing: number,
  stroke: boolean,
): void {
  if (spacing === 0) {
    if (stroke) ctx.strokeText(text, x, y);
    else ctx.fillText(text, x, y);
    return;
  }
  let cursor = x;
  for (const character of text) {
    if (stroke) ctx.strokeText(character, cursor, y);
    else ctx.fillText(character, cursor, y);
    cursor += ctx.measureText(character).width + spacing;
  }
}

export function roundedRectPath(ctx: CanvasRenderingContext2D, rect: Rect, radius: number): void {
  const limit = Math.max(0, Math.min(radius, rect.width / 2, rect.height / 2));
  const { x, y, width, height } = rect;
  ctx.beginPath();
  ctx.moveTo(x + limit, y);
  ctx.lineTo(x + width - limit, y);
  ctx.quadraticCurveTo(x + width, y, x + width, y + limit);
  ctx.lineTo(x + width, y + height - limit);
  ctx.quadraticCurveTo(x + width, y + height, x + width - limit, y + height);
  ctx.lineTo(x + limit, y + height);
  ctx.quadraticCurveTo(x, y + height, x, y + height - limit);
  ctx.lineTo(x, y + limit);
  ctx.quadraticCurveTo(x, y, x + limit, y);
  ctx.closePath();
}

function drawImageLayer(ctx: CanvasRenderingContext2D, layer: ImageLayer, resolve: ImageResolver): void {
  const bitmap = resolve(layer.source);
  if (!bitmap) return;
  const placement = fitPlacement(
    layer.fit,
    { width: layer.width, height: layer.height },
    { width: bitmap.width, height: bitmap.height },
  );
  ctx.save();
  ctx.globalAlpha = ctx.globalAlpha * (layer.opacity ?? 1);
  if (placement.clip) {
    ctx.beginPath();
    ctx.rect(0, 0, layer.width, layer.height);
    ctx.clip();
  }
  if (placement.src) {
    ctx.drawImage(
      bitmap.image,
      placement.src.x,
      placement.src.y,
      placement.src.width,
      placement.src.height,
      placement.dest.x,
      placement.dest.y,
      placement.dest.width,
      placement.dest.height,
    );
  } else {
    ctx.drawImage(
      bitmap.image,
      placement.dest.x,
      placement.dest.y,
      placement.dest.width,
      placement.dest.height,
    );
  }
  ctx.restore();
}

function drawRectLayer(ctx: CanvasRenderingContext2D, layer: RectLayer): void {
  const rect: Rect = { x: 0, y: 0, width: layer.width, height: layer.height };
  roundedRectPath(ctx, rect, layer.radius ?? 0);
  ctx.fillStyle = layer.fill;
  ctx.fill();
  const width = layer.stroke_width ?? 0;
  if (layer.stroke && width > 0) {
    ctx.lineWidth = width;
    ctx.strokeStyle = layer.stroke;
    ctx.stroke();
  }
}

function drawTextLayer(ctx: CanvasRenderingContext2D, layer: TextLayer): void {
  const lines = textLines(layer);
  const spacing = layer.letter_spacing ?? 0;
  const step = lineHeightOf(layer);
  ctx.font = fontStringFor(layer);
  ctx.textBaseline = "middle";
  ctx.textAlign = "left";
  const blockHeight = step * lines.length;
  const top = (layer.height - blockHeight) / 2;
  const strokeWidth = layer.stroke_width ?? 0;

  lines.forEach((line, index) => {
    const width = measureWithSpacing(ctx, line, spacing);
    const y = top + step * index + step / 2;
    let x = 0;
    if (layer.align === "center") x = (layer.width - width) / 2;
    if (layer.align === "right") x = layer.width - width;
    if (layer.stroke && strokeWidth > 0) {
      ctx.lineWidth = strokeWidth;
      ctx.strokeStyle = layer.stroke;
      ctx.lineJoin = "round";
      ctx.miterLimit = 2;
      drawSpacedText(ctx, line, x, y, spacing, true);
    }
    ctx.fillStyle = layer.colour;
    drawSpacedText(ctx, line, x, y, spacing, false);
  });
}

export function drawLayer(ctx: CanvasRenderingContext2D, layer: Layer, resolve: ImageResolver): void {
  ctx.save();
  ctx.translate(layer.x + layer.width / 2, layer.y + layer.height / 2);
  if (layer.rotation !== 0) ctx.rotate((layer.rotation * Math.PI) / 180);
  ctx.translate(-layer.width / 2, -layer.height / 2);
  switch (layer.kind) {
    case "image":
      drawImageLayer(ctx, layer, resolve);
      break;
    case "rect":
      drawRectLayer(ctx, layer);
      break;
    case "text":
      drawTextLayer(ctx, layer);
      break;
    default: {
      const exhaustive: never = layer;
      throw new Error(`unhandled layer kind ${String(exhaustive)}`);
    }
  }
  ctx.restore();
}

export function drawDoc(
  ctx: CanvasRenderingContext2D,
  doc: LabelDoc,
  resolve: ImageResolver,
  options: DrawOptions = {},
): void {
  ctx.save();
  ctx.fillStyle = doc.background;
  ctx.fillRect(0, 0, doc.width, doc.height);
  for (const layer of doc.layers) {
    if (layer.hidden && !options.includeHidden) continue;
    if (options.skip?.has(layer.id)) continue;
    drawLayer(ctx, layer, resolve);
  }
  ctx.restore();
}
