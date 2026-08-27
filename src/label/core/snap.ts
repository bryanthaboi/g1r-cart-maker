// Alignment, distribution and edge snapping. All maths in label space; no DOM.

import { boundsOfPoints, unionOf, type Rect } from "./geometry";

export type Axis = "x" | "y";

export interface Guide {
  axis: Axis;
  /** Label-space coordinate of the guide line. */
  at: number;
  /** Extent of the highlight along the other axis. */
  from: number;
  to: number;
}

export interface SnapResult {
  dx: number;
  dy: number;
  guides: Guide[];
}

export interface SnapOptions {
  threshold: number;
  canvas: Rect | null;
  targets: readonly Rect[];
}

function edgesOf(rect: Rect, axis: Axis): number[] {
  return axis === "x"
    ? [rect.x, rect.x + rect.width / 2, rect.x + rect.width]
    : [rect.y, rect.y + rect.height / 2, rect.y + rect.height];
}

function spanOf(rect: Rect, axis: Axis): [number, number] {
  return axis === "x" ? [rect.y, rect.y + rect.height] : [rect.x, rect.x + rect.width];
}

function snapAxis(moving: Rect, axis: Axis, options: SnapOptions): { delta: number; guides: Guide[] } {
  const candidates: Rect[] = [...options.targets];
  if (options.canvas) candidates.push(options.canvas);
  let best: number | null = null;
  let bestDistance = options.threshold;
  const hits: { at: number; other: Rect }[] = [];

  for (const candidate of candidates) {
    for (const target of edgesOf(candidate, axis)) {
      for (const edge of edgesOf(moving, axis)) {
        const distance = Math.abs(target - edge);
        if (distance > options.threshold) continue;
        const delta = target - edge;
        if (best === null || distance < bestDistance - 0.0001) {
          best = delta;
          bestDistance = distance;
          hits.length = 0;
          hits.push({ at: target, other: candidate });
        } else if (Math.abs(distance - bestDistance) <= 0.0001 && Math.abs(delta - (best ?? 0)) < 0.0001) {
          hits.push({ at: target, other: candidate });
        }
      }
    }
  }

  if (best === null) return { delta: 0, guides: [] };
  const moved: Rect =
    axis === "x" ? { ...moving, x: moving.x + best } : { ...moving, y: moving.y + best };
  const guides = hits.map((hit) => {
    const [a1, a2] = spanOf(moved, axis);
    const [b1, b2] = spanOf(hit.other, axis);
    return { axis, at: hit.at, from: Math.min(a1, b1), to: Math.max(a2, b2) };
  });
  return { delta: best, guides: dedupeGuides(guides) };
}

function dedupeGuides(guides: readonly Guide[]): Guide[] {
  const seen = new Map<string, Guide>();
  for (const guide of guides) {
    const key = `${guide.axis}:${guide.at.toFixed(3)}`;
    const existing = seen.get(key);
    if (existing) {
      existing.from = Math.min(existing.from, guide.from);
      existing.to = Math.max(existing.to, guide.to);
    } else {
      seen.set(key, { ...guide });
    }
  }
  return [...seen.values()];
}

/** Nudge a moving box onto nearby edges and centres, returning the correction plus guides. */
export function snapMove(moving: Rect, options: SnapOptions): SnapResult {
  const horizontal = snapAxis(moving, "x", options);
  const vertical = snapAxis(moving, "y", options);
  return {
    dx: horizontal.delta,
    dy: vertical.delta,
    guides: [...horizontal.guides, ...vertical.guides],
  };
}

export type AlignMode = "left" | "hcentre" | "right" | "top" | "vmiddle" | "bottom";
export type DistributeMode = "horizontal" | "vertical";

/** Align boxes against `bounds`; pass the canvas rect to align against the canvas. */
export function alignRects(rects: readonly Rect[], mode: AlignMode, bounds: Rect): Rect[] {
  return rects.map((rect) => {
    switch (mode) {
      case "left":
        return { ...rect, x: bounds.x };
      case "hcentre":
        return { ...rect, x: bounds.x + (bounds.width - rect.width) / 2 };
      case "right":
        return { ...rect, x: bounds.x + bounds.width - rect.width };
      case "top":
        return { ...rect, y: bounds.y };
      case "vmiddle":
        return { ...rect, y: bounds.y + (bounds.height - rect.height) / 2 };
      case "bottom":
        return { ...rect, y: bounds.y + bounds.height - rect.height };
      default: {
        const exhaustive: never = mode;
        throw new Error(`unhandled align mode ${String(exhaustive)}`);
      }
    }
  });
}

/** Even gaps between the outermost two boxes; fewer than three boxes is a no-op. */
export function distributeRects(rects: readonly Rect[], mode: DistributeMode): Rect[] {
  if (rects.length < 3) return rects.map((rect) => ({ ...rect }));
  const axis: Axis = mode === "horizontal" ? "x" : "y";
  const sizeKey = mode === "horizontal" ? "width" : "height";
  const order = rects
    .map((rect, index) => ({ rect, index }))
    .sort((a, b) => a.rect[axis] - b.rect[axis]);
  const first = order[0];
  const last = order[order.length - 1];
  if (!first || !last) return rects.map((rect) => ({ ...rect }));
  const span = last.rect[axis] + last.rect[sizeKey] - first.rect[axis];
  const used = order.reduce((total, entry) => total + entry.rect[sizeKey], 0);
  const gap = (span - used) / (order.length - 1);
  const output = rects.map((rect) => ({ ...rect }));
  let cursor = first.rect[axis];
  for (const entry of order) {
    const target = output[entry.index];
    if (!target) continue;
    target[axis] = cursor;
    cursor += entry.rect[sizeKey] + gap;
  }
  return output;
}

export function boundsOfRects(rects: readonly Rect[]): Rect {
  if (rects.length === 0) return { x: 0, y: 0, width: 0, height: 0 };
  return unionOf(rects);
}

export function boundsOfCorners(points: readonly { x: number; y: number }[]): Rect {
  return boundsOfPoints(points);
}

/** Arrow-key nudge: one label pixel, ten with shift. */
export function nudgeDelta(key: string, shift: boolean): { dx: number; dy: number } | null {
  const step = shift ? 10 : 1;
  switch (key) {
    case "ArrowLeft":
      return { dx: -step, dy: 0 };
    case "ArrowRight":
      return { dx: step, dy: 0 };
    case "ArrowUp":
      return { dx: 0, dy: -step };
    case "ArrowDown":
      return { dx: 0, dy: step };
    default:
      return null;
  }
}
