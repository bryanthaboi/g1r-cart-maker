// Canvas geometry in label space: the document's own pixel grid, never screen pixels.

import type { FitMode, Layer } from "../../lib/types";

export interface Point {
  x: number;
  y: number;
}

export interface Size {
  width: number;
  height: number;
}

export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** Where a bitmap lands inside its layer box: dest in layer space, src in image space. */
export interface Placement {
  dest: Rect;
  src: Rect | null;
  clip: boolean;
}

export type HandleId = "nw" | "n" | "ne" | "e" | "se" | "s" | "sw" | "w";

export const HANDLES: readonly HandleId[] = ["nw", "n", "ne", "e", "se", "s", "sw", "w"];

export function rectOfLayer(layer: Layer): Rect {
  return { x: layer.x, y: layer.y, width: layer.width, height: layer.height };
}

export function centreOf(rect: Rect): Point {
  return { x: rect.x + rect.width / 2, y: rect.y + rect.height / 2 };
}

export function rotatePoint(point: Point, origin: Point, degrees: number): Point {
  if (degrees === 0) return { x: point.x, y: point.y };
  const radians = (degrees * Math.PI) / 180;
  const cos = Math.cos(radians);
  const sin = Math.sin(radians);
  const dx = point.x - origin.x;
  const dy = point.y - origin.y;
  return { x: origin.x + dx * cos - dy * sin, y: origin.y + dx * sin + dy * cos };
}

/** Undo a layer's rotation so a point can be tested against its unrotated box. */
export function toLocalPoint(point: Point, rect: Rect, rotation: number): Point {
  return rotatePoint(point, centreOf(rect), -rotation);
}

export function containsPoint(rect: Rect, rotation: number, point: Point): boolean {
  const local = toLocalPoint(point, rect, rotation);
  return (
    local.x >= rect.x &&
    local.x <= rect.x + rect.width &&
    local.y >= rect.y &&
    local.y <= rect.y + rect.height
  );
}

export function cornersOf(rect: Rect, rotation: number): Point[] {
  const origin = centreOf(rect);
  const raw: Point[] = [
    { x: rect.x, y: rect.y },
    { x: rect.x + rect.width, y: rect.y },
    { x: rect.x + rect.width, y: rect.y + rect.height },
    { x: rect.x, y: rect.y + rect.height },
  ];
  return raw.map((corner) => rotatePoint(corner, origin, rotation));
}

/** Axis-aligned bounding box of a rotated rect; alignment and snapping work on these. */
export function boundsOf(rect: Rect, rotation: number): Rect {
  if (rotation === 0) return { ...rect };
  const points = cornersOf(rect, rotation);
  return boundsOfPoints(points);
}

export function boundsOfPoints(points: readonly Point[]): Rect {
  const first = points[0];
  if (!first) return { x: 0, y: 0, width: 0, height: 0 };
  let minX = first.x;
  let maxX = first.x;
  let minY = first.y;
  let maxY = first.y;
  for (const point of points) {
    minX = Math.min(minX, point.x);
    maxX = Math.max(maxX, point.x);
    minY = Math.min(minY, point.y);
    maxY = Math.max(maxY, point.y);
  }
  return { x: minX, y: minY, width: maxX - minX, height: maxY - minY };
}

export function unionOf(rects: readonly Rect[]): Rect {
  const points: Point[] = [];
  for (const rect of rects) {
    points.push({ x: rect.x, y: rect.y });
    points.push({ x: rect.x + rect.width, y: rect.y + rect.height });
  }
  return boundsOfPoints(points);
}

export function handlePoints(rect: Rect, rotation: number): Record<HandleId, Point> {
  const origin = centreOf(rect);
  const midX = rect.x + rect.width / 2;
  const midY = rect.y + rect.height / 2;
  const raw: Record<HandleId, Point> = {
    nw: { x: rect.x, y: rect.y },
    n: { x: midX, y: rect.y },
    ne: { x: rect.x + rect.width, y: rect.y },
    e: { x: rect.x + rect.width, y: midY },
    se: { x: rect.x + rect.width, y: rect.y + rect.height },
    s: { x: midX, y: rect.y + rect.height },
    sw: { x: rect.x, y: rect.y + rect.height },
    w: { x: rect.x, y: midY },
  };
  for (const id of HANDLES) {
    raw[id] = rotatePoint(raw[id], origin, rotation);
  }
  return raw;
}

export interface ResizeOptions {
  keepAspect: boolean;
  minSize: number;
}

/** Resize by dragging one handle; the opposite edge stays put. Deltas arrive in layer space. */
export function resizeRect(
  rect: Rect,
  handle: HandleId,
  delta: Point,
  options: ResizeOptions = { keepAspect: false, minSize: 4 },
): Rect {
  const min = Math.max(1, options.minSize);
  let { x, y, width, height } = rect;
  const west = handle === "nw" || handle === "w" || handle === "sw";
  const east = handle === "ne" || handle === "e" || handle === "se";
  const north = handle === "nw" || handle === "n" || handle === "ne";
  const south = handle === "sw" || handle === "s" || handle === "se";

  if (east) width = rect.width + delta.x;
  if (west) {
    width = rect.width - delta.x;
    x = rect.x + delta.x;
  }
  if (south) height = rect.height + delta.y;
  if (north) {
    height = rect.height - delta.y;
    y = rect.y + delta.y;
  }

  if (options.keepAspect && rect.width > 0 && rect.height > 0 && (east || west) && (north || south)) {
    const aspect = rect.width / rect.height;
    if (Math.abs(width - rect.width) >= Math.abs(height - rect.height)) {
      const next = width / aspect;
      if (north) y = y + (height - next);
      height = next;
    } else {
      const next = height * aspect;
      if (west) x = x + (width - next);
      width = next;
    }
  }

  if (width < min) {
    if (west) x = rect.x + rect.width - min;
    width = min;
  }
  if (height < min) {
    if (north) y = rect.y + rect.height - min;
    height = min;
  }
  return { x, y, width, height };
}

/**
 * Where a bitmap of `natural` size sits inside a `box`-sized layer, per fit mode.
 * contain/cover/scale centre the image; crop expresses cover as a source rectangle.
 */
export function fitPlacement(mode: FitMode, box: Size, natural: Size): Placement {
  const bw = Math.max(0, box.width);
  const bh = Math.max(0, box.height);
  const nw = natural.width;
  const nh = natural.height;
  if (nw <= 0 || nh <= 0 || bw <= 0 || bh <= 0) {
    return { dest: { x: 0, y: 0, width: bw, height: bh }, src: null, clip: false };
  }
  switch (mode) {
    case "stretch":
      return { dest: { x: 0, y: 0, width: bw, height: bh }, src: null, clip: false };
    case "contain": {
      const scale = Math.min(bw / nw, bh / nh);
      const width = nw * scale;
      const height = nh * scale;
      return {
        dest: { x: (bw - width) / 2, y: (bh - height) / 2, width, height },
        src: null,
        clip: false,
      };
    }
    case "cover": {
      const scale = Math.max(bw / nw, bh / nh);
      const width = nw * scale;
      const height = nh * scale;
      return {
        dest: { x: (bw - width) / 2, y: (bh - height) / 2, width, height },
        src: null,
        clip: true,
      };
    }
    case "scale": {
      return {
        dest: { x: (bw - nw) / 2, y: (bh - nh) / 2, width: nw, height: nh },
        src: null,
        clip: true,
      };
    }
    case "crop": {
      const scale = Math.max(bw / nw, bh / nh);
      const sw = Math.min(nw, bw / scale);
      const sh = Math.min(nh, bh / scale);
      return {
        dest: { x: 0, y: 0, width: bw, height: bh },
        src: { x: (nw - sw) / 2, y: (nh - sh) / 2, width: sw, height: sh },
        clip: false,
      };
    }
    default: {
      const exhaustive: never = mode;
      throw new Error(`unhandled fit mode ${String(exhaustive)}`);
    }
  }
}

export function clamp(value: number, low: number, high: number): number {
  return Math.min(high, Math.max(low, value));
}

export function roundTo(value: number, places = 2): number {
  const factor = 10 ** places;
  return Math.round(value * factor) / factor;
}
