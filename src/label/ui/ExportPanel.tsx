// Export controls and the result of the last manifest check. Writing is gated by
// the backend check, so this panel only ever shows what came back from it.

import type { ExportCheck } from "../../lib/types";
import {
  MULTIPLES,
  QUANTIZE_STEPS,
  describeSettings,
  formatBytes,
  type ExportSettings,
} from "../core/exportGuard";
import { SelectField } from "./fields";

export type ExportPhase =
  | { kind: "idle" }
  | { kind: "working"; note: string }
  | { kind: "blocked"; check: ExportCheck; retry: ExportSettings | null }
  | { kind: "done"; check: ExportCheck }
  | { kind: "error"; message: string };

export interface ExportPanelProps {
  labelPath: string;
  settings: ExportSettings;
  phase: ExportPhase;
  estimate: number | null;
  onSettings: (settings: ExportSettings) => void;
  onExport: () => void;
  onRetry: (settings: ExportSettings) => void;
  onEstimate: () => void;
}

export default function ExportPanel(props: ExportPanelProps): JSX.Element {
  const working = props.phase.kind === "working";
  return (
    <section className="ld-section">
      <h3 className="ld-title">Export</h3>
      <p className="ld-note">
        Writes {props.labelPath} into the cart directory. {describeSettings(props.settings)}.
      </p>
      <div className="ld-row">
        <SelectField
          label="Resolution"
          value={String(props.settings.multiple)}
          options={MULTIPLES.map((multiple) => ({
            value: String(multiple),
            label: `${multiple}x native`,
          }))}
          onChange={(value) =>
            props.onSettings({ ...props.settings, multiple: Number.parseInt(value, 10) || 1 })
          }
        />
        <SelectField
          label="Colour depth"
          value={props.settings.quantize === null ? "full" : String(props.settings.quantize)}
          options={QUANTIZE_STEPS.map((step) => ({
            value: step === null ? "full" : String(step),
            label: step === null ? "Full colour" : `${step} levels`,
          }))}
          onChange={(value) =>
            props.onSettings({
              ...props.settings,
              quantize: value === "full" ? null : Number.parseInt(value, 10),
            })
          }
        />
      </div>
      <div className="ld-chips">
        <button
          type="button"
          className="ld-button ld-button--primary"
          disabled={working}
          onClick={props.onExport}
        >
          {working ? "Working..." : "Export PNG"}
        </button>
        <button type="button" className="ld-button" disabled={working} onClick={props.onEstimate}>
          Check size
        </button>
      </div>
      {props.estimate !== null ? (
        <p className="ld-note">Rendered size: {formatBytes(props.estimate)}</p>
      ) : null}

      {props.phase.kind === "working" ? <p className="ld-note">{props.phase.note}</p> : null}

      {props.phase.kind === "error" ? (
        <div className="ld-banner ld-banner--error">
          <strong>Export failed.</strong>
          <span>{props.phase.message}</span>
        </div>
      ) : null}

      {props.phase.kind === "blocked" ? (
        <div className="ld-banner ld-banner--error">
          <strong>Nothing was written: the manifest would reject this file.</strong>
          <ul>
            {props.phase.check.problems.map((problem) => (
              <li key={problem}>{problem}</li>
            ))}
          </ul>
          <div className="ld-banner__actions">
            {props.phase.retry ? (
              <button
                type="button"
                className="ld-button ld-button--primary"
                onClick={() => {
                  const retry = props.phase.kind === "blocked" ? props.phase.retry : null;
                  if (retry) props.onRetry(retry);
                }}
              >
                Recompress at {describeSettings(props.phase.retry)} and retry
              </button>
            ) : (
              <span>Reduce the artwork before exporting again.</span>
            )}
          </div>
        </div>
      ) : null}

      {props.phase.kind === "done" ? (
        <div
          className={`ld-banner ${
            props.phase.check.warnings.length > 0 ? "ld-banner--warn" : "ld-banner--ok"
          }`}
        >
          <strong>
            Wrote {props.labelPath} ({formatBytes(props.phase.check.bytes)}
            {props.phase.check.width && props.phase.check.height
              ? `, ${props.phase.check.width} x ${props.phase.check.height}`
              : ""}
            ).
          </strong>
          {props.phase.check.warnings.length > 0 ? (
            <ul>
              {props.phase.check.warnings.map((warning) => (
                <li key={warning}>{warning}</li>
              ))}
            </ul>
          ) : null}
        </div>
      ) : null}
    </section>
  );
}
