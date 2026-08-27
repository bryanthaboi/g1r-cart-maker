import { describe, expect, it } from "vitest";
import {
  coerceForRow,
  defaultsFor,
  formatScalar,
  isRowVisible,
  optionCountProblem,
  parseRawOption,
  parseScalar,
  visibleRows,
  withDefaults,
} from "./options";
import type { OptionRow, OptionValue } from "./types";

const rows: OptionRow[] = [
  { key: "enabled", label: "Enabled", type: "toggle", default: true },
  {
    key: "mode",
    label: "Mode",
    type: "choice",
    default: "standard",
    choices: [
      ["Standard", "standard"],
      ["Custom", "custom"],
    ],
  },
  {
    key: "bump",
    label: "Bump",
    type: "number",
    default: 3,
    min: 0,
    max: 10,
    step: 1,
    visible_if: { key: "mode", equals: "custom" },
  },
  {
    key: "banner",
    label: "Banner",
    type: "text",
    default: "",
    maxLen: 8,
    visible_if: { key: "enabled", not_equals: false },
  },
];

describe("visible_if", () => {
  it("shows a row with no condition", () => {
    expect(isRowVisible(null, {})).toBe(true);
    expect(isRowVisible(undefined, {})).toBe(true);
  });

  it("honours equals", () => {
    expect(isRowVisible({ key: "mode", equals: "custom" }, { mode: "custom" })).toBe(true);
    expect(isRowVisible({ key: "mode", equals: "custom" }, { mode: "standard" })).toBe(false);
  });

  it("honours not_equals", () => {
    expect(isRowVisible({ key: "enabled", not_equals: false }, { enabled: true })).toBe(true);
    expect(isRowVisible({ key: "enabled", not_equals: false }, { enabled: false })).toBe(false);
  });

  it("treats an unset dependency as not equal", () => {
    expect(isRowVisible({ key: "mode", equals: "custom" }, {})).toBe(false);
    expect(isRowVisible({ key: "mode", not_equals: "custom" }, {})).toBe(true);
  });

  it("compares across types by string", () => {
    expect(isRowVisible({ key: "level", equals: 2 }, { level: "2" })).toBe(true);
  });

  it("filters the row list", () => {
    const defaults = defaultsFor(rows);
    expect(visibleRows(rows, defaults).map((row) => row.key)).toEqual(["enabled", "mode", "banner"]);
    expect(visibleRows(rows, { ...defaults, mode: "custom" }).map((row) => row.key)).toEqual([
      "enabled",
      "mode",
      "bump",
      "banner",
    ]);
    expect(visibleRows(rows, { ...defaults, enabled: false }).map((row) => row.key)).toEqual(["enabled", "mode"]);
  });
});

describe("defaults", () => {
  it("collects every row default", () => {
    expect(defaultsFor(rows)).toEqual({ enabled: true, mode: "standard", bump: 3, banner: "" });
  });

  it("lets stored values win", () => {
    expect(withDefaults(rows, { mode: "custom" }).mode).toBe("custom");
    expect(withDefaults(rows, { mode: "custom" }).enabled).toBe(true);
  });
});

describe("coercion", () => {
  it("clamps a number to its range", () => {
    const row = rows[2] as OptionRow;
    expect(coerceForRow(row, "20")).toBe(10);
    expect(coerceForRow(row, "-4")).toBe(0);
    expect(coerceForRow(row, "not a number")).toBe(3);
  });

  it("falls back to the default for an unknown choice", () => {
    const row = rows[1] as OptionRow;
    expect(coerceForRow(row, "custom")).toBe("custom");
    expect(coerceForRow(row, "nonsense")).toBe("standard");
  });

  it("truncates text to maxLen", () => {
    const row = rows[3] as OptionRow;
    expect(coerceForRow(row, "abcdefghijkl")).toBe("abcdefgh");
  });

  it("reads a toggle from its string forms", () => {
    const row = rows[0] as OptionRow;
    expect(coerceForRow(row, "true")).toBe(true);
    expect(coerceForRow(row, "false")).toBe(false);
  });
});

describe("raw key/value entry", () => {
  it("parses key=value", () => {
    expect(parseRawOption("difficulty=hard")).toEqual({ key: "difficulty", value: "hard" });
  });

  it("parses booleans and numbers", () => {
    expect(parseRawOption("on=true")).toEqual({ key: "on", value: true });
    expect(parseRawOption("rate=2.5")).toEqual({ key: "rate", value: 2.5 });
  });

  it("keeps a value containing an equals sign", () => {
    expect(parseRawOption("expr=a=b")).toEqual({ key: "expr", value: "a=b" });
  });

  it("rejects a missing equals sign and an empty key", () => {
    expect(parseRawOption("nope")).toEqual({ error: "Write it as key=value." });
    expect(parseRawOption("=value")).toEqual({ error: "Write it as key=value." });
  });

  it("rejects an over-long key or value", () => {
    expect(parseRawOption(`${"k".repeat(65)}=v`)).toHaveProperty("error");
    expect(parseRawOption(`k=${"v".repeat(257)}`)).toHaveProperty("error");
  });
});

describe("scalars and limits", () => {
  it("round-trips through format and parse", () => {
    expect(parseScalar(formatScalar(true))).toBe(true);
    expect(parseScalar(formatScalar(12))).toBe(12);
    expect(parseScalar(formatScalar("text"))).toBe("text");
  });

  it("flags more than 64 options", () => {
    const many: Record<string, OptionValue> = {};
    for (let index = 0; index < 65; index += 1) many[`k${index}`] = index;
    expect(optionCountProblem(many)).toMatch(/at most 64/);
    expect(optionCountProblem({ a: 1 })).toBeNull();
  });
});
