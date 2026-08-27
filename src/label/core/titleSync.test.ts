import { describe, expect, it } from "vitest";
import type { Cart, LabelTemplate } from "../../lib/types";
import { addLayer, newRectLayer, newTextLayer } from "./doc";
import { docFromBlank, docFromTemplate, initialDoc, resetLayer, slotOf } from "./templates";
import { applyTextSync, factsOf, planTextSync } from "./titleSync";

function cart(overrides: Partial<Cart> = {}): Cart {
  return {
    schema: 1,
    id: "kanto-remix",
    title: "Kanto Remix",
    version: "1.0.0",
    author: "someone",
    shell: "#d33a2c",
    base: "red",
    mods: [],
    ...overrides,
  };
}

const template: LabelTemplate = {
  id: "red",
  name: "Red",
  base: "red",
  width: 500,
  height: 441,
  dataUrl: "data:image/png;base64,iVBORw0KGgo=",
};

describe("template derivation", () => {
  it("derives artwork plus the two text slots", () => {
    const doc = docFromTemplate(template, cart());
    expect(doc.template).toBe("red");
    expect(doc.layers).toHaveLength(3);
    expect(doc.layers.map(slotOf)).toEqual(["art", "title", "base"]);
    expect(doc.width).toBe(500);
    expect(doc.height).toBe(441);
  });

  it("falls back to a shell-coloured blank when no template matches the base", () => {
    const result = initialDoc([], cart({ base: "crystal" }));
    expect(result.template).toBeNull();
    expect(result.doc.template).toBe("blank");
    expect(result.doc.background).toBe("#d33a2c");
    expect(result.doc.layers.map(slotOf)).toEqual(["title", "base"]);
  });

  it("picks the template that matches the base game", () => {
    const others: LabelTemplate[] = [template, { ...template, id: "gold", name: "Gold", base: "gold" }];
    expect(initialDoc(others, cart({ base: "gold" })).template?.id).toBe("gold");
  });

  it("resets a template layer back to its original, keeping its id", () => {
    const doc = docFromTemplate(template, cart());
    const title = doc.layers[1];
    if (!title || title.kind !== "text") throw new Error("no title layer");
    const moved = { ...title, x: 999, text: "typed over" };
    const restored = resetLayer(moved, template, cart());
    expect(restored?.id).toBe(title.id);
    expect(restored?.x).toBe(title.x);
    if (restored?.kind === "text") expect(restored.text).toBe("Kanto Remix");
  });

  it("has nothing to reset for a layer the user added", () => {
    const layer = newRectLayer({ x: 0, y: 0, width: 10, height: 10, fill: "#000000" });
    expect(resetLayer(layer, template, cart())).toBeNull();
  });
});

describe("offering to follow a cart change", () => {
  it("offers nothing when neither the title nor the base moved", () => {
    const doc = docFromTemplate(template, cart());
    expect(planTextSync(doc, factsOf(cart()), factsOf(cart()))).toEqual([]);
  });

  it("offers the template title slot when the title changes", () => {
    const doc = docFromTemplate(template, cart());
    const plan = planTextSync(doc, factsOf(cart()), factsOf(cart({ title: "Johto Remix" })));
    expect(plan).toHaveLength(1);
    expect(plan[0]?.reason).toBe("title");
    expect(plan[0]?.next).toBe("Johto Remix");
  });

  it("offers a plain text layer that still reads as the old title", () => {
    const doc = addLayer(
      docFromBlank(cart()),
      newTextLayer({ text: "Kanto Remix", x: 0, y: 0, width: 100, height: 20 }),
    );
    const plan = planTextSync(doc, factsOf(cart()), factsOf(cart({ title: "Johto Remix" })));
    expect(plan).toHaveLength(2);
  });

  it("leaves a layer the user retyped alone", () => {
    let doc = docFromTemplate(template, cart());
    doc = {
      ...doc,
      layers: doc.layers.map((layer) =>
        layer.kind === "text" && slotOf(layer) === "title"
          ? { ...layer, from_template: null, text: "My Own Title" }
          : layer,
      ),
    };
    expect(planTextSync(doc, factsOf(cart()), factsOf(cart({ title: "Johto Remix" })))).toEqual([]);
  });

  it("never rewrites a locked layer", () => {
    let doc = docFromTemplate(template, cart());
    doc = { ...doc, layers: doc.layers.map((layer) => ({ ...layer, locked: true })) };
    expect(planTextSync(doc, factsOf(cart()), factsOf(cart({ title: "Johto Remix" })))).toEqual([]);
  });

  it("follows a base game change on the base line", () => {
    const doc = docFromTemplate(template, cart());
    const plan = planTextSync(doc, factsOf(cart()), factsOf(cart({ base: "gold" })));
    expect(plan).toHaveLength(1);
    expect(plan[0]?.reason).toBe("base");
    expect(plan[0]?.next).toBe("Gold version");
  });

  it("applies only the candidates it is given", () => {
    const doc = docFromTemplate(template, cart());
    const plan = planTextSync(doc, factsOf(cart()), factsOf(cart({ title: "Johto Remix" })));
    const next = applyTextSync(doc, plan);
    const title = next.layers[1];
    if (title?.kind !== "text") throw new Error("no title layer");
    expect(title.text).toBe("Johto Remix");
    expect(applyTextSync(doc, [])).toBe(doc);
  });
});
