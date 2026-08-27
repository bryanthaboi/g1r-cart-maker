import { describe, expect, it } from "vitest";
import type { Layer } from "../../lib/types";
import { newRectLayer } from "./doc";
import { pickBelow, pickInMarquee, pickLayer } from "./hittest";

function rect(id: string, x: number, y: number, extra: Partial<Layer> = {}): Layer {
  const layer = newRectLayer({ x, y, width: 100, height: 100, fill: "#ffffff" });
  return { ...layer, id, name: id, ...extra } as Layer;
}

describe("hit testing", () => {
  const layers: Layer[] = [rect("bottom", 0, 0), rect("top", 50, 50)];

  it("picks the topmost layer under the point", () => {
    expect(pickLayer(layers, { x: 60, y: 60 })?.id).toBe("top");
    expect(pickLayer(layers, { x: 10, y: 10 })?.id).toBe("bottom");
    expect(pickLayer(layers, { x: 400, y: 400 })).toBeNull();
  });

  it("never picks a hidden layer", () => {
    const hidden = [rect("bottom", 0, 0), rect("top", 50, 50, { hidden: true })];
    expect(pickLayer(hidden, { x: 60, y: 60 })?.id).toBe("bottom");
  });

  it("skips locked layers unless they are asked for", () => {
    const locked = [rect("bottom", 0, 0), rect("top", 50, 50, { locked: true })];
    expect(pickLayer(locked, { x: 60, y: 60 })?.id).toBe("bottom");
    expect(pickLayer(locked, { x: 60, y: 60 }, { includeLocked: true })?.id).toBe("top");
  });

  it("respects rotation when testing", () => {
    const turned = [rect("turned", 0, 0, { rotation: 45 })];
    expect(pickLayer(turned, { x: 50, y: 50 })?.id).toBe("turned");
    expect(pickLayer(turned, { x: 2, y: 2 })).toBeNull();
  });

  it("drills to the next layer below one already selected", () => {
    expect(pickBelow(layers, { x: 60, y: 60 }, "top")?.id).toBe("bottom");
    expect(pickBelow(layers, { x: 60, y: 60 }, "bottom")).toBeNull();
  });

  it("selects everything a marquee touches, in any drag direction", () => {
    expect(pickInMarquee(layers, { x: 0, y: 0, width: 200, height: 200 })).toHaveLength(2);
    expect(pickInMarquee(layers, { x: 120, y: 120, width: 40, height: 40 })).toHaveLength(1);
    expect(pickInMarquee(layers, { x: 160, y: 160, width: -40, height: -40 })).toHaveLength(1);
    expect(pickInMarquee(layers, { x: 400, y: 400, width: 10, height: 10 })).toHaveLength(0);
  });
});
