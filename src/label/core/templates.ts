// Deriving a starting document from a shipped template. Slot ids in `from_template`
// are what a reset and a title sync look for, so they are part of the saved format.

import { BASE_LABELS } from "../../lib/constants";
import type { Base, Cart, LabelDoc, LabelTemplate, Layer } from "../../lib/types";
import { inkFor, normaliseHex } from "./colour";
import {
  CANVAS_HEIGHT,
  CANVAS_WIDTH,
  blankDoc,
  canvasFor,
  newImageLayer,
  newTextLayer,
  type CanvasSize,
  type TextLayer,
} from "./doc";

export const ART_SLOT = "art";
export const TITLE_SLOT = "title";
export const BASE_SLOT = "base";

export function slotOf(layer: Layer): string | null {
  if (!layer.from_template) return null;
  const parts = layer.from_template.split(":");
  return parts.length > 1 ? (parts[parts.length - 1] ?? null) : null;
}

export function slotRef(templateId: string, slot: string): string {
  return `${templateId}:${slot}`;
}

export function baseName(base: Base): string {
  return BASE_LABELS[base] ?? base;
}

export function baseLine(base: Base): string {
  return `${baseName(base)} version`;
}

export function templateForBase(
  templates: readonly LabelTemplate[],
  base: Base,
): LabelTemplate | null {
  return (
    templates.find((template) => template.base === base) ??
    templates.find((template) => template.id === base) ??
    null
  );
}

export function templateById(
  templates: readonly LabelTemplate[],
  id: string,
): LabelTemplate | null {
  return templates.find((template) => template.id === id) ?? null;
}

function titleText(cart: Cart): string {
  return cart.title.trim().length > 0 ? cart.title.trim() : cart.id;
}

interface TextSlot {
  x: number;
  y: number;
  width: number;
  height: number;
  size: number;
}

export function textSlots(size: CanvasSize): { title: TextSlot; base: TextSlot } {
  if (size.width > size.height * 1.5) {
    const x = Math.round(size.width * 0.2);
    const width = size.width - x - Math.round(size.width * 0.05);
    const unit = size.height / 260;
    return {
      title: { x, y: Math.round(156 * unit), width, height: Math.round(50 * unit), size: Math.round(34 * unit) },
      base: { x, y: Math.round(210 * unit), width, height: Math.round(30 * unit), size: Math.round(18 * unit) },
    };
  }
  return {
    title: { x: 42, y: 196, width: size.width - 84, height: 62, size: 40 },
    base: { x: 42, y: 264, width: size.width - 84, height: 32, size: 21 },
  };
}

function sizeOf(template: LabelTemplate | null, cart: Cart): CanvasSize {
  if (template && template.width > 0 && template.height > 0) {
    return { width: template.width, height: template.height };
  }
  return canvasFor(cart.base);
}

/** The generated text layers: a title band and a base-game line, both resettable. */
export function titleLayer(
  templateId: string,
  cart: Cart,
  ink: string,
  size: CanvasSize = canvasFor(cart.base),
): TextLayer {
  const slot = textSlots(size).title;
  const layer = newTextLayer({
    text: titleText(cart),
    x: slot.x,
    y: slot.y,
    width: slot.width,
    height: slot.height,
    size: slot.size,
    colour: ink,
    align: "center",
    name: "Cart title",
    fromTemplate: slotRef(templateId, TITLE_SLOT),
    weight: "800",
    stroke: ink === "#ffffff" ? "#1b1b1b" : "#ffffff",
    strokeWidth: 6,
  });
  return layer;
}

export function baseLayer(
  templateId: string,
  cart: Cart,
  ink: string,
  size: CanvasSize = canvasFor(cart.base),
): TextLayer {
  const slot = textSlots(size).base;
  return newTextLayer({
    text: baseLine(cart.base),
    x: slot.x,
    y: slot.y,
    width: slot.width,
    height: slot.height,
    size: slot.size,
    colour: ink,
    align: "center",
    name: "Base game",
    fromTemplate: slotRef(templateId, BASE_SLOT),
    weight: "600",
    stroke: ink === "#ffffff" ? "#1b1b1b" : "#ffffff",
    strokeWidth: 4,
  });
}

export function artLayer(template: LabelTemplate): Layer {
  const layer = newImageLayer({
    source: template.dataUrl,
    x: 0,
    y: 0,
    width: template.width || CANVAS_WIDTH,
    height: template.height || CANVAS_HEIGHT,
    fit: "cover",
    name: `${template.name} artwork`,
    fromTemplate: slotRef(template.id, ART_SLOT),
  });
  return { ...layer, locked: false };
}

export function docFromTemplate(template: LabelTemplate, cart: Cart): LabelDoc {
  const ink = "#ffffff";
  const size = sizeOf(template, cart);
  const doc = blankDoc(normaliseHex(cart.shell, "#ffffff"), template.id, size);
  return {
    ...doc,
    layers: [
      artLayer(template),
      titleLayer(template.id, cart, ink, size),
      baseLayer(template.id, cart, ink, size),
    ],
  };
}

/** No template available: a shell-coloured card with the same two text slots. */
export function docFromBlank(cart: Cart): LabelDoc {
  const background = normaliseHex(cart.shell, "#d0d4da");
  const ink = inkFor(background);
  const size = canvasFor(cart.base);
  const doc = blankDoc(background, "blank", size);
  return {
    ...doc,
    layers: [titleLayer("blank", cart, ink, size), baseLayer("blank", cart, ink, size)],
  };
}

export function initialDoc(
  templates: readonly LabelTemplate[],
  cart: Cart,
): { doc: LabelDoc; template: LabelTemplate | null } {
  const template = templateForBase(templates, cart.base);
  if (!template) return { doc: docFromBlank(cart), template: null };
  return { doc: docFromTemplate(template, cart), template };
}

/** Rebuild one template-derived layer, keeping its id and its place in the stack. */
export function resetLayer(
  layer: Layer,
  template: LabelTemplate | null,
  cart: Cart,
): Layer | null {
  const slot = slotOf(layer);
  if (!slot) return null;
  const templateId = template?.id ?? "blank";
  const ink = template ? "#ffffff" : inkFor(normaliseHex(cart.shell, "#d0d4da"));
  switch (slot) {
    case ART_SLOT:
      return template ? { ...artLayer(template), id: layer.id, name: layer.name } : null;
    case TITLE_SLOT:
      return { ...titleLayer(templateId, cart, ink, sizeOf(template, cart)), id: layer.id, name: layer.name };
    case BASE_SLOT:
      return { ...baseLayer(templateId, cart, ink, sizeOf(template, cart)), id: layer.id, name: layer.name };
    default:
      return null;
  }
}
