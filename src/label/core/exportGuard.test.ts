import { describe, expect, it } from "vitest";
import type { ExportCheck } from "../../lib/types";
import {
  MAX_BYTES,
  WARN_BYTES,
  dataUrlBytes,
  decide,
  localCheck,
  looksLikePng,
  nextRecompress,
  pathProblem,
} from "./exportGuard";

const PNG = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUg==";

function check(partial: Partial<ExportCheck>): ExportCheck {
  return { ok: true, bytes: 0, width: null, height: null, problems: [], warnings: [], ...partial };
}

describe("data URL sizing", () => {
  it("measures the decoded byte length", () => {
    expect(dataUrlBytes("data:image/png;base64,AAAA")).toBe(3);
    expect(dataUrlBytes("data:image/png;base64,AAA=")).toBe(2);
    expect(dataUrlBytes("data:image/png;base64,AA==")).toBe(1);
    expect(dataUrlBytes("nonsense")).toBe(0);
  });

  it("recognises a PNG data URL by its signature", () => {
    expect(looksLikePng(PNG)).toBe(true);
    expect(looksLikePng("data:image/jpeg;base64,/9j/4AAQ")).toBe(false);
    expect(looksLikePng("data:image/png;base64,QUJD")).toBe(false);
  });
});

describe("path rules", () => {
  it("accepts a plain relative path", () => {
    expect(pathProblem("label.png")).toBeNull();
    expect(pathProblem("art/label.png")).toBeNull();
  });

  it("refuses absolute, escaping and over-long paths", () => {
    expect(pathProblem("/tmp/label.png")).not.toBeNull();
    expect(pathProblem("C:\\label.png")).not.toBeNull();
    expect(pathProblem("../label.png")).not.toBeNull();
    expect(pathProblem(`${"a".repeat(130)}.png`)).not.toBeNull();
    expect(pathProblem("")).not.toBeNull();
  });
});

describe("the local rehearsal of the backend check", () => {
  it("passes a small PNG", () => {
    expect(localCheck(PNG, "label.png").ok).toBe(true);
  });

  it("warns past the soft limit and blocks past the hard limit", () => {
    const warn = localCheck(`data:image/png;base64,iVBORw0KGgo${"A".repeat(Math.ceil((WARN_BYTES * 4) / 3) + 8)}`, "label.png");
    expect(warn.ok).toBe(true);
    expect(warn.warnings).toHaveLength(1);
    const blocked = localCheck(
      `data:image/png;base64,iVBORw0KGgo${"A".repeat(Math.ceil((MAX_BYTES * 4) / 3) + 8)}`,
      "label.png",
    );
    expect(blocked.ok).toBe(false);
  });

  it("blocks anything that is not a PNG", () => {
    expect(localCheck("data:image/jpeg;base64,/9j/4AAQ", "label.png").ok).toBe(false);
  });
});

describe("the export decision", () => {
  it("writes straight through when the check is clean", () => {
    expect(decide(check({}), { multiple: 1, quantize: null }).verdict).toBe("ok");
  });

  it("writes with a warning when only warnings came back", () => {
    const decision = decide(check({ warnings: ["large"] }), { multiple: 1, quantize: null });
    expect(decision.verdict).toBe("warn");
    expect(decision.retry).toBeNull();
  });

  it("offers a smaller export when the only problem is size", () => {
    const decision = decide(
      check({ ok: false, problems: ["label art is 2000000 bytes; the manifest caps it at 1048576"] }),
      { multiple: 4, quantize: null },
    );
    expect(decision.verdict).toBe("blocked");
    expect(decision.retry).toEqual({ multiple: 3, quantize: null });
  });

  it("offers nothing when the problem is not about size", () => {
    const decision = decide(
      check({ ok: false, problems: ["the exported file is not a PNG"] }),
      { multiple: 2, quantize: null },
    );
    expect(decision.retry).toBeNull();
  });

  it("steps down resolution first, then colour depth, then gives up", () => {
    expect(nextRecompress({ multiple: 3, quantize: null })).toEqual({ multiple: 2, quantize: null });
    expect(nextRecompress({ multiple: 2, quantize: null })).toEqual({ multiple: 1, quantize: null });
    expect(nextRecompress({ multiple: 1, quantize: null })).toEqual({ multiple: 1, quantize: 64 });
    expect(nextRecompress({ multiple: 1, quantize: 64 })).toEqual({ multiple: 1, quantize: 32 });
    expect(nextRecompress({ multiple: 1, quantize: 32 })).toEqual({ multiple: 1, quantize: 16 });
    expect(nextRecompress({ multiple: 1, quantize: 16 })).toBeNull();
  });
});
