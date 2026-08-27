import { describe, expect, it } from "vitest";
import {
  boundsOf,
  containsPoint,
  fitPlacement,
  handlePoints,
  resizeRect,
  rotatePoint,
  unionOf,
  type Rect,
} from "./geometry";

const box = { width: 200, height: 100 };

describe("fit modes", () => {
  it("contains a wide image inside the box, letterboxed", () => {
    const placement = fitPlacement("contain", box, { width: 400, height: 100 });
    expect(placement.dest).toEqual({ x: 0, y: 25, width: 200, height: 50 });
    expect(placement.src).toBeNull();
    expect(placement.clip).toBe(false);
  });

  it("covers the box and clips the overflow", () => {
    const placement = fitPlacement("cover", box, { width: 400, height: 100 });
    expect(placement.dest).toEqual({ x: -100, y: 0, width: 400, height: 100 });
    expect(placement.clip).toBe(true);
  });

  it("crops to a centred source rectangle instead of overflowing", () => {
    const placement = fitPlacement("crop", box, { width: 400, height: 400 });
    expect(placement.dest).toEqual({ x: 0, y: 0, width: 200, height: 100 });
    expect(placement.src).toEqual({ x: 0, y: 100, width: 400, height: 200 });
  });

  it("scale keeps native pixels and centres them", () => {
    const placement = fitPlacement("scale", box, { width: 50, height: 50 });
    expect(placement.dest).toEqual({ x: 75, y: 25, width: 50, height: 50 });
    expect(placement.clip).toBe(true);
  });

  it("stretch fills the box and ignores the aspect ratio", () => {
    const placement = fitPlacement("stretch", box, { width: 4, height: 400 });
    expect(placement.dest).toEqual({ x: 0, y: 0, width: 200, height: 100 });
  });

  it("degrades to the box when the bitmap has no size yet", () => {
    const placement = fitPlacement("contain", box, { width: 0, height: 0 });
    expect(placement.dest).toEqual({ x: 0, y: 0, width: 200, height: 100 });
  });
});

describe("rects and rotation", () => {
  const rect: Rect = { x: 10, y: 20, width: 100, height: 50 };

  it("rotates a point about an origin", () => {
    const point = rotatePoint({ x: 1, y: 0 }, { x: 0, y: 0 }, 90);
    expect(point.x).toBeCloseTo(0);
    expect(point.y).toBeCloseTo(1);
  });

  it("bounds a rotated rect by its corners", () => {
    const bounds = boundsOf(rect, 90);
    expect(bounds.width).toBeCloseTo(50);
    expect(bounds.height).toBeCloseTo(100);
    expect(bounds.x).toBeCloseTo(35);
    expect(bounds.y).toBeCloseTo(-5);
  });

  it("hit tests inside a rotated rect in its own frame", () => {
    expect(containsPoint(rect, 0, { x: 15, y: 25 })).toBe(true);
    expect(containsPoint(rect, 0, { x: 5, y: 25 })).toBe(false);
    expect(containsPoint(rect, 90, { x: 60, y: 30 })).toBe(true);
    expect(containsPoint(rect, 90, { x: 5, y: 45 })).toBe(false);
  });

  it("places eight handles, rotated with the layer", () => {
    const points = handlePoints(rect, 0);
    expect(points.nw).toEqual({ x: 10, y: 20 });
    expect(points.se).toEqual({ x: 110, y: 70 });
    const turned = handlePoints(rect, 180);
    expect(turned.nw.x).toBeCloseTo(110);
    expect(turned.nw.y).toBeCloseTo(70);
  });

  it("unions rects", () => {
    expect(unionOf([rect, { x: 0, y: 0, width: 10, height: 10 }])).toEqual({
      x: 0,
      y: 0,
      width: 110,
      height: 70,
    });
  });
});

describe("resize", () => {
  const rect: Rect = { x: 0, y: 0, width: 100, height: 100 };

  it("drags the south east handle and keeps the origin", () => {
    expect(resizeRect(rect, "se", { x: 20, y: 10 })).toEqual({ x: 0, y: 0, width: 120, height: 110 });
  });

  it("drags the north west handle and moves the origin", () => {
    expect(resizeRect(rect, "nw", { x: 20, y: 20 })).toEqual({ x: 20, y: 20, width: 80, height: 80 });
  });

  it("only touches one axis for an edge handle", () => {
    expect(resizeRect(rect, "e", { x: -30, y: 99 })).toEqual({ x: 0, y: 0, width: 70, height: 100 });
  });

  it("keeps the aspect ratio when asked", () => {
    const result = resizeRect(
      { x: 0, y: 0, width: 200, height: 100 },
      "se",
      { x: 100, y: 0 },
      { keepAspect: true, minSize: 4 },
    );
    expect(result.width).toBeCloseTo(300);
    expect(result.height).toBeCloseTo(150);
  });

  it("never collapses below the minimum size", () => {
    const result = resizeRect(rect, "nw", { x: 400, y: 400 }, { keepAspect: false, minSize: 8 });
    expect(result.width).toBe(8);
    expect(result.height).toBe(8);
    expect(result.x).toBe(92);
  });
});
