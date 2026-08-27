// The export gate. Numbers mirror the manifest limits in the Rust core; the backend
// check is still authoritative, this only decides what to offer before and after it.

import type { ExportCheck } from "../../lib/types";

export const MAX_BYTES = 1024 * 1024;
export const WARN_BYTES = 256 * 1024;
export const MAX_PATH_CHARS = 128;
export const PNG_DATA_PREFIX = "data:image/png;base64,";
/** Base64 of the eight-byte PNG signature. */
const PNG_BASE64_HEAD = "iVBORw0KGgo";

export const MULTIPLES: readonly number[] = [1, 2, 3, 4];
export const QUANTIZE_STEPS: readonly (number | null)[] = [null, 64, 32, 16];

export interface ExportSettings {
  /** Resolution multiple of the document size. */
  multiple: number;
  /** Levels per channel, or null for full colour. */
  quantize: number | null;
}

export const DEFAULT_SETTINGS: ExportSettings = { multiple: 1, quantize: null };

/** Decoded byte length of a base64 data URL, without decoding it. */
export function dataUrlBytes(dataUrl: string): number {
  const comma = dataUrl.indexOf(",");
  if (comma < 0) return 0;
  const body = dataUrl.slice(comma + 1);
  if (!dataUrl.slice(0, comma).includes(";base64")) return body.length;
  const padding = body.endsWith("==") ? 2 : body.endsWith("=") ? 1 : 0;
  return Math.max(0, Math.floor((body.length * 3) / 4) - padding);
}

export function looksLikePng(dataUrl: string): boolean {
  if (!dataUrl.startsWith(PNG_DATA_PREFIX)) return false;
  return dataUrl.slice(PNG_DATA_PREFIX.length).startsWith(PNG_BASE64_HEAD);
}

export function pathProblem(labelPath: string): string | null {
  if (labelPath.trim().length === 0) return "the label path is empty";
  if ([...labelPath].length > MAX_PATH_CHARS) {
    return `the label path is longer than ${MAX_PATH_CHARS} characters`;
  }
  if (labelPath.startsWith("/") || labelPath.includes("\\") || /^[A-Za-z]:/.test(labelPath)) {
    return "the label path must be relative to the cart directory";
  }
  if (labelPath.split("/").some((segment) => segment === "..")) {
    return "the label path must not leave the cart directory";
  }
  return null;
}

/** A client-side rehearsal of the backend check, so the UI can react before the call. */
export function localCheck(dataUrl: string, labelPath: string): ExportCheck {
  const problems: string[] = [];
  const warnings: string[] = [];
  if (!looksLikePng(dataUrl)) problems.push("the exported image is not a PNG");
  const bytes = dataUrlBytes(dataUrl);
  if (bytes > MAX_BYTES) {
    problems.push(`label art is ${bytes} bytes; the manifest caps it at ${MAX_BYTES}`);
  } else if (bytes > WARN_BYTES) {
    warnings.push(`label art is ${bytes} bytes; a cart label wants a few KB, not a photo`);
  }
  const path = pathProblem(labelPath);
  if (path) problems.push(path);
  return { ok: problems.length === 0, bytes, width: null, height: null, problems, warnings };
}

export type Verdict = "ok" | "warn" | "blocked";

export interface Decision {
  verdict: Verdict;
  problems: string[];
  warnings: string[];
  /** Set when a smaller export could plausibly pass; null when nothing is left to try. */
  retry: ExportSettings | null;
}

export function isSizeProblem(problem: string): boolean {
  return problem.includes("bytes");
}

export function nextRecompress(settings: ExportSettings): ExportSettings | null {
  const multipleIndex = MULTIPLES.indexOf(settings.multiple);
  if (multipleIndex > 0) {
    const lower = MULTIPLES[multipleIndex - 1];
    if (lower !== undefined) return { ...settings, multiple: lower };
  }
  if (settings.multiple > 1) return { ...settings, multiple: 1 };
  const quantizeIndex = QUANTIZE_STEPS.indexOf(settings.quantize);
  const next = QUANTIZE_STEPS[quantizeIndex + 1];
  if (quantizeIndex >= 0 && next !== undefined) return { ...settings, quantize: next };
  return null;
}

/** Turn a check into what the UI does next: write, write with a warning, or recompress. */
export function decide(check: ExportCheck, settings: ExportSettings): Decision {
  if (check.ok) {
    return {
      verdict: check.warnings.length > 0 ? "warn" : "ok",
      problems: [],
      warnings: check.warnings,
      retry: null,
    };
  }
  const sizeOnly = check.problems.length > 0 && check.problems.every(isSizeProblem);
  return {
    verdict: "blocked",
    problems: check.problems,
    warnings: check.warnings,
    retry: sizeOnly ? nextRecompress(settings) : null,
  };
}

export function describeSettings(settings: ExportSettings): string {
  const scale = `${settings.multiple}x`;
  return settings.quantize === null ? scale : `${scale}, ${settings.quantize} levels per channel`;
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
}
