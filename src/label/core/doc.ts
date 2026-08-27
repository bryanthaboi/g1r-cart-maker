// The label document: creation, normalisation and layer edits. This is the on-disk
// format the Rust side validates, so nothing here may invent a field or a kind.

import type { FitMode, LabelDoc, Layer, TextAlign } from "../../lib/types";
import type { Rect } from "./geometry";

export const DOC_SCHEMA = 1;
export const CANVAS_WIDTH = 500;
export const CANVAS_HEIGHT = 441;

export type ImageLayer = Extract<Layer, { kind: "image" }>;
export type TextLayer = Extract<Layer, { kind: "text" }>;
export type RectLayer = Extract<Layer, { kind: "rect" }>;
export type LayerKind = Layer["kind"];

export const FIT_MODES: readonly FitMode[] = ["contain", "cover", "crop", "scale", "stretch"];
export const TEXT_ALIGNS: readonly TextAlign[] = ["left", "center", "right"];

let idCounter = 0;

/** Layer ids only need to be unique inside one document. */
export function makeLayerId(prefix: string): string {
  idCounter += 1;
  const random = Math.floor(Math.random() * 0xffffff)
    .toString(16)
    .padStart(6, "0");
  return `${prefix}-${idCounter.toString(36)}${random}`;
}

export function blankDoc(background = "#ffffff", template = "blank"): LabelDoc {
  return {
    schema: DOC_SCHEMA,
    width: CANVAS_WIDTH,
    height: CANVAS_HEIGHT,
    template,
    background,
    layers: [],
  };
}

export function cloneDoc(doc: LabelDoc): LabelDoc {
  return { ...doc, layers: doc.layers.map((layer) => ({ ...layer })) };
}

export function canvasRect(doc: LabelDoc): Rect {
  return { x: 0, y: 0, width: doc.width, height: doc.height };
}

function num(value: unknown, fallback: number): number {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

function str(value: unknown, fallback: string): string {
  return typeof value === "string" ? value : fallback;
}

function bool(value: unknown): boolean {
  return value === true;
}

function optNum(value: unknown): number | null | undefined {
  if (value === undefined) return undefined;
  if (value === null) return null;
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function optStr(value: unknown): string | null | undefined {
  if (value === undefined) return undefined;
  if (value === null) return null;
  return typeof value === "string" ? value : null;
}

function pickFit(value: unknown): FitMode {
  return FIT_MODES.includes(value as FitMode) ? (value as FitMode) : "contain";
}

function pickAlign(value: unknown): TextAlign {
  return TEXT_ALIGNS.includes(value as TextAlign) ? (value as TextAlign) : "center";
}

type Bag = Record<string, unknown>;

function baseOf(raw: Bag, index: number): Omit<Layer, "kind"> & Bag {
  const from = optStr(raw.from_template);
  return {
    id: str(raw.id, makeLayerId("layer")),
    name: str(raw.name, `Layer ${index + 1}`),
    x: num(raw.x, 0),
    y: num(raw.y, 0),
    width: Math.max(1, num(raw.width, 100)),
    height: Math.max(1, num(raw.height, 100)),
    rotation: num(raw.rotation, 0),
    hidden: bool(raw.hidden),
    locked: bool(raw.locked),
    from_template: from === undefined ? null : from,
  };
}

function withDefined<T extends Bag>(value: T): T {
  const output: Bag = {};
  for (const [key, entry] of Object.entries(value)) {
    if (entry !== undefined) output[key] = entry;
  }
  return output as T;
}

export function normaliseLayer(raw: unknown, index: number): Layer | null {
  if (!raw || typeof raw !== "object") return null;
  const bag = raw as Bag;
  const base = baseOf(bag, index);
  switch (bag.kind) {
    case "image":
      return withDefined({
        ...base,
        kind: "image",
        source: str(bag.source, ""),
        fit: pickFit(bag.fit),
        opacity: optNum(bag.opacity),
      }) as ImageLayer;
    case "text":
      return withDefined({
        ...base,
        kind: "text",
        text: str(bag.text, ""),
        font: str(bag.font, "system-ui, sans-serif"),
        size: Math.max(1, num(bag.size, 24)),
        colour: str(bag.colour, "#000000"),
        align: pickAlign(bag.align),
        weight: optStr(bag.weight),
        letter_spacing: optNum(bag.letter_spacing),
        line_height: optNum(bag.line_height),
        stroke: optStr(bag.stroke),
        stroke_width: optNum(bag.stroke_width),
      }) as TextLayer;
    case "rect":
      return withDefined({
        ...base,
        kind: "rect",
        fill: str(bag.fill, "#000000"),
        radius: optNum(bag.radius),
        stroke: optStr(bag.stroke),
        stroke_width: optNum(bag.stroke_width),
      }) as RectLayer;
    default:
      return null;
  }
}

export function normaliseDoc(raw: unknown): LabelDoc {
  const bag = (raw && typeof raw === "object" ? raw : {}) as Bag;
  const layers = Array.isArray(bag.layers) ? bag.layers : [];
  return {
    schema: DOC_SCHEMA,
    width: Math.max(1, Math.round(num(bag.width, CANVAS_WIDTH))),
    height: Math.max(1, Math.round(num(bag.height, CANVAS_HEIGHT))),
    template: str(bag.template, "blank"),
    background: str(bag.background, "#ffffff"),
    layers: layers
      .map((layer, index) => normaliseLayer(layer, index))
      .filter((layer): layer is Layer => layer !== null),
  };
}

export function serialiseDoc(doc: LabelDoc): string {
  return `${JSON.stringify(doc, null, 2)}\n`;
}

export type ParseResult = { ok: true; doc: LabelDoc } | { ok: false; error: string };

export function parseDoc(body: string): ParseResult {
  let raw: unknown;
  try {
    raw = JSON.parse(body);
  } catch (problem) {
    return { ok: false, error: `label document unreadable: ${String(problem)}` };
  }
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) {
    return { ok: false, error: "label document is not an object" };
  }
  const schema = (raw as Bag).schema;
  if (typeof schema === "number" && schema > DOC_SCHEMA) {
    return { ok: false, error: `label document schema ${schema} is newer than this app reads` };
  }
  return { ok: true, doc: normaliseDoc(raw) };
}

export function findLayer(doc: LabelDoc, id: string): Layer | null {
  return doc.layers.find((layer) => layer.id === id) ?? null;
}

export function mapLayer(doc: LabelDoc, id: string, fn: (layer: Layer) => Layer): LabelDoc {
  return { ...doc, layers: doc.layers.map((layer) => (layer.id === id ? fn(layer) : layer)) };
}

export function mapLayers(
  doc: LabelDoc,
  ids: readonly string[],
  fn: (layer: Layer) => Layer,
): LabelDoc {
  const wanted = new Set(ids);
  return { ...doc, layers: doc.layers.map((layer) => (wanted.has(layer.id) ? fn(layer) : layer)) };
}

export function removeLayers(doc: LabelDoc, ids: readonly string[]): LabelDoc {
  const wanted = new Set(ids);
  return { ...doc, layers: doc.layers.filter((layer) => !wanted.has(layer.id)) };
}

export function addLayer(doc: LabelDoc, layer: Layer, atTop = true): LabelDoc {
  return { ...doc, layers: atTop ? [...doc.layers, layer] : [layer, ...doc.layers] };
}

export function uniqueName(doc: LabelDoc, wanted: string): string {
  const taken = new Set(doc.layers.map((layer) => layer.name));
  if (!taken.has(wanted)) return wanted;
  let counter = 2;
  while (taken.has(`${wanted} ${counter}`)) counter += 1;
  return `${wanted} ${counter}`;
}

export function duplicateLayers(doc: LabelDoc, ids: readonly string[]): { doc: LabelDoc; ids: string[] } {
  const made: string[] = [];
  let next = doc;
  for (const id of ids) {
    const layer = findLayer(next, id);
    if (!layer) continue;
    const copy: Layer = {
      ...layer,
      id: makeLayerId(layer.kind),
      name: uniqueName(next, `${layer.name} copy`),
      x: layer.x + 8,
      y: layer.y + 8,
      locked: false,
    };
    made.push(copy.id);
    next = addLayer(next, copy);
  }
  return { doc: next, ids: made };
}

/** Move a set of layers so the first of them lands at `target` in document order. */
export function reorderLayers(doc: LabelDoc, ids: readonly string[], target: number): LabelDoc {
  const wanted = new Set(ids);
  const moving = doc.layers.filter((layer) => wanted.has(layer.id));
  if (moving.length === 0) return doc;
  const rest = doc.layers.filter((layer) => !wanted.has(layer.id));
  const before = doc.layers.slice(0, target).filter((layer) => !wanted.has(layer.id)).length;
  const index = Math.max(0, Math.min(rest.length, before));
  return { ...doc, layers: [...rest.slice(0, index), ...moving, ...rest.slice(index)] };
}

export type ZMove = "front" | "back" | "forward" | "backward";

export function moveInZ(doc: LabelDoc, ids: readonly string[], move: ZMove): LabelDoc {
  const wanted = new Set(ids);
  const indexes = doc.layers
    .map((layer, index) => (wanted.has(layer.id) ? index : -1))
    .filter((index) => index >= 0);
  if (indexes.length === 0) return doc;
  const lowest = Math.min(...indexes);
  const highest = Math.max(...indexes);
  switch (move) {
    case "front":
      return reorderLayers(doc, ids, doc.layers.length);
    case "back":
      return reorderLayers(doc, ids, 0);
    case "forward":
      return reorderLayers(doc, ids, Math.min(doc.layers.length, highest + 2));
    case "backward":
      return reorderLayers(doc, ids, Math.max(0, lowest - 1));
    default: {
      const exhaustive: never = move;
      throw new Error(`unhandled z move ${String(exhaustive)}`);
    }
  }
}

export interface NewTextOptions {
  text: string;
  x: number;
  y: number;
  width: number;
  height: number;
  size?: number;
  colour?: string;
  font?: string;
  align?: TextAlign;
  name?: string;
  fromTemplate?: string | null;
  weight?: string | null;
  stroke?: string | null;
  strokeWidth?: number | null;
}

export function newTextLayer(options: NewTextOptions): TextLayer {
  return withDefined({
    id: makeLayerId("text"),
    name: options.name ?? "Text",
    x: options.x,
    y: options.y,
    width: options.width,
    height: options.height,
    rotation: 0,
    hidden: false,
    locked: false,
    from_template: options.fromTemplate ?? null,
    kind: "text" as const,
    text: options.text,
    font: options.font ?? "system-ui, -apple-system, Segoe UI, Roboto, sans-serif",
    size: options.size ?? 28,
    colour: options.colour ?? "#111111",
    align: options.align ?? "center",
    weight: options.weight ?? "700",
    letter_spacing: 0,
    line_height: 1.2,
    stroke: options.stroke ?? null,
    stroke_width: options.strokeWidth ?? null,
  }) as TextLayer;
}

export interface NewImageOptions {
  source: string;
  x: number;
  y: number;
  width: number;
  height: number;
  fit?: FitMode;
  name?: string;
  fromTemplate?: string | null;
}

export function newImageLayer(options: NewImageOptions): ImageLayer {
  return {
    id: makeLayerId("image"),
    name: options.name ?? "Image",
    x: options.x,
    y: options.y,
    width: options.width,
    height: options.height,
    rotation: 0,
    hidden: false,
    locked: false,
    from_template: options.fromTemplate ?? null,
    kind: "image",
    source: options.source,
    fit: options.fit ?? "contain",
    opacity: 1,
  };
}

export interface NewRectOptions {
  x: number;
  y: number;
  width: number;
  height: number;
  fill: string;
  radius?: number;
  name?: string;
}

export function newRectLayer(options: NewRectOptions): RectLayer {
  return {
    id: makeLayerId("rect"),
    name: options.name ?? "Rectangle",
    x: options.x,
    y: options.y,
    width: options.width,
    height: options.height,
    rotation: 0,
    hidden: false,
    locked: false,
    from_template: null,
    kind: "rect",
    fill: options.fill,
    radius: options.radius ?? 0,
    stroke: null,
    stroke_width: null,
  };
}

/** A layer box that keeps a bitmap's aspect ratio inside the canvas. */
export function boxForImage(doc: LabelDoc, natural: { width: number; height: number }): Rect {
  const maxWidth = doc.width * 0.8;
  const maxHeight = doc.height * 0.8;
  const scale = Math.min(1, maxWidth / Math.max(1, natural.width), maxHeight / Math.max(1, natural.height));
  const width = Math.max(8, Math.round(natural.width * scale));
  const height = Math.max(8, Math.round(natural.height * scale));
  return {
    x: Math.round((doc.width - width) / 2),
    y: Math.round((doc.height - height) / 2),
    width,
    height,
  };
}
