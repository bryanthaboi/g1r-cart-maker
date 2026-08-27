import { describe, expect, it } from "vitest";
import { readinessHeadline, summarizeReadiness } from "./readiness";
import type { ReadinessItem } from "./types";

function item(overrides: Partial<ReadinessItem>): ReadinessItem {
  return { id: "x", label: "x", ok: false, blocking: false, detail: "", fix: null, fixId: null, ...overrides };
}

describe("summarizeReadiness", () => {
  it("splits blocking, recommended and met", () => {
    const summary = summarizeReadiness([
      item({ id: "a", ok: true, blocking: true }),
      item({ id: "b", ok: false, blocking: true }),
      item({ id: "c", ok: false, blocking: false }),
    ]);
    expect(summary.met.map((entry) => entry.id)).toEqual(["a"]);
    expect(summary.blocking.map((entry) => entry.id)).toEqual(["b"]);
    expect(summary.recommended.map((entry) => entry.id)).toEqual(["c"]);
    expect(summary.listable).toBe(false);
    expect(summary.total).toBe(3);
    expect(summary.metCount).toBe(1);
  });

  it("is listable when nothing blocking is open", () => {
    const summary = summarizeReadiness([item({ ok: true, blocking: true }), item({ ok: false, blocking: false })]);
    expect(summary.listable).toBe(true);
  });

  it("handles an empty list", () => {
    const summary = summarizeReadiness([]);
    expect(summary.listable).toBe(true);
    expect(readinessHeadline(summary)).toBe("Nothing to check yet.");
  });
});

describe("readinessHeadline", () => {
  it("counts blocking items", () => {
    const summary = summarizeReadiness([item({ blocking: true }), item({ blocking: true })]);
    expect(readinessHeadline(summary)).toBe("2 items must be fixed before the index will list this cart.");
  });

  it("uses the singular for one blocking item", () => {
    const summary = summarizeReadiness([item({ blocking: true })]);
    expect(readinessHeadline(summary)).toBe("1 item must be fixed before the index will list this cart.");
  });

  it("mentions open recommendations once listable", () => {
    const summary = summarizeReadiness([item({ ok: true, blocking: true }), item({ blocking: false })]);
    expect(readinessHeadline(summary)).toBe("Listable. 1 recommended item still open.");
  });

  it("says ready when everything is met", () => {
    const summary = summarizeReadiness([item({ ok: true, blocking: true })]);
    expect(readinessHeadline(summary)).toBe("Ready to be listed in the index.");
  });
});
