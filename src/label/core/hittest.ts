// Selection picking. Hidden layers are never hit; locked layers are hit only when asked for.

import type { Layer } from "../../lib/types";
import { boundsOf, containsPoint, rectOfLayer, type Point, type Rect } from "./geometry";

export interface PickOptions {
  includeLocked: boolean;
}

const DEFAULTS: PickOptions = { includeLocked: false };

export function isPickable(layer: Layer, options: PickOptions = DEFAULTS): boolean {
  if (layer.hidden) return false;
  if (layer.locked && !options.includeLocked) return false;
  return true;
}

/** Topmost first: the last layer in document order draws on top, so it wins. */
export function pickLayer(
  layers: readonly Layer[],
  point: Point,
  options: PickOptions = DEFAULTS,
): Layer | null {
  for (let index = layers.length - 1; index >= 0; index -= 1) {
    const layer = layers[index];
    if (!layer || !isPickable(layer, options)) continue;
    if (containsPoint(rectOfLayer(layer), layer.rotation, point)) return layer;
  }
  return null;
}

/** The next layer under `belowId` at the same point, for alt-click drilling. */
export function pickBelow(
  layers: readonly Layer[],
  point: Point,
  belowId: string,
  options: PickOptions = DEFAULTS,
): Layer | null {
  const start = layers.findIndex((layer) => layer.id === belowId);
  const from = start < 0 ? layers.length - 1 : start - 1;
  for (let index = from; index >= 0; index -= 1) {
    const layer = layers[index];
    if (!layer || !isPickable(layer, options)) continue;
    if (containsPoint(rectOfLayer(layer), layer.rotation, point)) return layer;
  }
  return null;
}

function intersects(a: Rect, b: Rect): boolean {
  return !(a.x + a.width < b.x || b.x + b.width < a.x || a.y + a.height < b.y || b.y + b.height < a.y);
}

export function pickInMarquee(
  layers: readonly Layer[],
  marquee: Rect,
  options: PickOptions = DEFAULTS,
): Layer[] {
  const box = normaliseRect(marquee);
  return layers.filter(
    (layer) => isPickable(layer, options) && intersects(boundsOf(rectOfLayer(layer), layer.rotation), box),
  );
}

export function normaliseRect(rect: Rect): Rect {
  return {
    x: rect.width < 0 ? rect.x + rect.width : rect.x,
    y: rect.height < 0 ? rect.y + rect.height : rect.y,
    width: Math.abs(rect.width),
    height: Math.abs(rect.height),
  };
}
