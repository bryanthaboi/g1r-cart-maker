// Screen/document transforms and the anchor rules used while dragging handles.

import type { LabelDoc, Layer } from "../../lib/types";
import {
  boundsOf,
  centreOf,
  rectOfLayer,
  resizeRect,
  rotatePoint,
  type HandleId,
  type Point,
  type Rect,
} from "../core/geometry";

export interface StageView {
  zoom: number;
  offsetX: number;
  offsetY: number;
  showGrid: boolean;
  showRulers: boolean;
  snap: boolean;
}

export const MIN_ZOOM = 0.1;
export const MAX_ZOOM = 16;
export const RULER_SIZE = 18;

export function toScreen(view: StageView, point: Point): Point {
  return { x: point.x * view.zoom + view.offsetX, y: point.y * view.zoom + view.offsetY };
}

export function toDoc(view: StageView, point: Point): Point {
  return { x: (point.x - view.offsetX) / view.zoom, y: (point.y - view.offsetY) / view.zoom };
}

export function fitView(doc: LabelDoc, width: number, height: number, view: StageView): StageView {
  const padding = 48;
  const usable = {
    width: Math.max(32, width - padding * 2),
    height: Math.max(32, height - padding * 2),
  };
  const zoom = Math.min(usable.width / doc.width, usable.height / doc.height);
  const clamped = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, zoom));
  return {
    ...view,
    zoom: clamped,
    offsetX: (width - doc.width * clamped) / 2,
    offsetY: (height - doc.height * clamped) / 2,
  };
}

/** Zoom about a fixed screen point, so the pixel under the cursor stays put. */
export function zoomAt(view: StageView, screen: Point, factor: number): StageView {
  const zoom = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, view.zoom * factor));
  const before = toDoc(view, screen);
  const next: StageView = { ...view, zoom };
  const after = toDoc(next, screen);
  return {
    ...next,
    offsetX: next.offsetX + (after.x - before.x) * zoom,
    offsetY: next.offsetY + (after.y - before.y) * zoom,
  };
}

export function selectionBounds(layers: readonly Layer[]): Rect | null {
  const boxes = layers.map((layer) => boundsOf(rectOfLayer(layer), layer.rotation));
  const first = boxes[0];
  if (!first) return null;
  let minX = first.x;
  let minY = first.y;
  let maxX = first.x + first.width;
  let maxY = first.y + first.height;
  for (const box of boxes) {
    minX = Math.min(minX, box.x);
    minY = Math.min(minY, box.y);
    maxX = Math.max(maxX, box.x + box.width);
    maxY = Math.max(maxY, box.y + box.height);
  }
  return { x: minX, y: minY, width: maxX - minX, height: maxY - minY };
}

/** The point a resize pins in place: the corner or edge opposite the dragged handle. */
export function anchorOf(rect: Rect, handle: HandleId): Point {
  const left = rect.x;
  const right = rect.x + rect.width;
  const top = rect.y;
  const bottom = rect.y + rect.height;
  const midX = rect.x + rect.width / 2;
  const midY = rect.y + rect.height / 2;
  switch (handle) {
    case "nw":
      return { x: right, y: bottom };
    case "n":
      return { x: midX, y: bottom };
    case "ne":
      return { x: left, y: bottom };
    case "e":
      return { x: left, y: midY };
    case "se":
      return { x: left, y: top };
    case "s":
      return { x: midX, y: top };
    case "sw":
      return { x: right, y: top };
    case "w":
      return { x: right, y: midY };
    default: {
      const exhaustive: never = handle;
      throw new Error(`unhandled handle ${String(exhaustive)}`);
    }
  }
}

export interface RotatedResize {
  rect: Rect;
}

/**
 * Resize a rotated layer: the drag runs in the layer's own frame and the anchor
 * point is put back where it was, so the opposite edge does not swing.
 */
export function resizeRotated(
  rect: Rect,
  rotation: number,
  handle: HandleId,
  worldDelta: Point,
  keepAspect: boolean,
): Rect {
  const local = rotatePoint(worldDelta, { x: 0, y: 0 }, -rotation);
  const next = resizeRect(rect, handle, local, { keepAspect, minSize: 4 });
  if (rotation === 0) return next;
  const anchorBefore = rotatePoint(anchorOf(rect, handle), centreOf(rect), rotation);
  const anchorAfter = rotatePoint(anchorOf(next, handle), centreOf(next), rotation);
  return {
    ...next,
    x: next.x + (anchorBefore.x - anchorAfter.x),
    y: next.y + (anchorBefore.y - anchorAfter.y),
  };
}

export function angleBetween(origin: Point, point: Point): number {
  return (Math.atan2(point.y - origin.y, point.x - origin.x) * 180) / Math.PI;
}

export function normaliseAngle(degrees: number): number {
  const wrapped = degrees % 360;
  return wrapped < 0 ? wrapped + 360 : wrapped;
}
