import { useMemo } from "react";
import { FieldFindings, FindingsList } from "../components/Findings";
import { Banner, Button, Card, Chip, ColourPicker, Field, Select, TextArea, TextInput } from "../components/ui";
import {
  BASES,
  BASE_LABELS,
  FINISHES,
  FINISH_HELP,
  LIMITS,
  SEALS,
  SEAL_HELP,
  SPEED_LADDER,
} from "../lib/constants";
import { findingsForPath, summarize } from "../lib/findings";
import { formatBytes } from "../lib/format";
import type { Base, Finish, Seal } from "../lib/types";
import { validateCart } from "../lib/validate";
import { useStore } from "../state/store";

export function CartScreen(): JSX.Element {
  const { state, patch, saveDraft, dispatch } = useStore();
  const cart = state.draft;
  const project = state.project;

  const findings = useMemo(() => (cart ? validateCart(cart) : []), [cart]);

  if (!cart || !project) {
    return (
      <div className="screen">
        <Banner tone="note">Open or create a cart to edit it.</Banner>
      </div>
    );
  }

  const speeds = cart.speeds;
  const allSpeeds = speeds === undefined;
  const labelInfo = project.label;

  const toggleSpeed = (speed: number) => {
    const current = speeds ?? [...SPEED_LADDER];
    const next = current.includes(speed) ? current.filter((entry) => entry !== speed) : [...current, speed];
    next.sort((a, b) => a - b);
    patch({ speeds: next });
  };

  return (
    <div className="screen">
      <div className="screen-head">
        <div>
          <h1>{cart.title || cart.id}</h1>
          <p className="screen-sub">
            {summarize(findings)}. Saving writes cart.json in place and preserves key order.
          </p>
        </div>
        <div className="screen-head-actions">
          <Button onClick={() => dispatch({ type: "draft/reset" })} disabled={!state.dirty}>
            Revert
          </Button>
          <Button variant="primary" onClick={() => void saveDraft()} disabled={!state.dirty}>
            Save cart.json
          </Button>
        </div>
      </div>

      {state.dirty ? <Banner tone="warn">Unsaved changes. Nothing on disk has moved yet.</Banner> : null}

      <div className="grid-2">
        <Card title="Identity">
          <Field label="Title" htmlFor="cart-title" counter={`${(cart.title ?? "").length}/${LIMITS.title}`}>
            <TextInput id="cart-title" value={cart.title ?? ""} onChange={(value) => patch({ title: value })} />
            <FieldFindings findings={findingsForPath(findings, "title")} />
          </Field>
          <Field label="Id" htmlFor="cart-id" hint="Changing an id changes the save scope of an installed cart.">
            <TextInput id="cart-id" mono value={cart.id ?? ""} onChange={(value) => patch({ id: value })} />
            <FieldFindings findings={findingsForPath(findings, "id")} />
          </Field>
          <Field label="Version" htmlFor="cart-version" hint="The release tag must be v followed by exactly this.">
            <TextInput id="cart-version" mono value={cart.version ?? ""} onChange={(value) => patch({ version: value })} />
            <FieldFindings findings={findingsForPath(findings, "version")} />
          </Field>
          <Field label="Author" htmlFor="cart-author" counter={`${(cart.author ?? "").length}/${LIMITS.author}`}>
            <TextInput id="cart-author" value={cart.author ?? ""} onChange={(value) => patch({ author: value })} />
            <FieldFindings findings={findingsForPath(findings, "author")} />
          </Field>
          <Field
            label="Summary"
            htmlFor="cart-summary"
            counter={`${(cart.summary ?? "").length}/${LIMITS.summary}`}
            hint="One line, shown on the index listing."
          >
            <TextArea
              id="cart-summary"
              rows={2}
              value={cart.summary ?? ""}
              onChange={(value) => patch({ summary: value.length > 0 ? value : undefined })}
            />
            <FieldFindings findings={findingsForPath(findings, "summary")} />
          </Field>
          <Field label="Repository" htmlFor="cart-repo" hint="owner/name. The index cannot list a cart it cannot fetch.">
            <TextInput
              id="cart-repo"
              mono
              value={cart.repo ?? ""}
              onChange={(value) => patch({ repo: value.length > 0 ? value : undefined })}
            />
            <FieldFindings findings={findingsForPath(findings, "repo")} />
          </Field>
        </Card>

        <Card title="Cartridge">
          <Field label="Base game" htmlFor="cart-base">
            <Select
              id="cart-base"
              value={cart.base}
              onChange={(value: Base) => patch({ base: value })}
              options={BASES.map((base) => ({ value: base, label: BASE_LABELS[base] }))}
            />
            <FieldFindings findings={findingsForPath(findings, "base")} />
          </Field>

          <Field label="Seal">
            <div className="seal-options" role="radiogroup" aria-label="Seal">
              {SEALS.map((seal) => (
                <button
                  key={seal}
                  type="button"
                  role="radio"
                  aria-checked={cart.seal === seal}
                  className={`seal-option${cart.seal === seal ? " seal-selected" : ""}`}
                  onClick={() => patch({ seal: seal as Seal })}
                >
                  <span className="seal-name">{seal}</span>
                  <span className="seal-help">{SEAL_HELP[seal]}</span>
                </button>
              ))}
            </div>
            <FieldFindings findings={findingsForPath(findings, "seal")} />
          </Field>

          <Field label="Finish" hint="Optional. How the launcher draws the cartridge.">
            <div className="chip-row">
              <Chip tone="default" active={cart.finish === undefined} onClick={() => patch({ finish: undefined })}>
                none
              </Chip>
              {FINISHES.map((finish) => (
                <Chip
                  key={finish}
                  active={cart.finish === finish}
                  onClick={() => patch({ finish: finish as Finish })}
                  title={FINISH_HELP[finish]}
                >
                  {finish}
                </Chip>
              ))}
            </div>
            <FieldFindings findings={findingsForPath(findings, "finish")} />
          </Field>

          <Field label="Shell colour" htmlFor="cart-shell">
            <ColourPicker id="cart-shell" value={cart.shell ?? ""} onChange={(value) => patch({ shell: value })} />
            <FieldFindings findings={findingsForPath(findings, "shell")} />
          </Field>

          <Field
            label="Label"
            htmlFor="cart-label"
            hint={
              labelInfo.exists
                ? `${labelInfo.width ?? "?"} x ${labelInfo.height ?? "?"}, ${formatBytes(labelInfo.bytes)} on disk.`
                : "No file at that path yet. The Label tab writes it."
            }
          >
            <TextInput
              id="cart-label"
              mono
              value={typeof cart.label === "string" ? cart.label : ""}
              onChange={(value) => patch({ label: value.length > 0 ? value : undefined })}
            />
            <FieldFindings findings={findingsForPath(findings, "label")} />
          </Field>
        </Card>
      </div>

      <div className="grid-2">
        <Card
          title="Speed ladder"
          subtitle="Which speeds the player may pick. Leaving it as the full ladder omits the key from cart.json."
        >
          <div className="speed-row">
            <Chip active={allSpeeds} onClick={() => patch({ speeds: undefined })}>
              All speeds
            </Chip>
            <span className="muted">or pick a subset:</span>
          </div>
          <div className="speed-grid" role="group" aria-label="Speed ladder">
            {SPEED_LADDER.map((speed) => {
              const on = allSpeeds || (speeds ?? []).includes(speed);
              return (
                <button
                  key={speed}
                  type="button"
                  aria-pressed={on}
                  className={`speed-cell${on ? " speed-on" : ""}${allSpeeds ? " speed-implicit" : ""}`}
                  onClick={() => toggleSpeed(speed)}
                >
                  {speed}x
                </button>
              );
            })}
          </div>
          {!allSpeeds ? (
            <p className="field-hint">
              {(speeds ?? []).length} of {SPEED_LADDER.length} speeds selected.{" "}
              <button type="button" className="link" onClick={() => patch({ speeds: undefined })}>
                Reset to the full ladder
              </button>
            </p>
          ) : null}
          <FieldFindings findings={findingsForPath(findings, "speeds")} />
        </Card>

        <Card title="Engine">
          <Field
            label="Engine range"
            htmlFor="cart-engine"
            hint="A semver range. Scaffolding writes >=<engine> <major+1>.0.0."
          >
            <TextInput
              id="cart-engine"
              mono
              value={cart.engine ?? ""}
              onChange={(value) => patch({ engine: value.length > 0 ? value : undefined })}
            />
            <FieldFindings findings={findingsForPath(findings, "engine")} />
          </Field>
          <div className="kv-block">
            <p className="field-hint">
              Schema {String(cart.schema)}. Directory {project.dir}.
            </p>
            <p className="field-hint">
              {project.hasWorkflow ? "Release workflow present." : "No .github/workflows/release.yml yet."}{" "}
              {project.isGitRepo ? "This directory is a git repository." : "Not a git repository yet."}
            </p>
          </div>
        </Card>
      </div>

      <Card title="Live validation" subtitle="Errors fail. Warnings fail an export. Notes never fail anything.">
        <FindingsList findings={findings} />
      </Card>
    </div>
  );
}
