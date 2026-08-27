import { renderToString } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { Cart, LabelDoc } from "../lib/types";
import LabelDesigner from "./LabelDesigner";
import { blankDoc, newTextLayer } from "./core/doc";

const cart: Cart = {
  schema: 1,
  id: "kanto-remix",
  title: "Kanto Remix",
  version: "1.0.0",
  author: "someone",
  shell: "#d33a2c",
  base: "red",
  finish: "sparkle+holo",
  mods: [],
};

function doc(): LabelDoc {
  const base = blankDoc("#202020", "red");
  return {
    ...base,
    layers: [newTextLayer({ text: "Kanto Remix", x: 40, y: 190, width: 420, height: 60 })],
  };
}

describe("the designer shell", () => {
  it("renders a document without touching the backend", () => {
    const markup = renderToString(
      <LabelDesigner
        doc={doc()}
        cart={cart}
        labelPath="label.png"
        dir="/carts/kanto-remix"
        onChange={() => undefined}
        onExported={() => undefined}
      />,
    );
    expect(markup).toContain("Layers");
    expect(markup).toContain("label.png");
    expect(markup).toContain("441");
    expect(markup).toContain("template ");
  });

  it("renders with no document at all", () => {
    const markup = renderToString(
      <LabelDesigner
        doc={null}
        cart={cart}
        labelPath="label.png"
        dir="/carts/kanto-remix"
        onChange={() => undefined}
        onExported={() => undefined}
      />,
    );
    expect(markup).toContain("Export PNG");
  });
});
