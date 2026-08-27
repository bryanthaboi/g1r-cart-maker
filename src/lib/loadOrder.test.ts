import { describe, expect, it } from "vitest";
import { moveItem, normalizeLoadOrder, sameOrder, shiftById } from "./loadOrder";

describe("moveItem", () => {
  it("moves an item down", () => {
    expect(moveItem(["a", "b", "c"], 0, 2)).toEqual(["b", "c", "a"]);
  });

  it("moves an item up", () => {
    expect(moveItem(["a", "b", "c"], 2, 0)).toEqual(["c", "a", "b"]);
  });

  it("clamps a target past the end", () => {
    expect(moveItem(["a", "b"], 0, 9)).toEqual(["b", "a"]);
    expect(moveItem(["a", "b"], 1, -4)).toEqual(["b", "a"]);
  });

  it("returns a copy when the index is out of range", () => {
    const list = ["a", "b"];
    const result = moveItem(list, 5, 0);
    expect(result).toEqual(list);
    expect(result).not.toBe(list);
  });

  it("never drops or duplicates an item", () => {
    const list = ["a", "b", "c", "d"];
    for (let from = 0; from < list.length; from += 1) {
      for (let to = 0; to < list.length; to += 1) {
        const moved = moveItem(list, from, to);
        expect(moved.slice().sort()).toEqual(list.slice().sort());
      }
    }
  });
});

describe("normalizeLoadOrder", () => {
  it("appends pins the order does not mention", () => {
    expect(normalizeLoadOrder(["b"], ["a", "b", "c"])).toEqual(["b", "a", "c"]);
  });

  it("drops an id that is no longer pinned", () => {
    expect(normalizeLoadOrder(["ghost", "a"], ["a"])).toEqual(["a"]);
  });

  it("drops a duplicate", () => {
    expect(normalizeLoadOrder(["a", "a", "b"], ["a", "b"])).toEqual(["a", "b"]);
  });

  it("builds the pin order when there is no load_order", () => {
    expect(normalizeLoadOrder(undefined, ["a", "b"])).toEqual(["a", "b"]);
  });

  it("returns an empty list when nothing is pinned", () => {
    expect(normalizeLoadOrder(["a"], [])).toEqual([]);
  });
});

describe("shiftById", () => {
  it("leaves the order alone for an unknown id", () => {
    expect(shiftById(["a", "b"], "ghost", 1)).toEqual(["a", "b"]);
  });

  it("shifts by one in each direction", () => {
    expect(shiftById(["a", "b", "c"], "b", -1)).toEqual(["b", "a", "c"]);
    expect(shiftById(["a", "b", "c"], "b", 1)).toEqual(["a", "c", "b"]);
  });

  it("stops at the ends", () => {
    expect(shiftById(["a", "b"], "a", -1)).toEqual(["a", "b"]);
    expect(shiftById(["a", "b"], "b", 1)).toEqual(["a", "b"]);
  });
});

describe("sameOrder", () => {
  it("compares position by position", () => {
    expect(sameOrder(["a", "b"], ["a", "b"])).toBe(true);
    expect(sameOrder(["a", "b"], ["b", "a"])).toBe(false);
    expect(sameOrder(["a"], ["a", "b"])).toBe(false);
  });
});
