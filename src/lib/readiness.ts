// Index readiness: a blocking item stops the cart being listable, the rest are
// recommendations.

import type { ReadinessItem } from "./types";

export interface ReadinessSummary {
  blocking: ReadinessItem[];
  recommended: ReadinessItem[];
  met: ReadinessItem[];
  listable: boolean;
  metCount: number;
  total: number;
}

export function summarizeReadiness(items: readonly ReadinessItem[]): ReadinessSummary {
  const blocking: ReadinessItem[] = [];
  const recommended: ReadinessItem[] = [];
  const met: ReadinessItem[] = [];
  for (const item of items) {
    if (item.ok) met.push(item);
    else if (item.blocking) blocking.push(item);
    else recommended.push(item);
  }
  return {
    blocking,
    recommended,
    met,
    listable: blocking.length === 0,
    metCount: met.length,
    total: items.length,
  };
}

export function readinessHeadline(summary: ReadinessSummary): string {
  if (summary.total === 0) return "Nothing to check yet.";
  if (summary.listable && summary.recommended.length === 0) return "Ready to be listed in the index.";
  if (summary.listable) {
    const count = summary.recommended.length;
    return `Listable. ${count} recommended item${count === 1 ? "" : "s"} still open.`;
  }
  const count = summary.blocking.length;
  return `${count} item${count === 1 ? "" : "s"} must be fixed before the index will list this cart.`;
}
