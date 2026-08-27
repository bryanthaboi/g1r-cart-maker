// Renders the real shipped template through the real export path on a real
// canvas, then reads the PNG back. The designer's own drawing is otherwise only
// ever exercised by hand, which is how an export can look nothing like the
// canvas and nobody finds out until a cart ships.

import { createCanvas, loadImage } from "@napi-rs/canvas";
import { readFileSync } from "node:fs";
import { resolve as joinPath } from "node:path";
import { beforeAll, describe, expect, it } from "vitest";
import { initialDoc } from "./templates";
import { exportDataUrl } from "./exportPng";
import type { Bitmap, ImageResolver } from "./render";
import type { Cart, LabelDoc, LabelTemplate } from "../../lib/types";

const TEMPLATE = joinPath(process.cwd(), "assets/labels/red.png");

let templateUrl = "";
let templateBitmap: Bitmap;

beforeAll(async () => {
  // exportPng builds its canvas through the document; give it a real one.
  (globalThis as unknown as { document: unknown }).document = {
    createElement(tag: string) {
      if (tag !== "canvas") throw new Error(`unexpected element ${tag}`);
      return createCanvas(1, 1);
    },
  };
  const bytes = readFileSync(TEMPLATE);
  templateUrl = `data:image/png;base64,${bytes.toString("base64")}`;
  const image = await loadImage(bytes);
  templateBitmap = { image, width: image.width, height: image.height } as unknown as Bitmap;
});

function templates(): LabelTemplate[] {
  return [{ id: "red", name: "Red", base: "red", width: 500, height: 441, dataUrl: templateUrl }];
}

function cart(): Cart {
  return { id: "test-cart", title: "TEST CART", base: "red", shell: "#d33a2c", version: "0.1.0" } as Cart;
}

/// Every image layer in these docs is the template art.
const resolve: ImageResolver = () => templateBitmap;

async function decodeAsync(dataUrl: string): Promise<{
  width: number;
  height: number;
  at: (x: number, y: number) => number[];
}> {
  const body = Buffer.from(dataUrl.slice(dataUrl.indexOf(",") + 1), "base64");
  const image = await loadImage(body);
  const canvas = createCanvas(image.width, image.height);
  const ctx = canvas.getContext("2d");
  ctx.drawImage(image, 0, 0);
  return {
    width: image.width,
    height: image.height,
    at: (x: number, y: number) => Array.from(ctx.getImageData(x, y, 1, 1).data),
  };
}

async function templatePixels(): Promise<(x: number, y: number) => number[]> {
  const image = await loadImage(readFileSync(TEMPLATE));
  const canvas = createCanvas(image.width, image.height);
  const ctx = canvas.getContext("2d");
  ctx.drawImage(image, 0, 0);
  return (x: number, y: number) => Array.from(ctx.getImageData(x, y, 1, 1).data);
}

function docFor(): LabelDoc {
  return initialDoc(templates(), cart()).doc;
}

describe("label export", () => {
  it("writes a PNG at the template's own size", async () => {
    const png = await decodeAsync(exportDataUrl(docFor(), resolve, { multiple: 1, quantize: null }));
    expect([png.width, png.height]).toEqual([500, 441]);
  });

  it("is fully opaque, with no transparent holes", async () => {
    const png = await decodeAsync(exportDataUrl(docFor(), resolve, { multiple: 1, quantize: null }));
    for (const [x, y] of [
      [1, 1],
      [250, 220],
      [498, 439],
      [1, 439],
      [498, 1],
    ] as const) {
      expect(png.at(x, y)[3], `alpha at ${x},${y}`).toBe(255);
    }
  });

  it("draws the template art itself, not just the shell colour", async () => {
    const png = await decodeAsync(exportDataUrl(docFor(), resolve, { multiple: 1, quantize: null }));
    const source = await templatePixels();
    let matching = 0;
    let compared = 0;
    for (let y = 4; y < 437; y += 17) {
      for (let x = 4; x < 496; x += 17) {
        const from = source(x, y);
        if ((from[3] ?? 0) < 250) continue;
        compared += 1;
        const got = png.at(x, y);
        const near =
          Math.abs((got[0] ?? 0) - (from[0] ?? 0)) <= 6 &&
          Math.abs((got[1] ?? 0) - (from[1] ?? 0)) <= 6 &&
          Math.abs((got[2] ?? 0) - (from[2] ?? 0)) <= 6;
        if (near) matching += 1;
      }
    }
    expect(compared).toBeGreaterThan(100);
    // The title text is drawn over the art, so a slice of the samples differ by
    // design; well over half matching proves the art itself is there.
    expect(matching / compared, `${matching} of ${compared} sampled pixels match the template`).toBeGreaterThan(0.8);
  });

  it("scales by a whole multiple without changing what is drawn", async () => {
    const one = await decodeAsync(exportDataUrl(docFor(), resolve, { multiple: 1, quantize: null }));
    const two = await decodeAsync(exportDataUrl(docFor(), resolve, { multiple: 2, quantize: null }));
    expect([two.width, two.height]).toEqual([1000, 882]);
    // Averaged over a block: a single sample at 2x can land on an antialiased
    // edge, and the fonts differ between a developer's machine and CI.
    const mean = (
      png: { at: (x: number, y: number) => number[] },
      x0: number,
      y0: number,
      size: number,
    ): number[] => {
      let red = 0;
      let green = 0;
      let blue = 0;
      let count = 0;
      for (let y = y0; y < y0 + size; y += 2) {
        for (let x = x0; x < x0 + size; x += 2) {
          const px = png.at(x, y);
          red += px[0] ?? 0;
          green += px[1] ?? 0;
          blue += px[2] ?? 0;
          count += 1;
        }
      }
      return [red / count, green / count, blue / count];
    };
    const a = mean(one, 60, 40, 40);
    const b = mean(two, 120, 80, 80);
    for (let channel = 0; channel < 3; channel += 1) {
      expect(Math.abs((a[channel] ?? 0) - (b[channel] ?? 0)), `channel ${channel}`).toBeLessThan(12);
    }
  });

  it("renders the same bytes twice for the same document", async () => {
    const first = exportDataUrl(docFor(), resolve, { multiple: 1, quantize: null });
    const second = exportDataUrl(docFor(), resolve, { multiple: 1, quantize: null });
    expect(first).toBe(second);
  });

  it("leaves a missing bitmap as a gap rather than failing the export", async () => {
    const png = await decodeAsync(
      exportDataUrl(docFor(), () => null, { multiple: 1, quantize: null }),
    );
    expect([png.width, png.height]).toEqual([500, 441]);
  });
});
