import { useCallback, useEffect, useMemo, useState } from "react";
import { Dialog } from "../components/Dialog";
import { Banner, Button, Chip, Field, Select, Spinner, TextInput, Toggle } from "../components/ui";
import { api, errorMessage } from "../lib/backend";
import { LIMITS } from "../lib/constants";
import { pickDirectory } from "../lib/dialogs";
import {
  coerceForRow,
  formatScalar,
  optionCountProblem,
  parseRawOption,
  parseScalar,
  visibleRows,
  withDefaults,
} from "../lib/options";
import type { ModPin, OptionDiscovery, OptionRow, OptionValue } from "../lib/types";
import { useStore } from "../state/store";

export function OptionsEditor({
  pin,
  onClose,
  onSave,
}: {
  pin: ModPin;
  onClose: () => void;
  onSave: (options: Record<string, OptionValue>) => void;
}): JSX.Element {
  const { state, toast } = useStore();
  const [discovery, setDiscovery] = useState<OptionDiscovery | null>(null);
  const [loading, setLoading] = useState(true);
  const [failure, setFailure] = useState<string | null>(null);
  const [values, setValues] = useState<Record<string, OptionValue>>(pin.options ?? {});
  const [rawEntry, setRawEntry] = useState("");
  const [rawError, setRawError] = useState<string | null>(null);

  const load = useCallback(() => {
    setLoading(true);
    setFailure(null);
    api.pins
      .options(pin)
      .then((result) => setDiscovery(result))
      .catch((problem: unknown) => setFailure(errorMessage(problem)))
      .finally(() => setLoading(false));
  }, [pin]);

  useEffect(load, [load]);

  const rows = useMemo(() => discovery?.rows ?? [], [discovery]);
  const merged = useMemo(() => (rows.length > 0 ? withDefaults(rows, values) : values), [rows, values]);
  const countProblem = optionCountProblem(values);

  const setValue = useCallback((key: string, value: OptionValue) => {
    setValues((current) => ({ ...current, [key]: value }));
  }, []);

  const removeKey = useCallback((key: string) => {
    setValues((current) => {
      const next = { ...current };
      delete next[key];
      return next;
    });
  }, []);

  const addRaw = useCallback(() => {
    const parsed = parseRawOption(rawEntry);
    if ("error" in parsed) {
      setRawError(parsed.error);
      return;
    }
    setRawError(null);
    setRawEntry("");
    setValue(parsed.key, parsed.value);
  }, [rawEntry, setValue]);

  const fromInstall = useCallback(async () => {
    const dir = await pickDirectory("Choose your game's save directory", state.settings?.gamePath ?? undefined);
    if (!dir) return;
    try {
      const all = await api.pins.optionsFromInstall(dir);
      const found = all[pin.id];
      if (!found) {
        toast("info", `mod_option_schemas.json in that folder lists no rows for ${pin.id}.`, "Enable the mod and boot the game once so the engine writes its schema.");
        return;
      }
      setDiscovery(found);
      setFailure(null);
    } catch (problem) {
      toast("error", errorMessage(problem), "Point at the folder that holds options.lua.");
    }
  }, [pin.id, state.settings?.gamePath, toast]);

  const visible = visibleRows(rows, merged);
  const hiddenCount = rows.length - visible.length;
  const extraKeys = Object.keys(values).filter((key) => !rows.some((row) => row.key === key));

  return (
    <Dialog
      title={`Options for ${pin.id}`}
      description="Values written here are frozen into the cart and seeded into the player's scope on first boot."
      onClose={onClose}
      wide
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button
            variant="primary"
            disabled={countProblem !== null}
            onClick={() => onSave(values)}
          >
            Save options
          </Button>
        </>
      }
    >
      {loading ? <Spinner label="Reading the mod's option schema" /> : null}

      {!loading && failure ? (
        <Banner tone="warn">
          {failure}
          <Button small onClick={load}>
            Retry
          </Button>
        </Banner>
      ) : null}

      {!loading && discovery ? (
        <p className="options-source">
          {discovery.source === "archive"
            ? "Schema read from the mod archive's options_schema, evaluated in a sandbox."
            : discovery.source === "probe"
              ? "This mod registers its options at runtime, so these rows come from running its entry in a sandbox. They are the mod's own rows, but a mod that varies them by game or by what the engine reports may register a different set in play. Read from a game install for the exact set."
              : discovery.source === "install"
                ? "Schema read from mod_option_schemas.json in a real install."
                : "No schema could be read."}
          {discovery.error ? (
            <span className="muted">
              {" "}
              {discovery.source === "probe" ? "It stopped afterwards: " : null}
              {discovery.error}
            </span>
          ) : null}
        </p>
      ) : null}

      {!loading && rows.length === 0 ? (
        <Banner tone="note">
          No option metadata is available for this mod, so keys and values are edited raw. They are written exactly as
          typed.
          <Button small onClick={() => void fromInstall()}>
            Read from a game install
          </Button>
        </Banner>
      ) : null}

      {visible.length > 0 ? (
        <div className="option-rows">
          {visible.map((row) => (
            <OptionControl
              key={row.key}
              row={row}
              value={merged[row.key] ?? row.default}
              onChange={(value) => setValue(row.key, value)}
              onReset={() => removeKey(row.key)}
              overridden={row.key in values}
            />
          ))}
        </div>
      ) : null}

      {hiddenCount > 0 ? (
        <p className="field-hint">
          {hiddenCount} row{hiddenCount === 1 ? " is" : "s are"} hidden by a visible_if condition. Their values stay in
          the cart.
        </p>
      ) : null}

      <div className="raw-options">
        <h4>Raw keys</h4>
        {extraKeys.length === 0 ? (
          <p className="field-hint">No keys outside the schema.</p>
        ) : (
          <ul className="raw-list">
            {extraKeys.map((key) => (
              <li key={key}>
                <code>{key}</code>
                <TextInput
                  value={formatScalar(values[key] ?? "")}
                  onChange={(next) => setValue(key, parseScalar(next))}
                  mono
                />
                <Button small onClick={() => removeKey(key)} ariaLabel={`Remove ${key}`}>
                  Remove
                </Button>
              </li>
            ))}
          </ul>
        )}
        <Field label="Add a key" error={rawError} hint={`key=value. Keys up to ${LIMITS.optionKey}, values up to ${LIMITS.optionText} characters.`}>
          <div className="path-row">
            <TextInput value={rawEntry} onChange={setRawEntry} mono placeholder="difficulty=hard" onEnter={addRaw} />
            <Button onClick={addRaw}>Add</Button>
          </div>
        </Field>
      </div>

      {countProblem ? <Banner tone="error">{countProblem}</Banner> : null}
      <p className="field-hint">
        <Chip>{Object.keys(values).length} set</Chip> of at most {LIMITS.options}.
      </p>
    </Dialog>
  );
}

function OptionControl({
  row,
  value,
  onChange,
  onReset,
  overridden,
}: {
  row: OptionRow;
  value: OptionValue;
  onChange: (value: OptionValue) => void;
  onReset: () => void;
  overridden: boolean;
}): JSX.Element {
  const control = (() => {
    switch (row.type) {
      case "toggle":
        return <Toggle checked={value === true} onChange={onChange} label={row.label} />;
      case "choice":
        return (
          <Select
            value={String(value)}
            onChange={(next) => onChange(coerceForRow(row, next))}
            options={row.choices.map(([label, candidate]) => ({ value: String(candidate), label }))}
          />
        );
      case "number":
        return (
          <div className="number-row">
            <input
              className="input"
              type="number"
              value={typeof value === "number" ? value : row.default}
              min={row.min ?? undefined}
              max={row.max ?? undefined}
              step={row.step ?? undefined}
              aria-label={row.label}
              onChange={(event) => onChange(coerceForRow(row, event.target.value))}
            />
            <span className="muted">
              {row.min ?? "-"} to {row.max ?? "-"}
            </span>
          </div>
        );
      case "text":
        return (
          <TextInput
            value={typeof value === "string" ? value : String(value)}
            onChange={(next) => onChange(coerceForRow(row, next))}
          />
        );
    }
  })();

  return (
    <div className="option-row">
      <div className="option-head">
        <label>{row.label}</label>
        <code className="option-key">{row.key}</code>
        {overridden ? (
          <button type="button" className="link" onClick={onReset}>
            Reset to default
          </button>
        ) : (
          <span className="muted">default</span>
        )}
      </div>
      {row.type === "toggle" ? control : <div className="option-control">{control}</div>}
    </div>
  );
}
