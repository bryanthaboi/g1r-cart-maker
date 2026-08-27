// load_order is an array of mod ids. Reordering never invents or drops one.

export function moveItem<T>(list: readonly T[], from: number, to: number): T[] {
  const next = list.slice();
  if (from < 0 || from >= next.length) return next;
  const clamped = Math.max(0, Math.min(next.length - 1, to));
  const [item] = next.splice(from, 1);
  if (item === undefined) return list.slice();
  next.splice(clamped, 0, item);
  return next;
}

/** Fills in unlisted ids in pin order and drops ids that are no longer pinned. */
export function normalizeLoadOrder(order: readonly string[] | undefined, modIds: readonly string[]): string[] {
  const known = new Set(modIds);
  const seen = new Set<string>();
  const out: string[] = [];
  for (const id of order ?? []) {
    if (!known.has(id) || seen.has(id)) continue;
    seen.add(id);
    out.push(id);
  }
  for (const id of modIds) {
    if (seen.has(id)) continue;
    seen.add(id);
    out.push(id);
  }
  return out;
}

export function shiftById(order: readonly string[], id: string, delta: number): string[] {
  const from = order.indexOf(id);
  if (from < 0) return order.slice();
  return moveItem(order, from, from + delta);
}

/** True when the two orders name the same ids in the same positions. */
export function sameOrder(a: readonly string[], b: readonly string[]): boolean {
  if (a.length !== b.length) return false;
  return a.every((id, index) => id === b[index]);
}
