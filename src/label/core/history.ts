// A bounded undo stack. Entries carry a coalesce key so one drag is one undo step.

export interface HistoryEntry<T> {
  state: T;
  label: string;
  coalesceKey: string | null;
}

export interface History<T> {
  past: readonly HistoryEntry<T>[];
  present: HistoryEntry<T>;
  future: readonly HistoryEntry<T>[];
  limit: number;
}

export const DEFAULT_LIMIT = 100;

export function createHistory<T>(state: T, limit = DEFAULT_LIMIT): History<T> {
  return {
    past: [],
    present: { state, label: "open", coalesceKey: null },
    future: [],
    limit: Math.max(1, limit),
  };
}

export interface PushOptions {
  label: string;
  /** Same key as the previous entry replaces it instead of adding a step. */
  coalesceKey?: string | null;
}

export function push<T>(history: History<T>, state: T, options: PushOptions): History<T> {
  const key = options.coalesceKey ?? null;
  if (key !== null && history.present.coalesceKey === key) {
    return {
      ...history,
      present: { state, label: options.label, coalesceKey: key },
      future: [],
    };
  }
  const past = [...history.past, history.present];
  while (past.length > history.limit) past.shift();
  return {
    past,
    present: { state, label: options.label, coalesceKey: key },
    future: [],
    limit: history.limit,
  };
}

/** Close the current coalescing run so the next edit starts a fresh entry. */
export function seal<T>(history: History<T>): History<T> {
  if (history.present.coalesceKey === null) return history;
  return { ...history, present: { ...history.present, coalesceKey: null } };
}

export function canUndo<T>(history: History<T>): boolean {
  return history.past.length > 0;
}

export function canRedo<T>(history: History<T>): boolean {
  return history.future.length > 0;
}

export function undo<T>(history: History<T>): History<T> {
  const previous = history.past[history.past.length - 1];
  if (!previous) return history;
  return {
    past: history.past.slice(0, -1),
    present: { ...previous, coalesceKey: null },
    future: [history.present, ...history.future],
    limit: history.limit,
  };
}

export function redo<T>(history: History<T>): History<T> {
  const next = history.future[0];
  if (!next) return history;
  return {
    past: [...history.past, history.present],
    present: { ...next, coalesceKey: null },
    future: history.future.slice(1),
    limit: history.limit,
  };
}

export function currentState<T>(history: History<T>): T {
  return history.present.state;
}

export function undoLabel<T>(history: History<T>): string | null {
  return history.present.label === "open" && history.past.length === 0 ? null : history.present.label;
}

export function redoLabel<T>(history: History<T>): string | null {
  return history.future[0]?.label ?? null;
}

/** Replace the present state without adding a step; for adopting an external document. */
export function reset<T>(history: History<T>, state: T, label = "open"): History<T> {
  return { past: [], present: { state, label, coalesceKey: null }, future: [], limit: history.limit };
}
