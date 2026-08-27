// "The cart title changed - update the label?" This decides what would change; the
// user always confirms. Locked and hidden layers are never rewritten.

import type { Base, Cart, LabelDoc, Layer } from "../../lib/types";
import { BASE_SLOT, TITLE_SLOT, baseLine, slotOf } from "./templates";

export interface CartFacts {
  title: string;
  base: Base;
}

export type SyncReason = "title" | "base";

export interface SyncCandidate {
  layerId: string;
  layerName: string;
  reason: SyncReason;
  current: string;
  next: string;
}

export function factsOf(cart: Cart): CartFacts {
  return { title: cart.title, base: cart.base };
}

function sameText(a: string, b: string): boolean {
  return a.trim().toLowerCase() === b.trim().toLowerCase();
}

function editable(layer: Layer): layer is Extract<Layer, { kind: "text" }> {
  return layer.kind === "text" && !layer.locked;
}

/**
 * A layer is a candidate when it fills the matching template slot, or when its text
 * still reads exactly as the old value, which means the user never retyped it.
 */
export function planTextSync(doc: LabelDoc, before: CartFacts, after: CartFacts): SyncCandidate[] {
  const candidates: SyncCandidate[] = [];
  const titleChanged = before.title !== after.title;
  const baseChanged = before.base !== after.base;
  if (!titleChanged && !baseChanged) return candidates;

  for (const layer of doc.layers) {
    if (!editable(layer)) continue;
    const slot = slotOf(layer);
    if (titleChanged && (slot === TITLE_SLOT || sameText(layer.text, before.title))) {
      if (layer.text !== after.title) {
        candidates.push({
          layerId: layer.id,
          layerName: layer.name,
          reason: "title",
          current: layer.text,
          next: after.title,
        });
      }
      continue;
    }
    if (baseChanged) {
      const wasLine = baseLine(before.base);
      const nextLine = baseLine(after.base);
      if ((slot === BASE_SLOT || sameText(layer.text, wasLine)) && layer.text !== nextLine) {
        candidates.push({
          layerId: layer.id,
          layerName: layer.name,
          reason: "base",
          current: layer.text,
          next: nextLine,
        });
      }
    }
  }
  return candidates;
}

export function applyTextSync(doc: LabelDoc, candidates: readonly SyncCandidate[]): LabelDoc {
  if (candidates.length === 0) return doc;
  const wanted = new Map(candidates.map((candidate) => [candidate.layerId, candidate.next]));
  return {
    ...doc,
    layers: doc.layers.map((layer) => {
      const next = wanted.get(layer.id);
      if (next === undefined || layer.kind !== "text") return layer;
      return { ...layer, text: next };
    }),
  };
}
