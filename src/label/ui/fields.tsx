// Small form controls shared by the inspector panels.

import { type ChangeEvent, type ReactNode } from "react";
import { isHex, normaliseHex } from "../core/colour";

export interface FieldProps {
  label: string;
  children: ReactNode;
}

export function Field({ label, children }: FieldProps): JSX.Element {
  return (
    <label className="ld-field">
      <span className="ld-field__label">{label}</span>
      {children}
    </label>
  );
}

export interface NumberFieldProps {
  label: string;
  value: number;
  onChange: (value: number) => void;
  step?: number;
  min?: number;
  max?: number;
  disabled?: boolean;
}

export function NumberField(props: NumberFieldProps): JSX.Element {
  const handle = (event: ChangeEvent<HTMLInputElement>): void => {
    const parsed = Number.parseFloat(event.target.value);
    if (Number.isFinite(parsed)) props.onChange(parsed);
  };
  return (
    <Field label={props.label}>
      <input
        className="ld-input"
        type="number"
        value={Number.isFinite(props.value) ? Math.round(props.value * 100) / 100 : 0}
        step={props.step ?? 1}
        min={props.min}
        max={props.max}
        disabled={props.disabled ?? false}
        onChange={handle}
      />
    </Field>
  );
}

export interface TextFieldProps {
  label: string;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  disabled?: boolean;
}

export function TextField(props: TextFieldProps): JSX.Element {
  return (
    <Field label={props.label}>
      <input
        className="ld-input"
        type="text"
        value={props.value}
        placeholder={props.placeholder ?? ""}
        disabled={props.disabled ?? false}
        onChange={(event) => props.onChange(event.target.value)}
      />
    </Field>
  );
}

export interface TextAreaFieldProps {
  label: string;
  value: string;
  onChange: (value: string) => void;
  disabled?: boolean;
}

export function TextAreaField(props: TextAreaFieldProps): JSX.Element {
  return (
    <Field label={props.label}>
      <textarea
        className="ld-textarea"
        value={props.value}
        disabled={props.disabled ?? false}
        onChange={(event) => props.onChange(event.target.value)}
      />
    </Field>
  );
}

export interface SelectOption {
  value: string;
  label: string;
}

export interface SelectFieldProps {
  label: string;
  value: string;
  options: readonly SelectOption[];
  onChange: (value: string) => void;
  disabled?: boolean;
}

export function SelectField(props: SelectFieldProps): JSX.Element {
  return (
    <Field label={props.label}>
      <select
        className="ld-select"
        value={props.value}
        disabled={props.disabled ?? false}
        onChange={(event) => props.onChange(event.target.value)}
      >
        {props.options.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
    </Field>
  );
}

export interface ColourFieldProps {
  label: string;
  value: string | null;
  onChange: (value: string | null) => void;
  /** Allows the colour to be cleared, for optional strokes. */
  optional?: boolean;
  disabled?: boolean;
}

/** A native picker plus a hex box; typing keeps the last good value until it parses. */
export function ColourField(props: ColourFieldProps): JSX.Element {
  const active = props.value ?? "#000000";
  return (
    <Field label={props.label}>
      <span className="ld-colour">
        <input
          className="ld-colour__swatch"
          type="color"
          value={normaliseHex(active)}
          disabled={props.disabled ?? false}
          onChange={(event) => props.onChange(event.target.value.toLowerCase())}
          aria-label={`${props.label} picker`}
        />
        <input
          className="ld-input ld-colour__hex"
          type="text"
          value={props.value ?? ""}
          placeholder={props.optional ? "none" : "#rrggbb"}
          disabled={props.disabled ?? false}
          onChange={(event) => {
            const text = event.target.value.trim();
            if (text.length === 0 && props.optional) {
              props.onChange(null);
              return;
            }
            const candidate = text.startsWith("#") ? text : `#${text}`;
            if (isHex(candidate)) props.onChange(candidate.toLowerCase());
            else props.onChange(props.value);
          }}
          aria-label={`${props.label} hex`}
        />
        {props.optional && props.value !== null ? (
          <button
            type="button"
            className="ld-button ld-button--icon ld-colour__clear"
            onClick={() => props.onChange(null)}
            title="Clear"
          >
            x
          </button>
        ) : null}
      </span>
    </Field>
  );
}

export interface ToggleProps {
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
}

export function Toggle(props: ToggleProps): JSX.Element {
  return (
    <label className="ld-field">
      <span className="ld-field__label">
        <input
          type="checkbox"
          checked={props.checked}
          disabled={props.disabled ?? false}
          onChange={(event) => props.onChange(event.target.checked)}
        />{" "}
        {props.label}
      </span>
    </label>
  );
}
