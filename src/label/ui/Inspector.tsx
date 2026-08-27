// Property editors for the current selection. Every change commits a whole document;
// repeated edits to one field coalesce into a single undo entry.

import type { LabelDoc, Layer, TextAlign } from "../../lib/types";
import { FIT_MODES, TEXT_ALIGNS, mapLayer, mapLayers } from "../core/doc";
import { FONT_CHOICES, WEIGHTS } from "../core/fonts";
import type { AlignMode, DistributeMode } from "../core/snap";
import { ColourField, NumberField, SelectField, TextAreaField } from "./fields";

export interface InspectorProps {
  doc: LabelDoc;
  selection: readonly string[];
  onEdit: (doc: LabelDoc, label: string, coalesceKey: string | null) => void;
  onReplaceImage: (layerId: string) => void;
  onFitBoxToImage: (layerId: string) => void;
  onAlign: (mode: AlignMode, against: "canvas" | "selection") => void;
  onDistribute: (mode: DistributeMode) => void;
}

const FIT_LABELS: Record<string, string> = {
  contain: "Contain (whole image, letterboxed)",
  cover: "Cover (fill the box, crop the overflow)",
  crop: "Crop (centre crop to the box)",
  scale: "Scale (native pixels, centred)",
  stretch: "Stretch (fill, ignore the aspect)",
};

export default function Inspector(props: InspectorProps): JSX.Element {
  const layers = props.doc.layers.filter((layer) => props.selection.includes(layer.id));
  const single = layers.length === 1 ? layers[0] : null;

  const edit = (id: string, label: string, key: string, fn: (layer: Layer) => Layer): void => {
    props.onEdit(mapLayer(props.doc, id, fn), label, `${key}:${id}`);
  };

  const editAll = (label: string, key: string, fn: (layer: Layer) => Layer): void => {
    props.onEdit(mapLayers(props.doc, props.selection, fn), label, `${key}:${props.selection.join(",")}`);
  };

  return (
    <>
      <section className="ld-section">
        <h3 className="ld-title">Document</h3>
        <ColourField
          label="Background"
          value={props.doc.background}
          onChange={(value) =>
            props.onEdit(
              { ...props.doc, background: value ?? props.doc.background },
              "Background colour",
              "doc:background",
            )
          }
        />
        <p className="ld-note">
          {props.doc.width} x {props.doc.height} px, template {props.doc.template}
        </p>
      </section>

      {layers.length === 0 ? (
        <section className="ld-section">
          <p className="ld-empty">Select a layer to edit it.</p>
        </section>
      ) : null}

      {layers.length > 0 ? (
        <section className="ld-section">
          <h3 className="ld-title">
            {layers.length === 1 ? (single?.name ?? "Layer") : `${layers.length} layers`}
          </h3>
          <div className="ld-row">
            <NumberField
              label="X"
              value={layers[0]?.x ?? 0}
              disabled={layers.length !== 1}
              onChange={(value) => single && edit(single.id, "Move layer", "x", (l) => ({ ...l, x: value }))}
            />
            <NumberField
              label="Y"
              value={layers[0]?.y ?? 0}
              disabled={layers.length !== 1}
              onChange={(value) => single && edit(single.id, "Move layer", "y", (l) => ({ ...l, y: value }))}
            />
          </div>
          <div className="ld-row">
            <NumberField
              label="Width"
              value={layers[0]?.width ?? 0}
              min={1}
              disabled={layers.length !== 1}
              onChange={(value) =>
                single &&
                edit(single.id, "Resize layer", "w", (l) => ({ ...l, width: Math.max(1, value) }))
              }
            />
            <NumberField
              label="Height"
              value={layers[0]?.height ?? 0}
              min={1}
              disabled={layers.length !== 1}
              onChange={(value) =>
                single &&
                edit(single.id, "Resize layer", "h", (l) => ({ ...l, height: Math.max(1, value) }))
              }
            />
          </div>
          <NumberField
            label="Rotation"
            value={layers[0]?.rotation ?? 0}
            step={1}
            onChange={(value) => editAll("Rotate layer", "rot", (l) => ({ ...l, rotation: value }))}
          />
        </section>
      ) : null}

      {layers.length > 0 ? (
        <section className="ld-section">
          <h3 className="ld-title">Align</h3>
          <div className="ld-chips">
            <button type="button" className="ld-button" onClick={() => props.onAlign("left", "canvas")}>
              Left
            </button>
            <button type="button" className="ld-button" onClick={() => props.onAlign("hcentre", "canvas")}>
              Centre
            </button>
            <button type="button" className="ld-button" onClick={() => props.onAlign("right", "canvas")}>
              Right
            </button>
            <button type="button" className="ld-button" onClick={() => props.onAlign("top", "canvas")}>
              Top
            </button>
            <button type="button" className="ld-button" onClick={() => props.onAlign("vmiddle", "canvas")}>
              Middle
            </button>
            <button type="button" className="ld-button" onClick={() => props.onAlign("bottom", "canvas")}>
              Bottom
            </button>
          </div>
          {layers.length > 1 ? (
            <div className="ld-chips">
              <button
                type="button"
                className="ld-button"
                onClick={() => props.onAlign("left", "selection")}
              >
                Left edges
              </button>
              <button
                type="button"
                className="ld-button"
                onClick={() => props.onAlign("top", "selection")}
              >
                Top edges
              </button>
              <button
                type="button"
                className="ld-button"
                onClick={() => props.onDistribute("horizontal")}
                disabled={layers.length < 3}
              >
                Space across
              </button>
              <button
                type="button"
                className="ld-button"
                onClick={() => props.onDistribute("vertical")}
                disabled={layers.length < 3}
              >
                Space down
              </button>
            </div>
          ) : null}
        </section>
      ) : null}

      {single && single.kind === "image" ? (
        <section className="ld-section">
          <h3 className="ld-title">Image</h3>
          <SelectField
            label="Fit"
            value={single.fit}
            options={FIT_MODES.map((mode) => ({ value: mode, label: FIT_LABELS[mode] ?? mode }))}
            onChange={(value) =>
              edit(single.id, "Fit mode", "fit", (l) =>
                l.kind === "image" ? { ...l, fit: FIT_MODES.find((mode) => mode === value) ?? l.fit } : l,
              )
            }
          />
          <NumberField
            label="Opacity"
            value={single.opacity ?? 1}
            step={0.05}
            min={0}
            max={1}
            onChange={(value) =>
              edit(single.id, "Opacity", "opacity", (l) =>
                l.kind === "image" ? { ...l, opacity: Math.min(1, Math.max(0, value)) } : l,
              )
            }
          />
          <div className="ld-chips">
            <button type="button" className="ld-button" onClick={() => props.onReplaceImage(single.id)}>
              Replace image
            </button>
            <button type="button" className="ld-button" onClick={() => props.onFitBoxToImage(single.id)}>
              Fit box to image
            </button>
          </div>
        </section>
      ) : null}

      {single && single.kind === "text" ? (
        <section className="ld-section">
          <h3 className="ld-title">Text</h3>
          <TextAreaField
            label="Content"
            value={single.text}
            onChange={(value) =>
              edit(single.id, "Edit text", "text", (l) => (l.kind === "text" ? { ...l, text: value } : l))
            }
          />
          <SelectField
            label="Font"
            value={single.font}
            options={[
              ...FONT_CHOICES.map((choice) => ({ value: choice.stack, label: choice.name })),
              ...(FONT_CHOICES.some((choice) => choice.stack === single.font)
                ? []
                : [{ value: single.font, label: single.font }]),
            ]}
            onChange={(value) =>
              edit(single.id, "Font", "font", (l) => (l.kind === "text" ? { ...l, font: value } : l))
            }
          />
          <div className="ld-row">
            <NumberField
              label="Size"
              value={single.size}
              min={1}
              onChange={(value) =>
                edit(single.id, "Text size", "size", (l) =>
                  l.kind === "text" ? { ...l, size: Math.max(1, value) } : l,
                )
              }
            />
            <SelectField
              label="Weight"
              value={single.weight ?? "400"}
              options={WEIGHTS.map((weight) => ({ value: weight, label: weight }))}
              onChange={(value) =>
                edit(single.id, "Text weight", "weight", (l) =>
                  l.kind === "text" ? { ...l, weight: value } : l,
                )
              }
            />
          </div>
          <div className="ld-row">
            <SelectField
              label="Align"
              value={single.align}
              options={TEXT_ALIGNS.map((align) => ({ value: align, label: align }))}
              onChange={(value) =>
                edit(single.id, "Text align", "align", (l) =>
                  l.kind === "text"
                    ? { ...l, align: TEXT_ALIGNS.find((entry) => entry === value) ?? (l.align as TextAlign) }
                    : l,
                )
              }
            />
            <NumberField
              label="Letter spacing"
              value={single.letter_spacing ?? 0}
              step={0.5}
              onChange={(value) =>
                edit(single.id, "Letter spacing", "spacing", (l) =>
                  l.kind === "text" ? { ...l, letter_spacing: value } : l,
                )
              }
            />
          </div>
          <NumberField
            label="Line height"
            value={single.line_height ?? 1.2}
            step={0.05}
            min={0.5}
            onChange={(value) =>
              edit(single.id, "Line height", "lineheight", (l) =>
                l.kind === "text" ? { ...l, line_height: Math.max(0.5, value) } : l,
              )
            }
          />
          <ColourField
            label="Colour"
            value={single.colour}
            onChange={(value) =>
              edit(single.id, "Text colour", "colour", (l) =>
                l.kind === "text" && value ? { ...l, colour: value } : l,
              )
            }
          />
          <div className="ld-row">
            <ColourField
              label="Stroke"
              value={single.stroke ?? null}
              optional
              onChange={(value) =>
                edit(single.id, "Text stroke", "stroke", (l) =>
                  l.kind === "text" ? { ...l, stroke: value } : l,
                )
              }
            />
            <NumberField
              label="Stroke width"
              value={single.stroke_width ?? 0}
              min={0}
              step={0.5}
              onChange={(value) =>
                edit(single.id, "Stroke width", "strokewidth", (l) =>
                  l.kind === "text" ? { ...l, stroke_width: Math.max(0, value) } : l,
                )
              }
            />
          </div>
        </section>
      ) : null}

      {single && single.kind === "rect" ? (
        <section className="ld-section">
          <h3 className="ld-title">Rectangle</h3>
          <ColourField
            label="Fill"
            value={single.fill}
            onChange={(value) =>
              edit(single.id, "Fill colour", "fill", (l) =>
                l.kind === "rect" && value ? { ...l, fill: value } : l,
              )
            }
          />
          <NumberField
            label="Corner radius"
            value={single.radius ?? 0}
            min={0}
            onChange={(value) =>
              edit(single.id, "Corner radius", "radius", (l) =>
                l.kind === "rect" ? { ...l, radius: Math.max(0, value) } : l,
              )
            }
          />
          <div className="ld-row">
            <ColourField
              label="Stroke"
              value={single.stroke ?? null}
              optional
              onChange={(value) =>
                edit(single.id, "Rect stroke", "rectstroke", (l) =>
                  l.kind === "rect" ? { ...l, stroke: value } : l,
                )
              }
            />
            <NumberField
              label="Stroke width"
              value={single.stroke_width ?? 0}
              min={0}
              step={0.5}
              onChange={(value) =>
                edit(single.id, "Rect stroke width", "rectstrokewidth", (l) =>
                  l.kind === "rect" ? { ...l, stroke_width: Math.max(0, value) } : l,
                )
              }
            />
          </div>
        </section>
      ) : null}
    </>
  );
}
