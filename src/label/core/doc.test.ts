import { describe, expect, it } from "vitest";
import type { LabelDoc } from "../../lib/types";
import {
  addLayer,
  blankDoc,
  duplicateLayers,
  moveInZ,
  newImageLayer,
  newRectLayer,
  newTextLayer,
  parseDoc,
  removeLayers,
  reorderLayers,
  serialiseDoc,
  uniqueName,
} from "./doc";

function sample(): LabelDoc {
  let doc = blankDoc("#101010", "red");
  doc = addLayer(
    doc,
    newImageLayer({
      source: "data:image/png;base64,AAAA",
      x: 0,
      y: 0,
      width: 500,
      height: 441,
      fit: "cover",
      name: "Artwork",
      fromTemplate: "red:art",
    }),
  );
  doc = addLayer(
    doc,
    newTextLayer({ text: "Kanto Remix", x: 42, y: 196, width: 416, height: 62, fromTemplate: "red:title" }),
  );
  doc = addLayer(doc, newRectLayer({ x: 10, y: 10, width: 80, height: 40, fill: "#ff0000", radius: 6 }));
  return doc;
}

describe("label document round trip", () => {
  it("survives serialise then parse unchanged", () => {
    const doc = sample();
    const parsed = parseDoc(serialiseDoc(doc));
    expect(parsed.ok).toBe(true);
    if (!parsed.ok) return;
    expect(parsed.doc).toEqual(doc);
    expect(serialiseDoc(parsed.doc)).toBe(serialiseDoc(doc));
  });

  it("keeps every layer field, including the optional ones", () => {
    const doc = sample();
    const parsed = parseDoc(serialiseDoc(doc));
    if (!parsed.ok) throw new Error(parsed.error);
    const text = parsed.doc.layers.find((layer) => layer.kind === "text");
    expect(text).toBeDefined();
    if (text?.kind !== "text") return;
    expect(text.stroke).toBeNull();
    expect(text.line_height).toBe(1.2);
    expect(text.from_template).toBe("red:title");
  });

  it("refuses a document from a newer schema", () => {
    const parsed = parseDoc(JSON.stringify({ ...sample(), schema: 99 }));
    expect(parsed.ok).toBe(false);
  });

  it("refuses text that is not a document", () => {
    expect(parseDoc("not json").ok).toBe(false);
    expect(parseDoc("[1,2,3]").ok).toBe(false);
  });

  it("drops layers of an unknown kind rather than importing them", () => {
    const raw = JSON.stringify({ ...blankDoc(), layers: [{ id: "x", kind: "video", x: 0, y: 0 }] });
    const parsed = parseDoc(raw);
    expect(parsed.ok).toBe(true);
    if (!parsed.ok) return;
    expect(parsed.doc.layers).toHaveLength(0);
  });

  it("fills defaults for a sparse layer", () => {
    const raw = JSON.stringify({ schema: 1, layers: [{ kind: "rect", fill: "#123456" }] });
    const parsed = parseDoc(raw);
    if (!parsed.ok) throw new Error(parsed.error);
    const layer = parsed.doc.layers[0];
    expect(layer?.width).toBe(100);
    expect(layer?.rotation).toBe(0);
    expect(layer?.from_template).toBeNull();
    expect(parsed.doc.width).toBe(500);
    expect(parsed.doc.height).toBe(441);
  });
});

describe("layer operations", () => {
  it("reorders a layer to the front and the back", () => {
    const doc = sample();
    const first = doc.layers[0];
    if (!first) throw new Error("no layers");
    const front = moveInZ(doc, [first.id], "front");
    expect(front.layers[front.layers.length - 1]?.id).toBe(first.id);
    const back = moveInZ(front, [first.id], "back");
    expect(back.layers[0]?.id).toBe(first.id);
    expect(back.layers).toHaveLength(doc.layers.length);
  });

  it("steps one layer forward without disturbing the others", () => {
    const doc = sample();
    const middle = doc.layers[1];
    if (!middle) throw new Error("no layers");
    const moved = moveInZ(doc, [middle.id], "forward");
    expect(moved.layers.map((layer) => layer.id)).toEqual([
      doc.layers[0]?.id,
      doc.layers[2]?.id,
      middle.id,
    ]);
  });

  it("drops a moved layer at the requested index", () => {
    const doc = sample();
    const last = doc.layers[2];
    if (!last) throw new Error("no layers");
    const moved = reorderLayers(doc, [last.id], 0);
    expect(moved.layers[0]?.id).toBe(last.id);
  });

  it("duplicates with fresh ids and unique names", () => {
    const doc = sample();
    const source = doc.layers[1];
    if (!source) throw new Error("no layers");
    const result = duplicateLayers(doc, [source.id]);
    expect(result.doc.layers).toHaveLength(4);
    const copy = result.doc.layers[3];
    expect(copy?.id).not.toBe(source.id);
    expect(copy?.name).toBe(`${source.name} copy`);
    expect(result.ids).toEqual([copy?.id]);
  });

  it("removes exactly the requested layers", () => {
    const doc = sample();
    const ids = [doc.layers[0]?.id ?? "", doc.layers[2]?.id ?? ""];
    const left = removeLayers(doc, ids);
    expect(left.layers).toHaveLength(1);
    expect(left.layers[0]?.id).toBe(doc.layers[1]?.id);
  });

  it("suffixes a name that is already taken", () => {
    const doc = sample();
    expect(uniqueName(doc, "Artwork")).toBe("Artwork 2");
    expect(uniqueName(doc, "Fresh")).toBe("Fresh");
  });
});
