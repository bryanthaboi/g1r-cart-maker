import { describe, expect, it } from "vitest";
import { dropIndexFor } from "./pointerReorder";

// Four rows, 40px tall, stacked from y=0: centres at 20, 60, 100, 140.
const CENTRES = [20, 60, 100, 140];

describe("dropIndexFor", () => {
  it("keeps a row where it is when the pointer has not left it", () => {
    expect(dropIndexFor(CENTRES, 15, 0)).toBe(0);
    expect(dropIndexFor(CENTRES, 95, 2)).toBe(2);
  });

  it("moves a row down as the pointer passes each centre", () => {
    expect(dropIndexFor(CENTRES, 65, 0)).toBe(1);
    expect(dropIndexFor(CENTRES, 105, 0)).toBe(2);
    expect(dropIndexFor(CENTRES, 145, 0)).toBe(3);
  });

  it("moves a row up the same way", () => {
    expect(dropIndexFor(CENTRES, 15, 3)).toBe(0);
    expect(dropIndexFor(CENTRES, 55, 3)).toBe(1);
  });

  it("clamps past either end", () => {
    expect(dropIndexFor(CENTRES, -500, 2)).toBe(0);
    expect(dropIndexFor(CENTRES, 5000, 1)).toBe(3);
  });

  it("never lands on a row it could not measure", () => {
    expect(dropIndexFor([20, Number.POSITIVE_INFINITY], 30, 0)).toBe(0);
  });
});
