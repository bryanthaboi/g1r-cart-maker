import { describe, expect, it } from "vitest";
import { BASES, BASE_LABELS, BASE_SHELLS, SHELL_PATTERN, cartShape } from "../../lib/constants";
import type { Cart, LabelTemplate } from "../../lib/types";
import { canvasFor } from "./doc";
import { baseLine, docFromBlank, docFromTemplate, initialDoc, resetLayer, slotOf, textSlots } from "./templates";

function cart(overrides: Partial<Cart> = {}): Cart {
  return {
    schema: 1,
    id: "hoenn-remix",
    title: "Hoenn Remix",
    version: "1.0.0",
    author: "someone",
    shell: "#1f9e6e",
    base: "emerald",
    mods: [],
    ...overrides,
  };
}

const emerald: LabelTemplate = {
  id: "emerald",
  name: "Emerald",
  base: "emerald",
  width: 512,
  height: 260,
  dataUrl: "data:image/png;base64,iVBORw0KGgo=",
};

const GEN3 = ["firered", "leafgreen", "ruby", "sapphire", "emerald"] as const;

describe("gen 3 bases", () => {
  it("lists every base GameVersion.ORDER has, in that order", () => {
    expect(BASES).toEqual([
      "red",
      "blue",
      "yellow",
      "gold",
      "silver",
      "crystal",
      "firered",
      "leafgreen",
      "ruby",
      "sapphire",
      "emerald",
    ]);
  });

  it("gives each base a label, a launcher shell colour and a shape", () => {
    for (const base of BASES) {
      expect(BASE_LABELS[base].length).toBeGreaterThan(0);
      expect(BASE_SHELLS[base]).toMatch(SHELL_PATTERN);
    }
    for (const base of GEN3) expect(cartShape(base)).toBe("gba");
    expect(cartShape("crystal")).toBe("gb");
    expect(BASE_SHELLS.firered).toBe("#dc3030");
    expect(BASE_SHELLS.sapphire).toBe("#355ec4");
  });

  it("sizes a GBA label to the launcher's 512x260 canvas", () => {
    expect(canvasFor("ruby")).toEqual({ width: 512, height: 260 });
    expect(canvasFor("red")).toEqual({ width: 500, height: 441 });
  });

  it("derives a landscape document from a GBA template", () => {
    const doc = docFromTemplate(emerald, cart());
    expect(doc.width).toBe(512);
    expect(doc.height).toBe(260);
    expect(doc.layers.map(slotOf)).toEqual(["art", "title", "base"]);
    for (const layer of doc.layers) {
      expect(layer.x).toBeGreaterThanOrEqual(0);
      expect(layer.y).toBeGreaterThanOrEqual(0);
      expect(layer.x + layer.width).toBeLessThanOrEqual(doc.width);
      expect(layer.y + layer.height).toBeLessThanOrEqual(doc.height);
    }
    const base = doc.layers[2];
    expect(base?.kind === "text" ? base.text : null).toBe(baseLine("emerald"));
    expect(baseLine("leafgreen")).toBe("LeafGreen version");
  });

  it("keeps the GB text slots where they were", () => {
    const slots = textSlots({ width: 500, height: 441 });
    expect(slots.title).toEqual({ x: 42, y: 196, width: 416, height: 62, size: 40 });
    expect(slots.base).toEqual({ x: 42, y: 264, width: 416, height: 32, size: 21 });
  });

  it("falls back to a GBA-sized blank when no template matches", () => {
    const doc = docFromBlank(cart({ base: "sapphire" }));
    expect(doc.width).toBe(512);
    expect(doc.height).toBe(260);
    expect(initialDoc([], cart({ base: "firered" })).doc.width).toBe(512);
  });

  it("resets a GBA title back into the landscape slot", () => {
    const doc = docFromTemplate(emerald, cart());
    const title = doc.layers[1];
    if (!title) throw new Error("no title layer");
    const restored = resetLayer({ ...title, y: 5 }, emerald, cart());
    expect(restored?.y).toBe(title.y);
    expect(restored?.width).toBe(title.width);
  });
});
