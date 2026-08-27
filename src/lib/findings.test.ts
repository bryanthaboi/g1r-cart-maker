import { describe, expect, it } from "vitest";
import {
  blocksExport,
  countBySeverity,
  findingsForPath,
  groupFindings,
  mergeFindings,
  reportToFindings,
  summarize,
} from "./findings";
import type { Report } from "./types";
import type { UiFinding } from "./validate";

const findings: UiFinding[] = [
  { rule: "CK001", severity: "error", message: "no id", path: "id" },
  { rule: "CK005", severity: "warn", message: "no repo", path: "repo" },
  { rule: "", severity: "note", message: "GitHub was unreachable", path: null },
  { rule: "CK101", severity: "warn", message: "no sha256", path: "mods[0]" },
];

describe("groupFindings", () => {
  it("keeps the three severities apart", () => {
    const groups = groupFindings(findings);
    expect(groups.error).toHaveLength(1);
    expect(groups.warn).toHaveLength(2);
    expect(groups.note).toHaveLength(1);
  });

  it("returns three empty buckets for an empty list", () => {
    const groups = groupFindings([]);
    expect(groups).toEqual({ error: [], warn: [], note: [] });
  });
});

describe("reportToFindings", () => {
  it("turns report notes into note-severity findings", () => {
    const report: Report = {
      findings: [{ rule: "CK002", severity: "error", message: "bad base", path: "base" }],
      notes: ["The GameBanana API did not answer."],
    };
    const converted = reportToFindings(report);
    expect(converted).toHaveLength(2);
    expect(converted[0]?.severity).toBe("error");
    expect(converted[1]?.severity).toBe("note");
    expect(converted[1]?.rule).toBe("");
  });

  it("returns nothing for a null report", () => {
    expect(reportToFindings(null)).toEqual([]);
  });
});

describe("blocksExport", () => {
  it("blocks on an error", () => {
    expect(blocksExport([findings[0] as UiFinding])).toBe(true);
  });

  it("blocks on a warning, because packing is strict", () => {
    expect(blocksExport([findings[1] as UiFinding])).toBe(true);
  });

  it("never blocks on a note alone", () => {
    expect(blocksExport([findings[2] as UiFinding])).toBe(false);
  });
});

describe("mergeFindings", () => {
  it("drops an identical duplicate across lists", () => {
    const merged = mergeFindings(findings, [findings[0] as UiFinding]);
    expect(merged).toHaveLength(findings.length);
  });

  it("keeps two findings that differ only by severity", () => {
    const merged = mergeFindings(
      [{ rule: "CK001", severity: "error", message: "same", path: null }],
      [{ rule: "CK001", severity: "warn", message: "same", path: null }],
    );
    expect(merged).toHaveLength(2);
  });
});

describe("counting and summarising", () => {
  it("counts each severity", () => {
    expect(countBySeverity(findings)).toEqual({ error: 1, warn: 2, note: 1 });
  });

  it("summarises in plural where needed", () => {
    expect(summarize(findings)).toBe("1 error, 2 warnings, 1 note");
    expect(summarize([])).toBe("No findings");
  });

  it("filters by path", () => {
    expect(findingsForPath(findings, "repo")).toHaveLength(1);
    expect(findingsForPath(findings, "nothing")).toHaveLength(0);
  });
});
