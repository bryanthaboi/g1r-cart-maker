import { describe, expect, it } from "vitest";
import { blankDoc } from "../core/doc";
import { newRectLayer } from "../core/doc";
import { anchorOf, fitView, resizeRotated, selectionBounds, toDoc, toScreen, zoomAt, type StageView } from "./stageMath";

const view: StageView = {
  zoom: 2,
  offsetX: 30,
  offsetY: 10,
  showGrid: false,
  showRulers: false,
  snap: true,
};

describe("stage transforms", () => {
  it("maps document points to the screen and back", () => {
    const screen = toScreen(view, { x: 10, y: 5 });
    expect(screen).toEqual({ x: 50, y: 20 });
    expect(toDoc(view, screen)).toEqual({ x: 10, y: 5 });
  });

  it("fits the document inside the viewport with padding", () => {
    const fitted = fitView(blankDoc(), 596, 537, view);
    expect(fitted.zoom).toBeCloseTo(1);
    expect(fitted.offsetX).toBeCloseTo(48);
    expect(fitted.offsetY).toBeCloseTo(48);
  });

  it("clamps the fit zoom into the allowed range", () => {
    expect(fitView(blankDoc(), 40, 40, view).zoom).toBeCloseTo(0.1);
  });

  it("keeps the point under the cursor fixed while zooming", () => {
    const cursor = { x: 120, y: 80 };
    const before = toDoc(view, cursor);
    const zoomed = zoomAt(view, cursor, 1.5);
    expect(zoomed.zoom).toBeCloseTo(3);
    const after = toDoc(zoomed, cursor);
    expect(after.x).toBeCloseTo(before.x);
    expect(after.y).toBeCloseTo(before.y);
  });
});

describe("resize anchors", () => {
  it("pins the opposite corner or edge", () => {
    const rect = { x: 0, y: 0, width: 100, height: 50 };
    expect(anchorOf(rect, "nw")).toEqual({ x: 100, y: 50 });
    expect(anchorOf(rect, "e")).toEqual({ x: 0, y: 25 });
    expect(anchorOf(rect, "s")).toEqual({ x: 50, y: 0 });
  });

  it("resizes an unrotated layer exactly like the plain rect maths", () => {
    const rect = { x: 0, y: 0, width: 100, height: 100 };
    expect(resizeRotated(rect, 0, "se", { x: 10, y: 20 }, false)).toEqual({
      x: 0,
      y: 0,
      width: 110,
      height: 120,
    });
  });

  it("keeps the anchor of a rotated layer in place", () => {
    const rect = { x: 0, y: 0, width: 100, height: 100 };
    const next = resizeRotated(rect, 90, "se", { x: 0, y: 40 }, false);
    expect(next.width).toBeCloseTo(140);
    expect(next.height).toBeCloseTo(100);
    const centreBefore = { x: 50, y: 50 };
    const centreAfter = { x: next.x + next.width / 2, y: next.y + next.height / 2 };
    expect(centreAfter.x).toBeCloseTo(centreBefore.x);
    expect(centreAfter.y).toBeCloseTo(centreBefore.y + 20);
  });
});

describe("selection bounds", () => {
  it("unions the rotated boxes of every selected layer", () => {
    const one = newRectLayer({ x: 0, y: 0, width: 50, height: 50, fill: "#000000" });
    const two = { ...newRectLayer({ x: 100, y: 20, width: 50, height: 50, fill: "#000000" }) };
    expect(selectionBounds([one, two])).toEqual({ x: 0, y: 0, width: 150, height: 70 });
    expect(selectionBounds([])).toBeNull();
  });
});
