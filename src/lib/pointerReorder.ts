// Drag-to-reorder on pointer events.
//
// The webview's own HTML5 drag events never fire: Tauri claims them for native
// file drops (see lib/fileDrop.ts), so `draggable` list rows do nothing. Pointer
// events are untouched, and they also work under touch and pen.

import { useCallback, useEffect, useRef, useState } from "react";

/// Where the dragged row would land, given the pointer and the rows' boxes.
export function dropIndexFor(centres: number[], pointerY: number, from: number): number {
  let target = centres.findIndex((centre) => pointerY < centre);
  if (target === -1) target = centres.length;
  if (target > from) target -= 1;
  return Math.max(0, Math.min(centres.length - 1, target));
}

export interface ReorderState {
  /// Index being dragged, or null.
  dragging: number | null;
  /// Index the row would land on while dragging.
  over: number | null;
  /// Put this on each row's drag handle, or the row itself.
  handleProps: (index: number) => {
    onPointerDown: (event: React.PointerEvent) => void;
    style: { touchAction: "none" };
  };
  /// Put this on each row so its box can be measured.
  rowRef: (index: number) => (node: HTMLElement | null) => void;
}

/// `onCommit(from, to)` fires once, on release, and only when the order changed.
export function usePointerReorder(count: number, onCommit: (from: number, to: number) => void): ReorderState {
  const [dragging, setDragging] = useState<number | null>(null);
  const [over, setOver] = useState<number | null>(null);
  const rows = useRef<(HTMLElement | null)[]>([]);
  const target = useRef<number | null>(null);
  const commit = useRef(onCommit);
  commit.current = onCommit;

  const rowRef = useCallback(
    (index: number) => (node: HTMLElement | null) => {
      rows.current[index] = node;
    },
    [],
  );

  useEffect(() => {
    rows.current.length = count;
  }, [count]);

  useEffect(() => {
    if (dragging === null) return;

    const centres = () =>
      rows.current
        .slice(0, count)
        .map((node) => (node ? node.getBoundingClientRect() : null))
        .map((box) => (box ? box.top + box.height / 2 : Number.POSITIVE_INFINITY));

    const onMove = (event: PointerEvent) => {
      const next = dropIndexFor(centres(), event.clientY, dragging);
      target.current = next;
      setOver(next);
    };
    const onUp = () => {
      const to = target.current;
      const from = dragging;
      setDragging(null);
      setOver(null);
      target.current = null;
      if (to !== null && to !== from) commit.current(from, to);
    };
    const onCancel = () => {
      setDragging(null);
      setOver(null);
      target.current = null;
    };

    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onCancel);
    return () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("pointercancel", onCancel);
    };
  }, [dragging, count]);

  const handleProps = useCallback(
    (index: number) => ({
      onPointerDown: (event: React.PointerEvent) => {
        if (event.button !== 0) return;
        event.preventDefault();
        target.current = index;
        setDragging(index);
        setOver(index);
      },
      style: { touchAction: "none" as const },
    }),
    [],
  );

  return { dragging, over, handleProps, rowRef };
}
