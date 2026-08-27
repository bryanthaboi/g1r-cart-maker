import { describe, expect, it } from "vitest";
import { describeSpec, parseSpec } from "./spec";

describe("GitHub specs", () => {
  it("reads a bare owner/repo", () => {
    const spec = parseSpec("bryanthaboi/example-mod");
    expect(spec).toEqual({
      kind: "github",
      slug: "bryanthaboi/example-mod",
      version: null,
      normalized: "bryanthaboi/example-mod",
    });
  });

  it("reads owner/repo@version", () => {
    const spec = parseSpec("bryanthaboi/example-mod@1.2.3");
    expect(spec.kind).toBe("github");
    if (spec.kind === "github") {
      expect(spec.version).toBe("1.2.3");
      expect(spec.normalized).toBe("bryanthaboi/example-mod@1.2.3");
    }
  });

  it("reads a github.com URL and drops a trailing slash", () => {
    const spec = parseSpec("https://github.com/owner/name/");
    expect(spec.kind).toBe("github");
    if (spec.kind === "github") expect(spec.slug).toBe("owner/name");
  });

  it("reads a release tag URL as a version", () => {
    const spec = parseSpec("https://github.com/owner/name/releases/tag/v2.0.1");
    expect(spec.kind).toBe("github");
    if (spec.kind === "github") expect(spec.version).toBe("2.0.1");
  });

  it("strips a .git suffix", () => {
    const spec = parseSpec("https://github.com/owner/name.git");
    if (spec.kind === "github") expect(spec.slug).toBe("owner/name");
    else throw new Error("expected a github spec");
  });
});

describe("GameBanana specs", () => {
  it("reads a full mod URL", () => {
    expect(parseSpec("https://gamebanana.com/mods/546899")).toEqual({
      kind: "gamebanana",
      modId: 546899,
      normalized: "gamebanana:546899",
    });
  });

  it("reads the gamebanana: prefix", () => {
    const spec = parseSpec("gamebanana:546899");
    expect(spec.kind).toBe("gamebanana");
  });

  it("reads a bare numeric id", () => {
    const spec = parseSpec("546899");
    expect(spec.kind).toBe("gamebanana");
    if (spec.kind === "gamebanana") expect(spec.modId).toBe(546899);
  });

  it("rejects a gamebanana URL with no id", () => {
    expect(parseSpec("https://gamebanana.com/mods").kind).toBe("unknown");
  });
});

describe("malformed input", () => {
  it("rejects an empty string with an instruction", () => {
    const spec = parseSpec("   ");
    expect(spec.kind).toBe("unknown");
    if (spec.kind === "unknown") expect(spec.reason).toMatch(/Paste a GitHub repo/);
  });

  it("rejects an unrelated host by name", () => {
    const spec = parseSpec("https://example.com/owner/name");
    expect(spec.kind).toBe("unknown");
    if (spec.kind === "unknown") expect(spec.reason).toMatch(/example\.com/);
  });

  it("rejects a version with no owner", () => {
    const spec = parseSpec("name@1.2.3");
    expect(spec.kind).toBe("unknown");
    if (spec.kind === "unknown") expect(spec.reason).toMatch(/owner is missing/);
  });

  it("rejects a github URL with only an owner", () => {
    expect(parseSpec("https://github.com/owner").kind).toBe("unknown");
  });

  it("rejects a zero id", () => {
    expect(parseSpec("gamebanana:0").kind).toBe("unknown");
  });
});

describe("describeSpec", () => {
  it("explains each shape", () => {
    expect(describeSpec(parseSpec("owner/name@1.0.0"))).toMatch(/version 1\.0\.0/);
    expect(describeSpec(parseSpec("owner/name"))).toMatch(/to be chosen/);
    expect(describeSpec(parseSpec("gamebanana:12"))).toMatch(/GameBanana mod 12/);
    expect(describeSpec(parseSpec("???"))).toMatch(/Not recognised/);
  });
});
