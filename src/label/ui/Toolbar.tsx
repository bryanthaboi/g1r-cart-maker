// The stage toolbar: history, view controls, and the layer-creating actions.

import type { LabelTemplate } from "../../lib/types";
import type { StageView } from "./stageMath";

export interface ToolbarProps {
  view: StageView;
  onView: (view: StageView) => void;
  onFit: () => void;
  canUndo: boolean;
  canRedo: boolean;
  undoLabel: string | null;
  redoLabel: string | null;
  onUndo: () => void;
  onRedo: () => void;
  onAddText: () => void;
  onAddRect: () => void;
  onImport: () => void;
  onDuplicate: () => void;
  onDelete: () => void;
  onRaise: () => void;
  onLower: () => void;
  hasSelection: boolean;
  templates: readonly LabelTemplate[];
  templateId: string;
  onTemplate: (id: string) => void;
}

export default function Toolbar(props: ToolbarProps): JSX.Element {
  const zoomPercent = Math.round(props.view.zoom * 100);
  return (
    <div className="ld-bar">
      <div className="ld-group">
        <button
          type="button"
          className="ld-button"
          disabled={!props.canUndo}
          onClick={props.onUndo}
          title={props.undoLabel ? `Undo ${props.undoLabel}` : "Undo"}
        >
          Undo
        </button>
        <button
          type="button"
          className="ld-button"
          disabled={!props.canRedo}
          onClick={props.onRedo}
          title={props.redoLabel ? `Redo ${props.redoLabel}` : "Redo"}
        >
          Redo
        </button>
      </div>

      <div className="ld-group">
        <button type="button" className="ld-button" onClick={props.onAddText}>
          Text
        </button>
        <button type="button" className="ld-button" onClick={props.onAddRect}>
          Rect
        </button>
        <button type="button" className="ld-button" onClick={props.onImport}>
          Image...
        </button>
      </div>

      <div className="ld-group">
        <button
          type="button"
          className="ld-button"
          disabled={!props.hasSelection}
          onClick={props.onDuplicate}
        >
          Duplicate
        </button>
        <button
          type="button"
          className="ld-button"
          disabled={!props.hasSelection}
          onClick={props.onRaise}
          title="Bring forward"
        >
          Up
        </button>
        <button
          type="button"
          className="ld-button"
          disabled={!props.hasSelection}
          onClick={props.onLower}
          title="Send backward"
        >
          Down
        </button>
        <button
          type="button"
          className="ld-button ld-button--danger"
          disabled={!props.hasSelection}
          onClick={props.onDelete}
        >
          Delete
        </button>
      </div>

      <div className="ld-spacer" />

      <div className="ld-group">
        <label className="ld-field__label" htmlFor="ld-template">
          Template
        </label>
        <select
          id="ld-template"
          className="ld-select"
          value={props.templateId}
          onChange={(event) => props.onTemplate(event.target.value)}
        >
          <option value="blank">Blank</option>
          {props.templates.map((template) => (
            <option key={template.id} value={template.id}>
              {template.name}
            </option>
          ))}
        </select>
      </div>

      <div className="ld-group">
        <button
          type="button"
          className="ld-button ld-button--icon"
          onClick={() => props.onView({ ...props.view, zoom: Math.max(0.1, props.view.zoom / 1.25) })}
          title="Zoom out"
        >
          -
        </button>
        <span className="ld-status">{zoomPercent}%</span>
        <button
          type="button"
          className="ld-button ld-button--icon"
          onClick={() => props.onView({ ...props.view, zoom: Math.min(16, props.view.zoom * 1.25) })}
          title="Zoom in"
        >
          +
        </button>
        <button type="button" className="ld-button" onClick={props.onFit}>
          Fit
        </button>
      </div>

      <div className="ld-group">
        <button
          type="button"
          className="ld-button"
          aria-pressed={props.view.snap}
          onClick={() => props.onView({ ...props.view, snap: !props.view.snap })}
          title="Snap to edges and centres"
        >
          Snap
        </button>
        <button
          type="button"
          className="ld-button"
          aria-pressed={props.view.showGrid}
          onClick={() => props.onView({ ...props.view, showGrid: !props.view.showGrid })}
          title="Pixel grid, from 150% zoom"
        >
          Grid
        </button>
        <button
          type="button"
          className="ld-button"
          aria-pressed={props.view.showRulers}
          onClick={() => props.onView({ ...props.view, showRulers: !props.view.showRulers })}
        >
          Rulers
        </button>
      </div>
    </div>
  );
}
