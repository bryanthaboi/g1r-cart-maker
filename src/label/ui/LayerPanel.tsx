// The layer stack, topmost first, with drag-to-reorder and per-layer state toggles.

import { useState } from "react";
import { usePointerReorder } from "../../lib/pointerReorder";
import type { LabelDoc, Layer } from "../../lib/types";

export interface LayerPanelProps {
  doc: LabelDoc;
  selection: readonly string[];
  onSelect: (id: string, additive: boolean, range: boolean) => void;
  onRename: (id: string, name: string) => void;
  onToggleHidden: (id: string) => void;
  onToggleLocked: (id: string) => void;
  onReorder: (ids: readonly string[], target: number) => void;
  onReset: (id: string) => void;
  canReset: (layer: Layer) => boolean;
}

function kindGlyph(layer: Layer): string {
  switch (layer.kind) {
    case "image":
      return "IMG";
    case "text":
      return "TXT";
    case "rect":
      return "RCT";
    default:
      return "?";
  }
}

export default function LayerPanel(props: LayerPanelProps): JSX.Element {
  const [editing, setEditing] = useState<string | null>(null);
  const [draft, setDraft] = useState("");

  const ordered = [...props.doc.layers].reverse();

  // The panel shows the stack topmost first, so a display index counts back from
  // the top of doc.layers.
  const reorder = usePointerReorder(ordered.length, (from, to) => {
    const dragged = ordered[from];
    if (!dragged) return;
    const ids = props.selection.includes(dragged.id) ? props.selection : [dragged.id];
    props.onReorder(ids, props.doc.layers.length - to);
  });

  if (props.doc.layers.length === 0) {
    return <p className="ld-empty">No layers yet. Add text, a rectangle or an image.</p>;
  }

  return (
    <ul className="ld-layers">
      {ordered.map((layer, displayIndex) => {
        const selected = props.selection.includes(layer.id);
        return (
          <li
            key={layer.id}
            ref={reorder.rowRef(displayIndex)}
            className="ld-layer"
            data-selected={selected}
            data-dragover={reorder.over === displayIndex && reorder.dragging !== displayIndex}
            data-dragging={reorder.dragging === displayIndex}
          >
            <span
              className="ld-layer__grip"
              title="Drag to reorder"
              aria-label={`Reorder ${layer.name}`}
              {...reorder.handleProps(displayIndex)}
              onPointerDownCapture={() => {
                if (!props.selection.includes(layer.id)) props.onSelect(layer.id, false, false);
              }}
            >
              ::
            </span>
            <span className="ld-layer__badge">{kindGlyph(layer)}</span>
            {editing === layer.id ? (
              <input
                className="ld-input"
                autoFocus
                value={draft}
                onChange={(event) => setDraft(event.target.value)}
                onBlur={() => {
                  props.onRename(layer.id, draft.trim().length > 0 ? draft.trim() : layer.name);
                  setEditing(null);
                }}
                onKeyDown={(event) => {
                  if (event.key === "Enter") event.currentTarget.blur();
                  if (event.key === "Escape") {
                    setEditing(null);
                  }
                }}
              />
            ) : (
              <button
                type="button"
                className="ld-layer__name"
                title={layer.from_template ? `From template: ${layer.from_template}` : layer.name}
                onClick={(event) => props.onSelect(layer.id, event.shiftKey || event.metaKey || event.ctrlKey, false)}
                onDoubleClick={() => {
                  setEditing(layer.id);
                  setDraft(layer.name);
                }}
              >
                {layer.name}
              </button>
            )}
            {props.canReset(layer) ? (
              <button
                type="button"
                className="ld-button ld-button--icon"
                title="Reset to the template original"
                onClick={() => props.onReset(layer.id)}
              >
                rst
              </button>
            ) : null}
            <button
              type="button"
              className="ld-button ld-button--icon"
              aria-pressed={layer.hidden}
              title={layer.hidden ? "Show layer" : "Hide layer"}
              onClick={() => props.onToggleHidden(layer.id)}
            >
              {layer.hidden ? "off" : "on"}
            </button>
            <button
              type="button"
              className="ld-button ld-button--icon"
              aria-pressed={layer.locked}
              title={layer.locked ? "Unlock layer" : "Lock layer"}
              onClick={() => props.onToggleLocked(layer.id)}
            >
              {layer.locked ? "lck" : "unl"}
            </button>
          </li>
        );
      })}
    </ul>
  );
}
