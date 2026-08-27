// The three-way err/warn/note model. Notes never fail a cart and must never be
// folded into warnings.

import type { Report } from "./types";
import type { UiFinding, UiSeverity } from "./validate";

export interface GroupedFindings {
  error: UiFinding[];
  warn: UiFinding[];
  note: UiFinding[];
}

export const SEVERITY_ORDER: readonly UiSeverity[] = ["error", "warn", "note"];

export const SEVERITY_PLURAL: Record<UiSeverity, string> = {
  error: "Errors",
  warn: "Warnings",
  note: "Notes",
};

export function emptyGroups(): GroupedFindings {
  return { error: [], warn: [], note: [] };
}

export function groupFindings(findings: UiFinding[]): GroupedFindings {
  const groups = emptyGroups();
  for (const finding of findings) groups[finding.severity].push(finding);
  return groups;
}

/** A backend Report carries findings at two severities plus free-text notes. */
export function reportToFindings(report: Report | null): UiFinding[] {
  if (!report) return [];
  const out: UiFinding[] = report.findings.map((finding) => ({
    rule: finding.rule,
    severity: finding.severity === "error" ? ("error" as const) : ("warn" as const),
    message: finding.message,
    path: finding.path,
  }));
  for (const note of report.notes) {
    out.push({ rule: "", severity: "note", message: note, path: null });
  }
  return out;
}

export function mergeFindings(...lists: UiFinding[][]): UiFinding[] {
  const out: UiFinding[] = [];
  const seen = new Set<string>();
  for (const list of lists) {
    for (const finding of list) {
      const key = `${finding.severity} ${finding.rule} ${finding.path ?? ""} ${finding.message}`;
      if (seen.has(key)) continue;
      seen.add(key);
      out.push(finding);
    }
  }
  return out;
}

export function countBySeverity(findings: UiFinding[]): Record<UiSeverity, number> {
  const counts: Record<UiSeverity, number> = { error: 0, warn: 0, note: 0 };
  for (const finding of findings) counts[finding.severity] += 1;
  return counts;
}

/** Pack implies strict: a warning refuses the bundle, a note never does. */
export function blocksExport(findings: UiFinding[]): boolean {
  return findings.some((finding) => finding.severity === "error" || finding.severity === "warn");
}

export function findingsForPath(findings: UiFinding[], path: string): UiFinding[] {
  return findings.filter((finding) => finding.path === path);
}

export function summarize(findings: UiFinding[]): string {
  const counts = countBySeverity(findings);
  const parts: string[] = [];
  if (counts.error > 0) parts.push(`${counts.error} error${counts.error === 1 ? "" : "s"}`);
  if (counts.warn > 0) parts.push(`${counts.warn} warning${counts.warn === 1 ? "" : "s"}`);
  if (counts.note > 0) parts.push(`${counts.note} note${counts.note === 1 ? "" : "s"}`);
  if (parts.length === 0) return "No findings";
  return parts.join(", ");
}
