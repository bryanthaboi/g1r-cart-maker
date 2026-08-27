// The label designer. It owns the layer document and the undo history; the shell owns
// persistence, so every committed edit leaves here as a complete LabelDoc.

import { useCallback, useEffect, useMemo, useRef, useState, type ChangeEvent } from "react";
import { api } from "../lib/backend";
import { IpcError } from "../lib/ipc";

const labelIpc = api.label;
import type { Cart, ExportCheck, LabelDoc, LabelTemplate, Layer } from "../lib/types";
import "./designer.css";
import { bundledTemplates } from "./bundledTemplates";
import { normaliseHex } from "./core/colour";
import {
  addLayer,
  blankDoc,
  boxForImage,
  canvasRect,
  duplicateLayers,
  findLayer,
  mapLayer,
  moveInZ,
  newImageLayer,
  newRectLayer,
  newTextLayer,
  normaliseDoc,
  removeLayers,
  reorderLayers,
  serialiseDoc,
  uniqueName,
} from "./core/doc";
import {
  DEFAULT_SETTINGS,
  dataUrlBytes,
  decide,
  localCheck,
  type ExportSettings,
} from "./core/exportGuard";
import { exportDataUrl } from "./core/exportPng";
import { boundsOf, rectOfLayer } from "./core/geometry";
import {
  BitmapCache,
  blobToDataUrl,
  isAcceptedType,
  loadBitmap,
  ACCEPT_ATTRIBUTE,
} from "./core/images";
import {
  canRedo,
  canUndo,
  createHistory,
  currentState,
  push,
  redo,
  redoLabel,
  reset,
  seal,
  undo,
  undoLabel,
  type History,
} from "./core/history";
import {
  alignRects,
  distributeRects,
  nudgeDelta,
  type AlignMode,
  type DistributeMode,
} from "./core/snap";
import {
  docFromTemplate,
  initialDoc,
  resetLayer,
  slotOf,
  templateById,
} from "./core/templates";
import { applyTextSync, factsOf, planTextSync, type CartFacts, type SyncCandidate } from "./core/titleSync";
import { pickImages } from "./importFiles";
import CanvasStage from "./ui/CanvasStage";
import CartPreview from "./ui/CartPreview";
import ExportPanel, { type ExportPhase } from "./ui/ExportPanel";
import Inspector from "./ui/Inspector";
import LayerPanel from "./ui/LayerPanel";
import { selectionBounds, type StageView } from "./ui/stageMath";
import Toolbar from "./ui/Toolbar";

export interface LabelDesignerProps {
  doc: LabelDoc | null;
  cart: Cart;
  labelPath: string;
  dir: string;
  onChange: (doc: LabelDoc) => void;
  onExported: (check: ExportCheck) => void;
}

interface StatusLine {
  text: string;
  error: boolean;
}

const INITIAL_VIEW: StageView = {
  zoom: 1,
  offsetX: 0,
  offsetY: 0,
  showGrid: false,
  showRulers: true,
  snap: true,
};

function messageOf(problem: unknown): string {
  if (problem instanceof IpcError) {
    return problem.detail.length > 0 ? `${problem.message} (${problem.detail})` : problem.message;
  }
  if (problem instanceof Error) return problem.message;
  return String(problem);
}

function isTypingTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  const tag = target.tagName.toLowerCase();
  return tag === "input" || tag === "textarea" || tag === "select" || target.isContentEditable;
}

export default function LabelDesigner(props: LabelDesignerProps): JSX.Element {
  const { cart, dir, labelPath, onChange, onExported } = props;

  const [templates, setTemplates] = useState<readonly LabelTemplate[]>([]);
  const [templatesReady, setTemplatesReady] = useState(false);
  const [history, setHistory] = useState<History<LabelDoc>>(() =>
    createHistory(props.doc ? normaliseDoc(props.doc) : blankDoc(normaliseHex(cart.shell, "#ffffff"))),
  );
  /// The document the PNG on disk was last rendered from.
  const writtenRef = useRef<string | null>(null);
  const historyRef = useRef(history);
  historyRef.current = history;
  const doc = currentState(history);

  const [selection, setSelection] = useState<readonly string[]>([]);
  const [view, setView] = useState<StageView>(INITIAL_VIEW);
  const [fitToken, setFitToken] = useState(0);
  const [redrawToken, setRedrawToken] = useState(0);
  const [status, setStatus] = useState<StatusLine | null>(null);
  const [phase, setPhase] = useState<ExportPhase>({ kind: "idle" });
  const [settings, setSettings] = useState<ExportSettings>(DEFAULT_SETTINGS);
  const [estimate, setEstimate] = useState<number | null>(null);
  const [syncOffer, setSyncOffer] = useState<readonly SyncCandidate[]>([]);

  const emittedRef = useRef<LabelDoc | null>(props.doc);
  const derivedRef = useRef(false);
  const factsRef = useRef<CartFacts>(factsOf(cart));
  const replaceRef = useRef<string | null>(null);
  const fileInputRef = useRef<HTMLInputElement | null>(null);
  const cacheRef = useRef<BitmapCache | null>(null);
  if (cacheRef.current === null) {
    cacheRef.current = new BitmapCache(() => setRedrawToken((token) => token + 1));
  }
  const cache = cacheRef.current;
  const resolve = useMemo(() => cache.resolver(), [cache]);

  const template = useMemo(() => templateById(templates, doc.template), [templates, doc.template]);

  useEffect(() => {
    let alive = true;
    labelIpc
      .templates()
      .then((list) => {
        if (alive) setTemplates(list);
      })
      .catch(() =>
        bundledTemplates()
          .then((list) => {
            if (alive) setTemplates(list);
          })
          .catch((problem: unknown) => {
            if (alive) {
              setStatus({ text: `No label templates available: ${messageOf(problem)}`, error: true });
            }
          }),
      )
      .finally(() => {
        if (alive) setTemplatesReady(true);
      });
    return () => {
      alive = false;
    };
  }, []);

  const adopt = useCallback((next: LabelDoc) => {
    const history = reset(historyRef.current, next);
    historyRef.current = history;
    setHistory(history);
    setSelection([]);
    setFitToken((token) => token + 1);
  }, []);

  const commit = useCallback(
    (next: LabelDoc, label: string, coalesceKey: string | null = null) => {
      const history = push(historyRef.current, next, { label, coalesceKey });
      historyRef.current = history;
      setHistory(history);
      emittedRef.current = next;
      onChange(next);
    },
    [onChange],
  );

  const sealHistory = useCallback(() => {
    const history = seal(historyRef.current);
    if (history === historyRef.current) return;
    historyRef.current = history;
    setHistory(history);
  }, []);

  const travel = useCallback(
    (mode: "undo" | "redo") => {
      const current = historyRef.current;
      const next = mode === "undo" ? undo(current) : redo(current);
      if (next === current) return;
      historyRef.current = next;
      setHistory(next);
      const state = currentState(next);
      emittedRef.current = state;
      setSelection((ids) => ids.filter((id) => state.layers.some((layer) => layer.id === id)));
      onChange(state);
    },
    [onChange],
  );

  // A document handed in from outside replaces ours; our own commits come back as identical.
  useEffect(() => {
    if (!props.doc) return;
    if (props.doc === emittedRef.current) return;
    const incoming = normaliseDoc(props.doc);
    if (serialiseDoc(incoming) === serialiseDoc(currentState(historyRef.current))) {
      emittedRef.current = props.doc;
      return;
    }
    emittedRef.current = props.doc;
    derivedRef.current = true;
    adopt(incoming);
  }, [props.doc, adopt]);

  // A project with no layer document starts from the template for its base game.
  useEffect(() => {
    if (props.doc !== null || derivedRef.current || !templatesReady) return;
    derivedRef.current = true;
    const derived = initialDoc(templates, cart).doc;
    adopt(derived);
    emittedRef.current = derived;
    onChange(derived);
  }, [props.doc, templates, templatesReady, cart, adopt, onChange]);

  // The cart moved on: offer to follow, never rewrite the artwork silently.
  useEffect(() => {
    const next = factsOf(cart);
    const before = factsRef.current;
    if (before.title === next.title && before.base === next.base) return;
    factsRef.current = next;
    const plan = planTextSync(currentState(historyRef.current), before, next);
    if (plan.length > 0) setSyncOffer(plan);
  }, [cart]);

  const selectedLayers = useMemo(
    () => doc.layers.filter((layer) => selection.includes(layer.id)),
    [doc.layers, selection],
  );

  const editableSelection = useMemo(
    () => selectedLayers.filter((layer) => !layer.locked),
    [selectedLayers],
  );

  const placeImage = useCallback(
    async (source: string, name: string) => {
      let width = 0;
      let height = 0;
      try {
        const bitmap = await loadBitmap(source);
        width = bitmap.width;
        height = bitmap.height;
      } catch (problem) {
        setStatus({ text: `Could not read ${name}: ${messageOf(problem)}`, error: true });
        return;
      }
      const current = currentState(historyRef.current);
      const target = replaceRef.current;
      replaceRef.current = null;
      if (target && findLayer(current, target)) {
        commit(
          mapLayer(current, target, (layer) =>
            layer.kind === "image" ? { ...layer, source } : layer,
          ),
          "Replace image",
        );
        setStatus({ text: `Replaced the image on ${findLayer(current, target)?.name ?? "layer"}.`, error: false });
        return;
      }
      const box = boxForImage(current, { width, height });
      const layer = newImageLayer({
        source,
        x: box.x,
        y: box.y,
        width: box.width,
        height: box.height,
        fit: "contain",
        name: uniqueName(current, name),
      });
      commit(addLayer(current, layer), "Add image");
      setSelection([layer.id]);
      setStatus({ text: `Imported ${name} (${width} x ${height}).`, error: false });
    },
    [commit],
  );

  const importFiles = useCallback(
    async (files: readonly File[]) => {
      for (const file of files) {
        if (!isAcceptedType(file.type, file.name)) {
          setStatus({ text: `${file.name} is not a PNG, JPEG or WebP.`, error: true });
          continue;
        }
        try {
          const dataUrl = await blobToDataUrl(file);
          await placeImage(dataUrl, file.name);
        } catch (problem) {
          setStatus({ text: `Could not import ${file.name}: ${messageOf(problem)}`, error: true });
        }
      }
    },
    [placeImage],
  );

  const openPicker = useCallback(async () => {
    const result = await pickImages(false);
    if (result.ok) {
      for (const image of result.images) await placeImage(image.dataUrl, image.name);
      return;
    }
    if (result.fallback) {
      fileInputRef.current?.click();
      return;
    }
    setStatus({ text: result.error, error: true });
  }, [placeImage]);

  const onFileInput = useCallback(
    (event: ChangeEvent<HTMLInputElement>) => {
      const files = [...(event.target.files ?? [])];
      event.target.value = "";
      void importFiles(files);
    },
    [importFiles],
  );

  useEffect(() => {
    const onPaste = (event: ClipboardEvent): void => {
      if (isTypingTarget(event.target)) return;
      const files: File[] = [];
      for (const item of event.clipboardData?.items ?? []) {
        if (item.kind !== "file") continue;
        const file = item.getAsFile();
        if (file && isAcceptedType(file.type, file.name)) files.push(file);
      }
      if (files.length === 0) return;
      event.preventDefault();
      void importFiles(files);
    };
    window.addEventListener("paste", onPaste);
    return () => window.removeEventListener("paste", onPaste);
  }, [importFiles]);

  const deleteSelection = useCallback(() => {
    const current = currentState(historyRef.current);
    const ids = selection.filter((id) => {
      const layer = findLayer(current, id);
      return layer !== null && !layer.locked;
    });
    if (ids.length === 0) return;
    commit(removeLayers(current, ids), ids.length > 1 ? "Delete layers" : "Delete layer");
    setSelection([]);
  }, [commit, selection]);

  const duplicateSelection = useCallback(() => {
    const current = currentState(historyRef.current);
    const result = duplicateLayers(current, selection);
    if (result.ids.length === 0) return;
    commit(result.doc, "Duplicate layer");
    setSelection(result.ids);
  }, [commit, selection]);

  const nudge = useCallback(
    (dx: number, dy: number) => {
      const current = currentState(historyRef.current);
      const ids = editableSelection.map((layer) => layer.id);
      if (ids.length === 0) return;
      const next: LabelDoc = {
        ...current,
        layers: current.layers.map((layer) =>
          ids.includes(layer.id) ? { ...layer, x: layer.x + dx, y: layer.y + dy } : layer,
        ),
      };
      commit(next, "Nudge layer", `nudge:${ids.join(",")}`);
    },
    [commit, editableSelection],
  );

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent): void => {
      if (isTypingTarget(event.target)) return;
      const accel = event.metaKey || event.ctrlKey;
      if (accel && event.key.toLowerCase() === "z") {
        event.preventDefault();
        travel(event.shiftKey ? "redo" : "undo");
        return;
      }
      if (accel && event.key.toLowerCase() === "y") {
        event.preventDefault();
        travel("redo");
        return;
      }
      if (accel && event.key.toLowerCase() === "d") {
        event.preventDefault();
        duplicateSelection();
        return;
      }
      if (accel && event.key.toLowerCase() === "a") {
        event.preventDefault();
        setSelection(currentState(historyRef.current).layers.map((layer) => layer.id));
        return;
      }
      if (event.key === "Escape") {
        setSelection([]);
        return;
      }
      if (event.key === "Delete" || event.key === "Backspace") {
        event.preventDefault();
        deleteSelection();
        return;
      }
      if (event.key === "0") {
        setFitToken((token) => token + 1);
        return;
      }
      const delta = nudgeDelta(event.key, event.shiftKey);
      if (delta) {
        event.preventDefault();
        nudge(delta.dx, delta.dy);
      }
    };
    const onKeyUp = (event: KeyboardEvent): void => {
      if (event.key.startsWith("Arrow")) sealHistory();
    };
    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("keyup", onKeyUp);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("keyup", onKeyUp);
    };
  }, [deleteSelection, duplicateSelection, nudge, sealHistory, travel]);

  const align = useCallback(
    (mode: AlignMode, against: "canvas" | "selection") => {
      const current = currentState(historyRef.current);
      const layers = editableSelection;
      if (layers.length === 0) return;
      const boxes = layers.map((layer) => boundsOf(rectOfLayer(layer), layer.rotation));
      const bounds =
        against === "canvas" ? canvasRect(current) : (selectionBounds(layers) ?? canvasRect(current));
      const aligned = alignRects(boxes, mode, bounds);
      const deltas = new Map<string, { dx: number; dy: number }>();
      layers.forEach((layer, index) => {
        const before = boxes[index];
        const after = aligned[index];
        if (!before || !after) return;
        deltas.set(layer.id, { dx: after.x - before.x, dy: after.y - before.y });
      });
      const next: LabelDoc = {
        ...current,
        layers: current.layers.map((layer) => {
          const delta = deltas.get(layer.id);
          return delta ? { ...layer, x: layer.x + delta.dx, y: layer.y + delta.dy } : layer;
        }),
      };
      commit(next, "Align layers");
    },
    [commit, editableSelection],
  );

  const distribute = useCallback(
    (mode: DistributeMode) => {
      const current = currentState(historyRef.current);
      const layers = editableSelection;
      if (layers.length < 3) return;
      const boxes = layers.map((layer) => boundsOf(rectOfLayer(layer), layer.rotation));
      const spread = distributeRects(boxes, mode);
      const deltas = new Map<string, { dx: number; dy: number }>();
      layers.forEach((layer, index) => {
        const before = boxes[index];
        const after = spread[index];
        if (!before || !after) return;
        deltas.set(layer.id, { dx: after.x - before.x, dy: after.y - before.y });
      });
      const next: LabelDoc = {
        ...current,
        layers: current.layers.map((layer) => {
          const delta = deltas.get(layer.id);
          return delta ? { ...layer, x: layer.x + delta.dx, y: layer.y + delta.dy } : layer;
        }),
      };
      commit(next, "Distribute layers");
    },
    [commit, editableSelection],
  );

  const addText = useCallback(() => {
    const current = currentState(historyRef.current);
    const layer = newTextLayer({
      text: cart.title.trim().length > 0 ? cart.title : "New text",
      x: Math.round(current.width * 0.15),
      y: Math.round(current.height * 0.45),
      width: Math.round(current.width * 0.7),
      height: 56,
      name: uniqueName(current, "Text"),
    });
    commit(addLayer(current, layer), "Add text");
    setSelection([layer.id]);
  }, [cart.title, commit]);

  const addRect = useCallback(() => {
    const current = currentState(historyRef.current);
    const layer = newRectLayer({
      x: Math.round(current.width * 0.3),
      y: Math.round(current.height * 0.4),
      width: Math.round(current.width * 0.4),
      height: 80,
      fill: normaliseHex(cart.shell, "#000000"),
      name: uniqueName(current, "Rectangle"),
    });
    commit(addLayer(current, layer), "Add rectangle");
    setSelection([layer.id]);
  }, [cart.shell, commit]);

  const resetToTemplate = useCallback(
    (id: string) => {
      const current = currentState(historyRef.current);
      const layer = findLayer(current, id);
      if (!layer) return;
      const restored = resetLayer(layer, template, cart);
      if (!restored) {
        setStatus({ text: `${layer.name} did not come from a template.`, error: true });
        return;
      }
      commit(mapLayer(current, id, () => restored), "Reset layer");
    },
    [cart, commit, template],
  );

  const switchTemplate = useCallback(
    (id: string) => {
      const current = currentState(historyRef.current);
      const kept = current.layers.filter((layer) => layer.from_template === null);
      if (id === "blank") {
        commit({ ...current, template: "blank", layers: kept }, "Remove template artwork");
        return;
      }
      const chosen = templateById(templates, id);
      if (!chosen) {
        setStatus({ text: `Template ${id} is not available.`, error: true });
        return;
      }
      const fresh = docFromTemplate(chosen, cart);
      commit({ ...fresh, background: current.background, layers: [...fresh.layers, ...kept] }, "Change template");
      setSelection([]);
    },
    [cart, commit, templates],
  );

  const runExport = useCallback(
    async (chosen: ExportSettings) => {
      setPhase({ kind: "working", note: "Rendering the label..." });
      const current = currentState(historyRef.current);
      try {
        await cache.warm(
          current.layers.filter((layer) => layer.kind === "image").map((layer) => layer.source),
        );
        const dataUrl = exportDataUrl(current, cache.resolver(), chosen);
        const rehearsal = localCheck(dataUrl, labelPath);
        if (!rehearsal.ok) {
          setPhase({ kind: "blocked", check: rehearsal, retry: decide(rehearsal, chosen).retry });
          return;
        }
        setPhase({ kind: "working", note: "Checking it against the manifest limits..." });
        const check = await labelIpc.checkExport(dataUrl, labelPath);
        const decision = decide(check, chosen);
        if (decision.verdict === "blocked") {
          setPhase({ kind: "blocked", check, retry: decision.retry });
          return;
        }
        setPhase({ kind: "working", note: `Writing ${labelPath}...` });
        const written = await labelIpc.writePng(dir, labelPath, dataUrl);
        if (!written.ok) {
          setPhase({ kind: "blocked", check: written, retry: decide(written, chosen).retry });
          return;
        }
        setPhase({ kind: "done", check: written });
        setEstimate(written.bytes);
        onExported(written);
      } catch (problem) {
        setPhase({ kind: "error", message: messageOf(problem) });
      }
    },
    [cache, dir, labelPath, onExported],
  );

  /// Keep label.png in step with the design.
  ///
  /// The layer document saves on every edit, but the PNG the cart actually
  /// ships used to move only when someone pressed Export. A design could look
  /// finished on screen while the cart still carried the scaffold placeholder,
  /// and publishing shipped that. This writes the art a moment after editing
  /// stops; an export that the manifest would refuse is reported, never
  /// silently dropped.
  const autoWrite = useCallback(async () => {
    const current = currentState(historyRef.current);
    try {
      await cache.warm(
        current.layers.filter((layer) => layer.kind === "image").map((layer) => layer.source),
      );
      const dataUrl = exportDataUrl(current, cache.resolver(), DEFAULT_SETTINGS);
      const rehearsal = localCheck(dataUrl, labelPath);
      if (!rehearsal.ok) {
        setStatus({
          text: `${labelPath} is not saved: ${rehearsal.problems.join(" ")}`,
          error: true,
        });
        return;
      }
      const written = await labelIpc.writePng(dir, labelPath, dataUrl);
      if (!written.ok) {
        setStatus({ text: `${labelPath} is not saved: ${written.problems.join(" ")}`, error: true });
        return;
      }
      setEstimate(written.bytes);
      onExported(written);
    } catch (problem) {
      setStatus({ text: `Could not save ${labelPath}: ${messageOf(problem)}`, error: true });
    }
  }, [cache, dir, labelPath, onExported]);

  const autoWriteRef = useRef(autoWrite);
  autoWriteRef.current = autoWrite;

  useEffect(() => {
    if (serialiseDoc(doc) === writtenRef.current) return;
    const timer = window.setTimeout(() => {
      writtenRef.current = serialiseDoc(doc);
      void autoWriteRef.current();
    }, 900);
    return () => window.clearTimeout(timer);
  }, [doc]);

  const runEstimate = useCallback(async () => {
    const current = currentState(historyRef.current);
    try {
      await cache.warm(
        current.layers.filter((layer) => layer.kind === "image").map((layer) => layer.source),
      );
      setEstimate(dataUrlBytes(exportDataUrl(current, cache.resolver(), settings)));
    } catch (problem) {
      setStatus({ text: `Could not render the label: ${messageOf(problem)}`, error: true });
    }
  }, [cache, settings]);

  const selectLayer = useCallback((id: string, additive: boolean) => {
    setSelection((ids) => {
      if (!additive) return [id];
      return ids.includes(id) ? ids.filter((entry) => entry !== id) : [...ids, id];
    });
  }, []);

  const canReset = useCallback(
    (layer: Layer) => slotOf(layer) !== null,
    [],
  );

  return (
    <div className="ld-root">
      <div className="ld-column ld-column--left">
        <div className="ld-section">
          <h3 className="ld-title">Layers</h3>
        </div>
        <div className="ld-section ld-section--grow">
          <LayerPanel
            doc={doc}
            selection={selection}
            onSelect={(id, additive) => selectLayer(id, additive)}
            onRename={(id, name) =>
              commit(
                mapLayer(doc, id, (layer) => ({ ...layer, name })),
                "Rename layer",
              )
            }
            onToggleHidden={(id) =>
              commit(
                mapLayer(doc, id, (layer) => ({ ...layer, hidden: !layer.hidden })),
                "Toggle visibility",
              )
            }
            onToggleLocked={(id) =>
              commit(
                mapLayer(doc, id, (layer) => ({ ...layer, locked: !layer.locked })),
                "Toggle lock",
              )
            }
            onReorder={(ids, target) => commit(reorderLayers(doc, ids, target), "Reorder layers")}
            onReset={resetToTemplate}
            canReset={canReset}
          />
        </div>
        <div className="ld-section">
          <CartPreview
            doc={doc}
            resolve={resolve}
            shell={cart.shell}
            finish={cart.finish ?? null}
            redrawToken={redrawToken}
          />
        </div>
      </div>

      <div className="ld-centre">
        <Toolbar
          view={view}
          onView={setView}
          onFit={() => setFitToken((token) => token + 1)}
          canUndo={canUndo(history)}
          canRedo={canRedo(history)}
          undoLabel={undoLabel(history)}
          redoLabel={redoLabel(history)}
          onUndo={() => travel("undo")}
          onRedo={() => travel("redo")}
          onAddText={addText}
          onAddRect={addRect}
          onImport={() => void openPicker()}
          onDuplicate={duplicateSelection}
          onDelete={deleteSelection}
          onRaise={() => commit(moveInZ(doc, selection, "forward"), "Bring forward")}
          onLower={() => commit(moveInZ(doc, selection, "backward"), "Send backward")}
          hasSelection={selection.length > 0}
          templates={templates}
          templateId={doc.template}
          onTemplate={switchTemplate}
        />

        {syncOffer.length > 0 ? (
          <div className="ld-banner">
            <strong>The cart changed. Update the label text to match?</strong>
            <ul>
              {syncOffer.map((candidate) => (
                <li key={candidate.layerId}>
                  {candidate.layerName}: &ldquo;{candidate.current}&rdquo; becomes &ldquo;
                  {candidate.next}&rdquo;
                </li>
              ))}
            </ul>
            <div className="ld-banner__actions">
              <button
                type="button"
                className="ld-button ld-button--primary"
                onClick={() => {
                  commit(applyTextSync(currentState(historyRef.current), syncOffer), "Follow the cart");
                  setSyncOffer([]);
                }}
              >
                Update {syncOffer.length === 1 ? "the layer" : `${syncOffer.length} layers`}
              </button>
              <button type="button" className="ld-button" onClick={() => setSyncOffer([])}>
                Leave the label as it is
              </button>
            </div>
          </div>
        ) : null}

        <CanvasStage
          doc={doc}
          selection={selection}
          resolve={resolve}
          view={view}
          onView={setView}
          onSelection={setSelection}
          onEdit={commit}
          onSeal={sealHistory}
          onImport={(files) => void importFiles(files)}
          fitToken={fitToken}
          redrawToken={redrawToken}
        />

        <div className="ld-bar ld-bar--footer">
          <span className="ld-status">
            {doc.width} x {doc.height} px
            {selectedLayers.length > 0 ? ` - ${selectedLayers.length} selected` : ""}
          </span>
          <div className="ld-spacer" />
          <span className={`ld-status${status?.error ? " ld-status--error" : ""}`}>
            {status?.text ?? "Drag, paste or import a PNG, JPEG or WebP. Arrow keys nudge, shift for ten."}
          </span>
        </div>
      </div>

      <div className="ld-column ld-column--right">
        <div className="ld-section ld-section--grow">
          <Inspector
            doc={doc}
            selection={selection}
            onEdit={commit}
            onReplaceImage={(id) => {
              replaceRef.current = id;
              void openPicker();
            }}
            onFitBoxToImage={(id) => {
              const layer = findLayer(doc, id);
              if (!layer || layer.kind !== "image") return;
              const bitmap = cache.get(layer.source);
              if (!bitmap) {
                setStatus({ text: "That image is still decoding.", error: true });
                return;
              }
              const box = boxForImage(doc, { width: bitmap.width, height: bitmap.height });
              commit(
                mapLayer(doc, id, (entry) => ({ ...entry, width: box.width, height: box.height })),
                "Fit box to image",
              );
            }}
            onAlign={align}
            onDistribute={distribute}
          />
          <ExportPanel
            labelPath={labelPath}
            settings={settings}
            phase={phase}
            estimate={estimate}
            onSettings={(next) => {
              setSettings(next);
              setEstimate(null);
            }}
            onExport={() => void runExport(settings)}
            onRetry={(next) => {
              setSettings(next);
              void runExport(next);
            }}
            onEstimate={() => void runEstimate()}
          />
        </div>
      </div>

      <input
        ref={fileInputRef}
        className="ld-hidden-input"
        type="file"
        accept={ACCEPT_ATTRIBUTE}
        onChange={onFileInput}
      />
    </div>
  );
}
