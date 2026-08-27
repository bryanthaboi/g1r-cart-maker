// The editing surface: draws the document, the selection overlay and the guides,
// and turns pointer drags into committed document edits.

import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type DragEvent as ReactDragEvent,
  type PointerEvent as ReactPointerEvent,
  type WheelEvent as ReactWheelEvent,
} from "react";
import type { LabelDoc, Layer } from "../../lib/types";
import { canvasRect, findLayer } from "../core/doc";
import {
  HANDLES,
  boundsOf,
  centreOf,
  handlePoints,
  rectOfLayer,
  type HandleId,
  type Point,
  type Rect,
} from "../core/geometry";
import { normaliseRect, pickBelow, pickInMarquee, pickLayer } from "../core/hittest";
import { isAcceptedType } from "../core/images";
import { drawDoc, type ImageResolver } from "../core/render";
import { snapMove, type Guide } from "../core/snap";
import { readColours } from "./tokens";
import {
  RULER_SIZE,
  angleBetween,
  fitView,
  normaliseAngle,
  resizeRotated,
  selectionBounds,
  toDoc,
  toScreen,
  zoomAt,
  type StageView,
} from "./stageMath";

const HANDLE_SIZE = 9;
const HANDLE_GRAB = 7;
const ROTATE_OFFSET = 24;
const SNAP_THRESHOLD = 6;

export interface CanvasStageProps {
  doc: LabelDoc;
  selection: readonly string[];
  resolve: ImageResolver;
  view: StageView;
  onView: (view: StageView) => void;
  onSelection: (ids: string[]) => void;
  onEdit: (doc: LabelDoc, label: string, coalesceKey: string | null) => void;
  onSeal: () => void;
  onImport: (files: readonly File[]) => void;
  /** Changing this number refits the document to the viewport. */
  fitToken: number;
  redrawToken: number;
}

type DragMode = "move" | "resize" | "rotate" | "marquee" | "pan";

interface DragState {
  mode: DragMode;
  pointerId: number;
  start: Point;
  startDoc: LabelDoc;
  startView: StageView;
  ids: string[];
  handle: HandleId | null;
  startAngle: number;
  moved: boolean;
  marquee: Rect | null;
}

function isEditable(layer: Layer): boolean {
  return !layer.locked && !layer.hidden;
}

export default function CanvasStage(props: CanvasStageProps): JSX.Element {
  const hostRef = useRef<HTMLDivElement | null>(null);
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const dragRef = useRef<DragState | null>(null);
  const [size, setSize] = useState({ width: 0, height: 0 });
  const [guides, setGuides] = useState<readonly Guide[]>([]);
  const [marquee, setMarquee] = useState<Rect | null>(null);
  const [dropping, setDropping] = useState(false);
  const [spaceHeld, setSpaceHeld] = useState(false);

  const selected = useMemo(
    () => props.doc.layers.filter((layer) => props.selection.includes(layer.id)),
    [props.doc.layers, props.selection],
  );

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    const observer = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (!entry) return;
      setSize({ width: entry.contentRect.width, height: entry.contentRect.height });
    });
    observer.observe(host);
    setSize({ width: host.clientWidth, height: host.clientHeight });
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent): void => {
      if (event.code === "Space") setSpaceHeld(true);
    };
    const onKeyUp = (event: KeyboardEvent): void => {
      if (event.code === "Space") setSpaceHeld(false);
    };
    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("keyup", onKeyUp);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("keyup", onKeyUp);
    };
  }, []);

  const pointerIn = useCallback((event: { clientX: number; clientY: number }): Point => {
    const host = hostRef.current;
    if (!host) return { x: 0, y: 0 };
    const box = host.getBoundingClientRect();
    return { x: event.clientX - box.left, y: event.clientY - box.top };
  }, []);

  const draw = useCallback(() => {
    const canvas = canvasRef.current;
    const host = hostRef.current;
    if (!canvas || !host || size.width === 0 || size.height === 0) return;
    const ratio = window.devicePixelRatio || 1;
    const width = Math.round(size.width * ratio);
    const height = Math.round(size.height * ratio);
    if (canvas.width !== width || canvas.height !== height) {
      canvas.width = width;
      canvas.height = height;
    }
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    const colours = readColours(host);
    const view = props.view;

    ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
    ctx.clearRect(0, 0, size.width, size.height);
    ctx.fillStyle = colours.surface;
    ctx.fillRect(0, 0, size.width, size.height);

    ctx.save();
    ctx.translate(view.offsetX, view.offsetY);
    ctx.scale(view.zoom, view.zoom);
    ctx.save();
    ctx.shadowColor = "rgba(0, 0, 0, 0.45)";
    ctx.shadowBlur = 18 / view.zoom;
    ctx.fillStyle = props.doc.background;
    ctx.fillRect(0, 0, props.doc.width, props.doc.height);
    ctx.restore();
    ctx.imageSmoothingEnabled = true;
    ctx.imageSmoothingQuality = "high";
    drawDoc(ctx, props.doc, props.resolve);
    ctx.restore();

    if (view.showGrid && view.zoom >= 1.5) {
      ctx.save();
      ctx.strokeStyle = colours.border;
      ctx.globalAlpha = 0.5;
      ctx.lineWidth = 1;
      const step = view.zoom >= 6 ? 1 : view.zoom >= 3 ? 5 : 10;
      ctx.beginPath();
      for (let x = 0; x <= props.doc.width; x += step) {
        const at = Math.round(view.offsetX + x * view.zoom) + 0.5;
        ctx.moveTo(at, view.offsetY);
        ctx.lineTo(at, view.offsetY + props.doc.height * view.zoom);
      }
      for (let y = 0; y <= props.doc.height; y += step) {
        const at = Math.round(view.offsetY + y * view.zoom) + 0.5;
        ctx.moveTo(view.offsetX, at);
        ctx.lineTo(view.offsetX + props.doc.width * view.zoom, at);
      }
      ctx.stroke();
      ctx.restore();
    }

    ctx.save();
    ctx.strokeStyle = colours.border;
    ctx.lineWidth = 1;
    ctx.strokeRect(
      Math.round(view.offsetX) + 0.5,
      Math.round(view.offsetY) + 0.5,
      Math.round(props.doc.width * view.zoom),
      Math.round(props.doc.height * view.zoom),
    );
    ctx.restore();

    for (const layer of selected) {
      const rect = rectOfLayer(layer);
      const centre = centreOf(rect);
      ctx.save();
      const screenCentre = toScreen(view, centre);
      ctx.translate(screenCentre.x, screenCentre.y);
      ctx.rotate((layer.rotation * Math.PI) / 180);
      ctx.strokeStyle = layer.locked ? colours.faint : colours.focus;
      ctx.lineWidth = 1.5;
      ctx.setLineDash(layer.locked ? [4, 3] : []);
      ctx.strokeRect(
        (-rect.width / 2) * view.zoom,
        (-rect.height / 2) * view.zoom,
        rect.width * view.zoom,
        rect.height * view.zoom,
      );
      ctx.restore();
    }

    if (selected.length === 1) {
      const layer = selected[0];
      if (layer && isEditable(layer)) {
        const points = handlePoints(rectOfLayer(layer), layer.rotation);
        ctx.save();
        ctx.fillStyle = colours.raised;
        ctx.strokeStyle = colours.focus;
        ctx.lineWidth = 1.5;
        for (const id of HANDLES) {
          const at = toScreen(view, points[id]);
          ctx.beginPath();
          ctx.rect(at.x - HANDLE_SIZE / 2, at.y - HANDLE_SIZE / 2, HANDLE_SIZE, HANDLE_SIZE);
          ctx.fill();
          ctx.stroke();
        }
        const rotate = rotateHandlePoint(layer, props.view);
        const top = toScreen(view, points.n);
        ctx.beginPath();
        ctx.moveTo(top.x, top.y);
        ctx.lineTo(rotate.x, rotate.y);
        ctx.stroke();
        ctx.beginPath();
        ctx.arc(rotate.x, rotate.y, HANDLE_SIZE / 2, 0, Math.PI * 2);
        ctx.fill();
        ctx.stroke();
        ctx.restore();
      }
    }

    if (guides.length > 0) {
      ctx.save();
      ctx.strokeStyle = colours.accent;
      ctx.lineWidth = 1;
      ctx.setLineDash([4, 4]);
      ctx.beginPath();
      for (const guide of guides) {
        if (guide.axis === "x") {
          const x = Math.round(view.offsetX + guide.at * view.zoom) + 0.5;
          ctx.moveTo(x, view.offsetY + guide.from * view.zoom);
          ctx.lineTo(x, view.offsetY + guide.to * view.zoom);
        } else {
          const y = Math.round(view.offsetY + guide.at * view.zoom) + 0.5;
          ctx.moveTo(view.offsetX + guide.from * view.zoom, y);
          ctx.lineTo(view.offsetX + guide.to * view.zoom, y);
        }
      }
      ctx.stroke();
      ctx.restore();
    }

    if (marquee) {
      const box = normaliseRect(marquee);
      const topLeft = toScreen(view, { x: box.x, y: box.y });
      ctx.save();
      ctx.strokeStyle = colours.focus;
      ctx.lineWidth = 1;
      ctx.setLineDash([3, 3]);
      ctx.strokeRect(topLeft.x, topLeft.y, box.width * view.zoom, box.height * view.zoom);
      ctx.restore();
    }

    if (view.showRulers) drawRulers(ctx, props.doc, view, size, colours);
  }, [guides, marquee, props.doc, props.resolve, props.view, selected, size]);

  useEffect(() => {
    draw();
  }, [draw, props.redrawToken]);

  useEffect(() => {
    if (size.width === 0 || size.height === 0) return;
    props.onView(fitView(props.doc, size.width, size.height, props.view));
    // Refit only when the caller asks; view changes must not loop back into a fit.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [props.fitToken, size.width, size.height]);

  const handleUnder = useCallback(
    (screen: Point): HandleId | "rotate" | null => {
      if (selected.length !== 1) return null;
      const layer = selected[0];
      if (!layer || !isEditable(layer)) return null;
      const rotate = rotateHandlePoint(layer, props.view);
      if (Math.hypot(rotate.x - screen.x, rotate.y - screen.y) <= HANDLE_GRAB + 2) return "rotate";
      const points = handlePoints(rectOfLayer(layer), layer.rotation);
      for (const id of HANDLES) {
        const at = toScreen(props.view, points[id]);
        if (Math.abs(at.x - screen.x) <= HANDLE_GRAB && Math.abs(at.y - screen.y) <= HANDLE_GRAB) {
          return id;
        }
      }
      return null;
    },
    [props.view, selected],
  );

  const onPointerDown = (event: ReactPointerEvent<HTMLDivElement>): void => {
    const host = hostRef.current;
    if (!host) return;
    host.setPointerCapture(event.pointerId);
    const screen = pointerIn(event);
    const point = toDoc(props.view, screen);
    const panning = spaceHeld || event.button === 1;

    if (panning) {
      dragRef.current = {
        mode: "pan",
        pointerId: event.pointerId,
        start: screen,
        startDoc: props.doc,
        startView: props.view,
        ids: [],
        handle: null,
        startAngle: 0,
        moved: false,
        marquee: null,
      };
      return;
    }

    const grabbed = handleUnder(screen);
    if (grabbed) {
      const layer = selected[0];
      if (!layer) return;
      dragRef.current = {
        mode: grabbed === "rotate" ? "rotate" : "resize",
        pointerId: event.pointerId,
        start: point,
        startDoc: props.doc,
        startView: props.view,
        ids: [layer.id],
        handle: grabbed === "rotate" ? null : grabbed,
        startAngle: angleBetween(centreOf(rectOfLayer(layer)), point),
        moved: false,
        marquee: null,
      };
      return;
    }

    const additive = event.shiftKey;
    const hit = event.altKey
      ? pickBelow(props.doc.layers, point, props.selection[0] ?? "")
      : pickLayer(props.doc.layers, point);

    if (!hit) {
      if (!additive) props.onSelection([]);
      dragRef.current = {
        mode: "marquee",
        pointerId: event.pointerId,
        start: point,
        startDoc: props.doc,
        startView: props.view,
        ids: [...props.selection],
        handle: null,
        startAngle: 0,
        moved: false,
        marquee: { x: point.x, y: point.y, width: 0, height: 0 },
      };
      setMarquee({ x: point.x, y: point.y, width: 0, height: 0 });
      return;
    }

    let ids: string[];
    if (additive) {
      ids = props.selection.includes(hit.id)
        ? props.selection.filter((id) => id !== hit.id)
        : [...props.selection, hit.id];
    } else {
      ids = props.selection.includes(hit.id) ? [...props.selection] : [hit.id];
    }
    props.onSelection(ids);
    dragRef.current = {
      mode: "move",
      pointerId: event.pointerId,
      start: point,
      startDoc: props.doc,
      startView: props.view,
      ids,
      handle: null,
      startAngle: 0,
      moved: false,
      marquee: null,
    };
  };

  const onPointerMove = (event: ReactPointerEvent<HTMLDivElement>): void => {
    const drag = dragRef.current;
    if (!drag || drag.pointerId !== event.pointerId) return;
    const screen = pointerIn(event);
    const point = toDoc(drag.startView, screen);

    if (drag.mode === "pan") {
      props.onView({
        ...props.view,
        offsetX: drag.startView.offsetX + (screen.x - drag.start.x),
        offsetY: drag.startView.offsetY + (screen.y - drag.start.y),
      });
      return;
    }

    if (drag.mode === "marquee") {
      const box: Rect = {
        x: drag.start.x,
        y: drag.start.y,
        width: point.x - drag.start.x,
        height: point.y - drag.start.y,
      };
      drag.marquee = box;
      drag.moved = true;
      setMarquee(box);
      return;
    }

    if (drag.mode === "move") {
      const movable = drag.ids
        .map((id) => findLayer(drag.startDoc, id))
        .filter((layer): layer is Layer => layer !== null && isEditable(layer));
      if (movable.length === 0) return;
      let dx = point.x - drag.start.x;
      let dy = point.y - drag.start.y;
      if (event.shiftKey) {
        if (Math.abs(dx) > Math.abs(dy)) dy = 0;
        else dx = 0;
      }
      const bounds = selectionBounds(movable);
      let nextGuides: Guide[] = [];
      if (bounds && props.view.snap && !event.altKey) {
        const moved: Rect = { ...bounds, x: bounds.x + dx, y: bounds.y + dy };
        const others = drag.startDoc.layers
          .filter((layer) => !drag.ids.includes(layer.id) && !layer.hidden)
          .map((layer) => boundsOf(rectOfLayer(layer), layer.rotation));
        const snap = snapMove(moved, {
          threshold: SNAP_THRESHOLD / props.view.zoom,
          canvas: canvasRect(drag.startDoc),
          targets: others,
        });
        dx += snap.dx;
        dy += snap.dy;
        nextGuides = snap.guides;
      }
      setGuides(nextGuides);
      const ids = new Set(movable.map((layer) => layer.id));
      const next: LabelDoc = {
        ...drag.startDoc,
        layers: drag.startDoc.layers.map((layer) =>
          ids.has(layer.id) ? { ...layer, x: layer.x + dx, y: layer.y + dy } : layer,
        ),
      };
      drag.moved = true;
      props.onEdit(next, "Move layer", `move:${drag.ids.join(",")}`);
      return;
    }

    const layer = findLayer(drag.startDoc, drag.ids[0] ?? "");
    if (!layer) return;

    if (drag.mode === "rotate") {
      const centre = centreOf(rectOfLayer(layer));
      const angle = angleBetween(centre, point);
      const raw = layer.rotation + (angle - drag.startAngle);
      const rotation = event.shiftKey ? Math.round(raw / 15) * 15 : Math.round(raw * 10) / 10;
      const next: LabelDoc = {
        ...drag.startDoc,
        layers: drag.startDoc.layers.map((entry) =>
          entry.id === layer.id ? { ...entry, rotation: normaliseAngle(rotation) } : entry,
        ),
      };
      drag.moved = true;
      props.onEdit(next, "Rotate layer", `rotate:${layer.id}`);
      return;
    }

    if (drag.mode === "resize" && drag.handle) {
      const delta = { x: point.x - drag.start.x, y: point.y - drag.start.y };
      const rect = resizeRotated(
        rectOfLayer(layer),
        layer.rotation,
        drag.handle,
        delta,
        event.shiftKey,
      );
      const next: LabelDoc = {
        ...drag.startDoc,
        layers: drag.startDoc.layers.map((entry) =>
          entry.id === layer.id
            ? { ...entry, x: rect.x, y: rect.y, width: rect.width, height: rect.height }
            : entry,
        ),
      };
      drag.moved = true;
      props.onEdit(next, "Resize layer", `resize:${layer.id}`);
    }
  };

  const endDrag = (event: ReactPointerEvent<HTMLDivElement>): void => {
    const drag = dragRef.current;
    if (!drag || drag.pointerId !== event.pointerId) return;
    dragRef.current = null;
    hostRef.current?.releasePointerCapture(event.pointerId);
    setGuides([]);
    setMarquee(null);
    if (drag.mode === "marquee" && drag.marquee && drag.moved) {
      const hits = pickInMarquee(props.doc.layers, drag.marquee).map((layer) => layer.id);
      const merged = event.shiftKey ? [...new Set([...drag.ids, ...hits])] : hits;
      props.onSelection(merged);
      return;
    }
    if (drag.moved) props.onSeal();
  };

  const onWheel = (event: ReactWheelEvent<HTMLDivElement>): void => {
    const screen = pointerIn(event);
    if (event.ctrlKey || event.metaKey) {
      props.onView(zoomAt(props.view, screen, event.deltaY < 0 ? 1.1 : 1 / 1.1));
      return;
    }
    props.onView({
      ...props.view,
      offsetX: props.view.offsetX - event.deltaX,
      offsetY: props.view.offsetY - event.deltaY,
    });
  };

  const onDrop = (event: ReactDragEvent<HTMLDivElement>): void => {
    event.preventDefault();
    setDropping(false);
    const files = [...(event.dataTransfer?.files ?? [])].filter((file) =>
      isAcceptedType(file.type, file.name),
    );
    if (files.length > 0) props.onImport(files);
  };

  const cursor = spaceHeld ? "grab" : "default";

  return (
    <div
      ref={hostRef}
      className={`ld-stage${dropping ? " ld-stage--drop" : ""}`}
      style={{ cursor }}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={endDrag}
      onPointerCancel={endDrag}
      onWheel={onWheel}
      onDragOver={(event) => {
        event.preventDefault();
        setDropping(true);
      }}
      onDragLeave={() => setDropping(false)}
      onDrop={onDrop}
    >
      <canvas ref={canvasRef} />
    </div>
  );
}

function rotateHandlePoint(layer: Layer, view: StageView): Point {
  const rect = rectOfLayer(layer);
  const points = handlePoints(rect, layer.rotation);
  const top = toScreen(view, points.n);
  const centre = toScreen(view, centreOf(rect));
  const dx = top.x - centre.x;
  const dy = top.y - centre.y;
  const length = Math.hypot(dx, dy) || 1;
  return { x: top.x + (dx / length) * ROTATE_OFFSET, y: top.y + (dy / length) * ROTATE_OFFSET };
}

function drawRulers(
  ctx: CanvasRenderingContext2D,
  doc: LabelDoc,
  view: StageView,
  size: { width: number; height: number },
  colours: { raised: string; border: string; faint: string },
): void {
  const step = view.zoom >= 3 ? 10 : view.zoom >= 1 ? 25 : 50;
  ctx.save();
  ctx.fillStyle = colours.raised;
  ctx.fillRect(0, 0, size.width, RULER_SIZE);
  ctx.fillRect(0, 0, RULER_SIZE, size.height);
  ctx.strokeStyle = colours.border;
  ctx.lineWidth = 1;
  ctx.beginPath();
  ctx.moveTo(0, RULER_SIZE + 0.5);
  ctx.lineTo(size.width, RULER_SIZE + 0.5);
  ctx.moveTo(RULER_SIZE + 0.5, 0);
  ctx.lineTo(RULER_SIZE + 0.5, size.height);
  ctx.stroke();
  ctx.fillStyle = colours.faint;
  ctx.font = "9px system-ui, sans-serif";
  ctx.textBaseline = "top";
  ctx.textAlign = "left";
  for (let x = 0; x <= doc.width; x += step) {
    const at = view.offsetX + x * view.zoom;
    if (at < RULER_SIZE || at > size.width) continue;
    ctx.fillRect(Math.round(at), RULER_SIZE - 4, 1, 4);
    ctx.fillText(String(x), Math.round(at) + 2, 2);
  }
  for (let y = 0; y <= doc.height; y += step) {
    const at = view.offsetY + y * view.zoom;
    if (at < RULER_SIZE || at > size.height) continue;
    ctx.fillRect(RULER_SIZE - 4, Math.round(at), 4, 1);
    ctx.save();
    ctx.translate(2, Math.round(at) + 2);
    ctx.fillText(String(y), 0, 0);
    ctx.restore();
  }
  ctx.restore();
}
