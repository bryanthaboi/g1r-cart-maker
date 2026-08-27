// Mirrors ModIndex.compatIssues. The index feed carries no precomputed issue
// list, so they are derived from the entry against the configured engine.

import type { CompatIssue, IndexModEntry } from "./types";

export interface EngineContext {
  engineVersion: string;
  modApi: number;
}

function parts(version: string): number[] {
  const core = version.trim().split(/[-+]/)[0] ?? "";
  return core.split(".").map((piece) => {
    const parsed = Number.parseInt(piece, 10);
    return Number.isFinite(parsed) ? parsed : 0;
  });
}

export function compareVersions(a: string, b: string): number {
  const left = parts(a);
  const right = parts(b);
  const length = Math.max(left.length, right.length);
  for (let index = 0; index < length; index += 1) {
    const l = left[index] ?? 0;
    const r = right[index] ?? 0;
    if (l !== r) return l < r ? -1 : 1;
  }
  return 0;
}

/** Mirrors Semver.satisfies: `||` alternatives of space-joined comparators. */
export function satisfiesRange(version: string, range: string | null | undefined): boolean {
  if (!range || range.trim().length === 0) return true;
  return range
    .split("||")
    .some((alternative) => satisfiesAll(version, alternative));
}

function satisfiesAll(version: string, alternative: string): boolean {
  const terms = alternative.trim().split(/\s+/).filter((term) => term.length > 0);
  if (terms.length === 0) return false;
  for (const term of terms) {
    const match = /^(>=|<=|==|>|<|=|\^)?v?(.+)$/.exec(term);
    if (!match) return false;
    const operator = match[1] ?? "";
    const target = match[2] ?? "";
    if (target.length === 0) return false;
    const order = compareVersions(version, target);
    switch (operator) {
      case ">=":
        if (order < 0) return false;
        break;
      case ">":
        if (order <= 0) return false;
        break;
      case "<=":
        if (order > 0) return false;
        break;
      case "<":
        if (order >= 0) return false;
        break;
      case "^": {
        // Caret pins the leftmost non-zero component, as Semver.lua does.
        if (order < 0) return false;
        const bound = parts(target);
        const major = bound[0] ?? 0;
        const minor = bound[1] ?? 0;
        const patch = bound[2] ?? 0;
        const ceiling =
          major > 0
            ? `${major + 1}.0.0`
            : minor > 0
              ? `0.${minor + 1}.0`
              : `0.0.${patch + 1}`;
        if (compareVersions(version, ceiling) >= 0) return false;
        break;
      }
      default:
        if (order !== 0) return false;
        break;
    }
  }
  return true;
}

export function compatIssues(entry: IndexModEntry, context: EngineContext): CompatIssue[] {
  const issues: CompatIssue[] = [];
  if (entry.api !== null && entry.api > context.modApi) {
    issues.push({
      level: "error",
      text: `Needs mod API ${entry.api}; this engine provides ${context.modApi}.`,
    });
  }
  if (!satisfiesRange(context.engineVersion, entry.game_version)) {
    issues.push({
      level: "error",
      text: `Declares engine ${entry.game_version ?? ""}, which ${context.engineVersion} does not satisfy.`,
    });
  }
  if (entry.profile !== null && entry.profile !== "content") {
    issues.push({ level: "warn", text: `Profile is ${entry.profile}, not content. It changes more than assets.` });
  }
  if (entry.affects_link) {
    issues.push({ level: "warn", text: "Affects link play. Both players need the same cart." });
  }
  if (entry.experimental) {
    issues.push({ level: "warn", text: "Marked experimental by its author." });
  }
  if (entry.permissions.length > 0) {
    issues.push({ level: "note", text: `Requests ${entry.permissions.join(", ")}.` });
  }
  return issues;
}

export function worstLevel(issues: readonly CompatIssue[]): "error" | "warn" | "note" | null {
  if (issues.some((issue) => issue.level === "error")) return "error";
  if (issues.some((issue) => issue.level === "warn")) return "warn";
  if (issues.length > 0) return "note";
  return null;
}

export function matchesBase(entry: IndexModEntry, base: string): boolean {
  if (entry.games.length === 0) return true;
  return entry.games.includes(base);
}
