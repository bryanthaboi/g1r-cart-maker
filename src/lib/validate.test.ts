import { describe, expect, it } from "vitest";
import { RULES } from "./constants";
import type { Cart } from "./types";
import {
  checkLoadOrder,
  hasErrors,
  validateCart,
  validateEngineRange,
  validateId,
  validateNewCart,
  validateRepo,
  validateShell,
  validateSummary,
  validateTitle,
  type NewCartForm,
} from "./validate";

function baseCart(overrides: Partial<Cart> = {}): Cart {
  return {
    schema: 1,
    id: "example",
    title: "Example",
    version: "1.0.0",
    author: "someone",
    repo: "owner/example",
    summary: "A short summary.",
    shell: "#d33a2c",
    label: "label.png",
    base: "red",
    engine: ">=1.9.0 <2.0.0",
    seal: "sealed",
    mods: [{ id: "mod-a", source: "github", repo: "owner/mod-a", version: "1.0.0", sha256: "a".repeat(64) }],
    load_order: ["mod-a"],
    ...overrides,
  };
}

describe("id", () => {
  it("accepts letters, numbers, hyphen and underscore", () => {
    expect(validateId("Kanto_Hard-Mode2")).toBeNull();
  });

  it("rejects an empty id", () => {
    expect(validateId("  ")).toMatch(/required/);
  });

  it("rejects spaces, dots and slashes", () => {
    expect(validateId("has space")).toMatch(/letters, numbers/);
    expect(validateId("has.dot")).toMatch(/letters, numbers/);
    expect(validateId("has/slash")).toMatch(/letters, numbers/);
  });

  it("rejects more than 64 characters", () => {
    expect(validateId("a".repeat(64))).toBeNull();
    expect(validateId("a".repeat(65))).toMatch(/at most 64/);
  });
});

describe("title, summary and shell", () => {
  it("caps the title at 48", () => {
    expect(validateTitle("a".repeat(48))).toBeNull();
    expect(validateTitle("a".repeat(49))).toMatch(/at most 48/);
  });

  it("caps the summary at 120", () => {
    expect(validateSummary("a".repeat(120))).toBeNull();
    expect(validateSummary("a".repeat(121))).toMatch(/at most 120/);
  });

  it("requires a six digit hex shell", () => {
    expect(validateShell("#d33a2c")).toBeNull();
    expect(validateShell("#D33A2C")).toBeNull();
    expect(validateShell("d33a2c")).toMatch(/hex value/);
    expect(validateShell("#d33")).toMatch(/hex value/);
    expect(validateShell("")).toMatch(/required/);
  });
});

describe("repo and engine range", () => {
  it("accepts owner/name and nothing else", () => {
    expect(validateRepo("owner/name")).toBeNull();
    expect(validateRepo("")).toBeNull();
    expect(validateRepo("owner")).toMatch(/owner\/name/);
    expect(validateRepo("owner/name/extra")).toMatch(/owner\/name/);
  });

  it("accepts a comparator range", () => {
    expect(validateEngineRange(">=1.9.0 <2.0.0")).toBeNull();
    expect(validateEngineRange("^1.2.3")).toBeNull();
    expect(validateEngineRange("latest")).toMatch(/not a version range/);
  });
});

describe("validateNewCart", () => {
  const form: NewCartForm = {
    id: "kanto",
    title: "Kanto",
    author: "me",
    summary: "",
    base: "red",
    shell: "#ffffff",
    seal: "sealed",
    github: "",
    parent: "/tmp",
  };

  it("passes a complete form", () => {
    expect(hasErrors(validateNewCart(form))).toBe(false);
  });

  it("reports every bad field at once", () => {
    const errors = validateNewCart({ ...form, id: "bad id", shell: "red", base: "gameboy", parent: "" });
    expect(errors.id).toBeDefined();
    expect(errors.shell).toBeDefined();
    expect(errors.base).toBeDefined();
    expect(errors.parent).toBeDefined();
    expect(hasErrors(errors)).toBe(true);
  });
});

describe("validateCart", () => {
  it("finds nothing blocking in a complete cart", () => {
    const findings = validateCart(baseCart());
    expect(findings.filter((finding) => finding.severity === "error")).toHaveLength(0);
    expect(findings.filter((finding) => finding.severity === "warn")).toHaveLength(0);
  });

  it("notes a missing summary rather than failing it", () => {
    const findings = validateCart(baseCart({ summary: undefined }));
    const summary = findings.filter((finding) => finding.path === "summary");
    expect(summary).toHaveLength(1);
    expect(summary[0]?.severity).toBe("note");
  });

  it("warns about a missing repo", () => {
    const findings = validateCart(baseCart({ repo: undefined }));
    const repo = findings.find((finding) => finding.path === "repo");
    expect(repo?.severity).toBe("warn");
    expect(repo?.rule).toBe(RULES.references);
  });

  it("errors on a speed outside the ladder", () => {
    const findings = validateCart(baseCart({ speeds: [1, 7] }));
    expect(findings.some((finding) => finding.severity === "error" && finding.path === "speeds")).toBe(true);
  });

  it("errors on an empty speed list", () => {
    const findings = validateCart(baseCart({ speeds: [] }));
    expect(findings.some((finding) => finding.severity === "error" && finding.path === "speeds")).toBe(true);
  });

  it("errors on a duplicate pin", () => {
    const pin = { id: "mod-a", source: "github" as const, repo: "owner/a", version: "1.0.0", sha256: "b".repeat(64) };
    const findings = validateCart(baseCart({ mods: [pin, pin], load_order: ["mod-a", "mod-a"] }));
    expect(findings.some((finding) => finding.rule === RULES.pinIntegrity && finding.severity === "error")).toBe(true);
  });

  it("warns when a github pin has no sha256", () => {
    const findings = validateCart(
      baseCart({ mods: [{ id: "mod-a", source: "github", repo: "owner/a", version: "1.0.0" }] }),
    );
    const warn = findings.find((finding) => finding.severity === "warn" && finding.message.includes("sha256"));
    expect(warn?.rule).toBe(RULES.pinIntegrity);
  });

  it("errors on an unknown base game", () => {
    const findings = validateCart(baseCart({ base: "gameboy" as Cart["base"] }));
    expect(findings.some((finding) => finding.rule === RULES.vocabulary && finding.path === "base")).toBe(true);
  });

  it("errors on an option value over 256 characters", () => {
    const findings = validateCart(
      baseCart({
        mods: [
          {
            id: "mod-a",
            source: "github",
            repo: "owner/a",
            version: "1.0.0",
            sha256: "c".repeat(64),
            options: { banner: "x".repeat(257) },
          },
        ],
      }),
    );
    expect(findings.some((finding) => finding.rule === RULES.limits && finding.severity === "error")).toBe(true);
  });
});

describe("checkLoadOrder", () => {
  it("passes when the order matches the pins", () => {
    expect(checkLoadOrder(["a", "b"], ["b", "a"])).toHaveLength(0);
  });

  it("returns nothing when load_order is absent", () => {
    expect(checkLoadOrder(["a"], undefined)).toHaveLength(0);
  });

  it("flags an unknown id", () => {
    const findings = checkLoadOrder(["a"], ["a", "ghost"]);
    expect(findings.some((finding) => finding.rule === RULES.loadOrderMembership && finding.message.includes("ghost"))).toBe(true);
  });

  it("flags a duplicate", () => {
    const findings = checkLoadOrder(["a"], ["a", "a"]);
    expect(findings.some((finding) => finding.rule === RULES.loadOrderDuplicates)).toBe(true);
  });

  it("flags a missing pin", () => {
    const findings = checkLoadOrder(["a", "b"], ["a"]);
    expect(findings.some((finding) => finding.message.includes('missing "b"'))).toBe(true);
  });
});
