import { describe, expect, it } from "vitest";
import { compareVersions, compatIssues, matchesBase, satisfiesRange, worstLevel } from "./compat";
import type { IndexModEntry } from "./types";

function entry(overrides: Partial<IndexModEntry> = {}): IndexModEntry {
  return {
    id: "mod",
    title: "Mod",
    author: "someone",
    version: "1.0.0",
    summary: "",
    categories: [],
    tags: [],
    games: ["red"],
    repo: "owner/mod",
    github: null,
    api: 2,
    game_version: ">=1.0.0 <2.0.0",
    profile: "content",
    affects_link: false,
    experimental: false,
    permissions: [],
    thumbnail: null,
    description_url: null,
    downloads: null,
    first_release: null,
    last_release: null,
    update_check: "automatic",
    latest: null,
    ...overrides,
  };
}

const engine = { engineVersion: "1.9.0", modApi: 2 };

describe("compareVersions", () => {
  it("orders by each numeric part", () => {
    expect(compareVersions("1.2.3", "1.2.3")).toBe(0);
    expect(compareVersions("1.10.0", "1.9.0")).toBe(1);
    expect(compareVersions("1.0.0", "1.0.1")).toBe(-1);
  });

  it("ignores a prerelease suffix", () => {
    expect(compareVersions("2.0.0-rc1", "2.0.0")).toBe(0);
  });
});

describe("satisfiesRange", () => {
  it("treats an empty range as satisfied", () => {
    expect(satisfiesRange("1.0.0", null)).toBe(true);
    expect(satisfiesRange("1.0.0", "")).toBe(true);
  });

  it("reads a lower bound and a bare upper bound", () => {
    expect(satisfiesRange("1.9.0", ">=1.8.0 <2.0.0")).toBe(true);
    expect(satisfiesRange("1.7.0", ">=1.8.0 <2.0.0")).toBe(false);
    expect(satisfiesRange("2.0.0", ">=1.8.0 <2.0.0")).toBe(false);
  });

  it("reads caret the way Semver.lua does", () => {
    expect(satisfiesRange("1.9.0", "^1.2.0")).toBe(true);
    expect(satisfiesRange("2.0.0", "^1.2.0")).toBe(false);
    expect(satisfiesRange("0.2.9", "^0.2.0")).toBe(true);
    expect(satisfiesRange("0.3.0", "^0.2.0")).toBe(false);
  });

  it("treats a bare version as equality and reads alternatives", () => {
    expect(satisfiesRange("1.2.3", "1.2.3")).toBe(true);
    expect(satisfiesRange("1.2.4", "1.2.3")).toBe(false);
    expect(satisfiesRange("0.8.0", ">=1.0.0 || <0.9")).toBe(true);
    expect(satisfiesRange("0.9.5", ">=1.0.0 || <0.9")).toBe(false);
  });
});

describe("compatIssues", () => {
  it("reports nothing for a plain content mod", () => {
    expect(compatIssues(entry(), engine)).toHaveLength(0);
  });

  it("errors when the mod needs a newer API", () => {
    const issues = compatIssues(entry({ api: 3 }), engine);
    expect(issues[0]?.level).toBe("error");
  });

  it("errors when the engine range is unsatisfied", () => {
    const issues = compatIssues(entry({ game_version: ">=2.1.0 <3.0.0" }), engine);
    expect(issues.some((issue) => issue.level === "error")).toBe(true);
  });

  it("warns for a non-content profile, link effects and experimental", () => {
    const issues = compatIssues(entry({ profile: "overhaul", affects_link: true, experimental: true }), engine);
    expect(issues.filter((issue) => issue.level === "warn")).toHaveLength(3);
  });

  it("notes declared permissions", () => {
    const issues = compatIssues(entry({ permissions: ["network", "filesystem"] }), engine);
    expect(issues[0]?.level).toBe("note");
    expect(issues[0]?.text).toMatch(/network, filesystem/);
  });
});

describe("worstLevel and matchesBase", () => {
  it("picks the highest level present", () => {
    expect(worstLevel([{ level: "note", text: "" }, { level: "error", text: "" }])).toBe("error");
    expect(worstLevel([{ level: "note", text: "" }, { level: "warn", text: "" }])).toBe("warn");
    expect(worstLevel([{ level: "note", text: "" }])).toBe("note");
    expect(worstLevel([])).toBeNull();
  });

  it("treats an empty game list as matching everything", () => {
    expect(matchesBase(entry({ games: [] }), "crystal")).toBe(true);
    expect(matchesBase(entry({ games: ["red"] }), "crystal")).toBe(false);
    expect(matchesBase(entry({ games: ["red"] }), "red")).toBe(true);
  });
});
