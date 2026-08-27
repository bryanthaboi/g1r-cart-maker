// Option row metadata drives the editor. Never hard-code a mod's options here.

import { LIMITS } from "./constants";
import type { OptionRow, OptionValue, VisibleIf } from "./types";

function sameValue(a: OptionValue | undefined, b: OptionValue | null | undefined): boolean {
  if (a === undefined || b === undefined || b === null) return false;
  if (typeof a === typeof b) return a === b;
  return String(a) === String(b);
}

/** A row with no visible_if is always shown; an unset dependency reads as absent. */
export function isRowVisible(condition: VisibleIf | null | undefined, values: Record<string, OptionValue>): boolean {
  if (!condition) return true;
  const current = values[condition.key];
  if (condition.equals !== undefined && condition.equals !== null) {
    return sameValue(current, condition.equals);
  }
  if (condition.not_equals !== undefined && condition.not_equals !== null) {
    return !sameValue(current, condition.not_equals);
  }
  return true;
}

export function visibleRows(rows: OptionRow[], values: Record<string, OptionValue>): OptionRow[] {
  return rows.filter((row) => isRowVisible(row.visible_if, values));
}

export function defaultsFor(rows: OptionRow[]): Record<string, OptionValue> {
  const out: Record<string, OptionValue> = {};
  for (const row of rows) out[row.key] = row.default;
  return out;
}

/** Values are merged over defaults so a row the author never touched still reads. */
export function withDefaults(rows: OptionRow[], values: Record<string, OptionValue>): Record<string, OptionValue> {
  return { ...defaultsFor(rows), ...values };
}

export function coerceForRow(row: OptionRow, raw: string): OptionValue {
  switch (row.type) {
    case "toggle":
      return raw === "true" || raw === "1" || raw === "on";
    case "number": {
      const parsed = Number(raw);
      if (!Number.isFinite(parsed)) return row.default;
      const min = row.min ?? null;
      const max = row.max ?? null;
      let value = parsed;
      if (min !== null && value < min) value = min;
      if (max !== null && value > max) value = max;
      return value;
    }
    case "choice": {
      const match = row.choices.find(([, value]) => String(value) === raw);
      return match ? match[1] : row.default;
    }
    case "text": {
      const cap = Math.min(row.maxLen ?? LIMITS.optionText, LIMITS.optionText);
      return raw.slice(0, cap);
    }
  }
}

/** Parse a raw key/value pair the way cartkit pin --option k=v does. */
export function parseRawOption(text: string): { key: string; value: OptionValue } | { error: string } {
  const at = text.indexOf("=");
  if (at <= 0) return { error: "Write it as key=value." };
  const key = text.slice(0, at).trim();
  const raw = text.slice(at + 1);
  if (key.length === 0) return { error: "The key is empty." };
  if (key.length > LIMITS.optionKey) return { error: `A key is at most ${LIMITS.optionKey} characters.` };
  if (raw.length > LIMITS.optionText) return { error: `A value is at most ${LIMITS.optionText} characters.` };
  return { key, value: parseScalar(raw) };
}

export function parseScalar(raw: string): OptionValue {
  const trimmed = raw.trim();
  if (trimmed === "true") return true;
  if (trimmed === "false") return false;
  if (trimmed.length > 0 && /^-?\d+(\.\d+)?$/.test(trimmed)) {
    const parsed = Number(trimmed);
    if (Number.isFinite(parsed)) return parsed;
  }
  return raw;
}

export function formatScalar(value: OptionValue): string {
  if (typeof value === "boolean") return value ? "true" : "false";
  return String(value);
}

export function optionCountProblem(values: Record<string, OptionValue>): string | null {
  const count = Object.keys(values).length;
  if (count > LIMITS.options) return `A pin sets at most ${LIMITS.options} options; this one sets ${count}.`;
  return null;
}
