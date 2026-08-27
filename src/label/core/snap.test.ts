import { describe, expect, it } from "vitest";
import { alignRects, distributeRects, nudgeDelta, snapMove, type Guide } from "./snap";
import type { Rect } from "./geometry";

const canvas: Rect = { x: 0, y: 0, width: 500, height: 441 };

describe("snapping", () => {
  it("pulls a near edge onto the canvas edge and reports a guide", () => {
    const result = snapMove({ x: 3, y: 250, width: 100, height: 50 }, {
      threshold: 4,
      canvas,
      targets: [],
    });
    expect(result.dx).toBe(-3);
    expect(result.dy).toBe(0);
    expect(result.guides.some((guide: Guide) => guide.axis === "x" && guide.at === 0)).toBe(true);
  });

  it("centres against the canvas centre line", () => {
    const result = snapMove({ x: 198, y: 100, width: 100, height: 50 }, {
      threshold: 6,
      canvas,
      targets: [],
    });
    expect(result.dx).toBe(2);
  });

  it("snaps to another layer's edge", () => {
    const result = snapMove({ x: 98, y: 300, width: 40, height: 40 }, {
      threshold: 6,
      canvas: null,
      targets: [{ x: 100, y: 0, width: 40, height: 40 }],
    });
    expect(result.dx).toBe(2);
  });

  it("stays put when nothing is within the threshold", () => {
    const result = snapMove({ x: 220, y: 250, width: 40, height: 40 }, {
      threshold: 4,
      canvas,
      targets: [],
    });
    expect(result).toEqual({ dx: 0, dy: 0, guides: [] });
  });

  it("prefers the nearest edge when several are in range", () => {
    const result = snapMove({ x: 99, y: 10, width: 40, height: 40 }, {
      threshold: 12,
      canvas: null,
      targets: [
        { x: 105, y: 0, width: 40, height: 40 },
        { x: 100, y: 0, width: 40, height: 40 },
      ],
    });
    expect(result.dx).toBe(1);
  });
});

describe("align and distribute", () => {
  const rects: Rect[] = [
    { x: 10, y: 10, width: 100, height: 20 },
    { x: 200, y: 60, width: 60, height: 40 },
    { x: 400, y: 90, width: 40, height: 10 },
  ];

  it("aligns left, centre and right against a bounds rect", () => {
    expect(alignRects(rects, "left", canvas).map((rect) => rect.x)).toEqual([0, 0, 0]);
    expect(alignRects(rects, "right", canvas).map((rect) => rect.x)).toEqual([400, 440, 460]);
    expect(alignRects(rects, "hcentre", canvas).map((rect) => rect.x)).toEqual([200, 220, 230]);
  });

  it("aligns top, middle and bottom", () => {
    expect(alignRects(rects, "top", canvas).map((rect) => rect.y)).toEqual([0, 0, 0]);
    expect(alignRects(rects, "bottom", canvas).map((rect) => rect.y)).toEqual([421, 401, 431]);
    expect(alignRects(rects, "vmiddle", canvas).map((rect) => rect.y)).toEqual([210.5, 200.5, 215.5]);
  });

  it("spaces the middle boxes evenly and leaves the outer two alone", () => {
    const spread = distributeRects(rects, "horizontal");
    expect(spread[0]?.x).toBe(10);
    expect(spread[2]?.x).toBe(400);
    const gapOne = (spread[1]?.x ?? 0) - ((spread[0]?.x ?? 0) + 100);
    const gapTwo = (spread[2]?.x ?? 0) - ((spread[1]?.x ?? 0) + 60);
    expect(gapOne).toBeCloseTo(gapTwo);
  });

  it("is a no-op below three boxes", () => {
    const two = rects.slice(0, 2);
    expect(distributeRects(two, "horizontal")).toEqual(two);
  });
});

describe("nudging", () => {
  it("moves one pixel, ten with shift", () => {
    expect(nudgeDelta("ArrowLeft", false)).toEqual({ dx: -1, dy: 0 });
    expect(nudgeDelta("ArrowDown", true)).toEqual({ dx: 0, dy: 10 });
    expect(nudgeDelta("Enter", false)).toBeNull();
  });
});
