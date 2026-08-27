import {
  forwardRef,
  useCallback,
  useId,
  useRef,
  useState,
  type ChangeEvent,
  type KeyboardEvent,
  type ReactNode,
} from "react";
import { copyToClipboard } from "../lib/dialogs";

export type ButtonVariant = "primary" | "default" | "danger";

interface ButtonProps {
  children: ReactNode;
  onClick?: () => void;
  variant?: ButtonVariant;
  disabled?: boolean;
  type?: "button" | "submit";
  title?: string;
  small?: boolean;
  wide?: boolean;
  ariaLabel?: string;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(props, ref) {
  const { children, onClick, variant = "default", disabled, type = "button", title, small, wide, ariaLabel } = props;
  return (
    <button
      ref={ref}
      type={type}
      className={`btn btn-${variant}${small ? " btn-small" : ""}${wide ? " btn-wide" : ""}`}
      onClick={onClick}
      disabled={disabled}
      title={title}
      aria-label={ariaLabel}
    >
      {children}
    </button>
  );
});

export function Field({
  label,
  hint,
  error,
  children,
  htmlFor,
  counter,
}: {
  label: string;
  hint?: string;
  error?: string | null;
  children: ReactNode;
  htmlFor?: string;
  counter?: string;
}): JSX.Element {
  return (
    <div className={`field${error ? " field-invalid" : ""}`}>
      <div className="field-head">
        <label htmlFor={htmlFor}>{label}</label>
        {counter ? <span className="field-counter">{counter}</span> : null}
      </div>
      {children}
      {error ? (
        <p className="field-error" role="alert">
          {error}
        </p>
      ) : hint ? (
        <p className="field-hint">{hint}</p>
      ) : null}
    </div>
  );
}

export function TextInput({
  value,
  onChange,
  placeholder,
  id,
  invalid,
  mono,
  disabled,
  onEnter,
  type = "text",
  autoFocus,
}: {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  id?: string;
  invalid?: boolean;
  mono?: boolean;
  disabled?: boolean;
  onEnter?: () => void;
  type?: string;
  autoFocus?: boolean;
}): JSX.Element {
  return (
    <input
      id={id}
      type={type}
      className={`input${mono ? " input-mono" : ""}`}
      value={value}
      placeholder={placeholder}
      disabled={disabled}
      aria-invalid={invalid ? true : undefined}
      autoFocus={autoFocus}
      onChange={(event: ChangeEvent<HTMLInputElement>) => onChange(event.target.value)}
      onKeyDown={(event: KeyboardEvent<HTMLInputElement>) => {
        if (event.key === "Enter" && onEnter) {
          event.preventDefault();
          onEnter();
        }
      }}
    />
  );
}

export function TextArea({
  value,
  onChange,
  placeholder,
  id,
  rows = 3,
  invalid,
}: {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  id?: string;
  rows?: number;
  invalid?: boolean;
}): JSX.Element {
  return (
    <textarea
      id={id}
      className="input textarea"
      rows={rows}
      value={value}
      placeholder={placeholder}
      aria-invalid={invalid ? true : undefined}
      onChange={(event) => onChange(event.target.value)}
    />
  );
}

export function Select<T extends string>({
  value,
  onChange,
  options,
  id,
  disabled,
}: {
  value: T;
  onChange: (value: T) => void;
  options: readonly { value: T; label: string }[];
  id?: string;
  disabled?: boolean;
}): JSX.Element {
  return (
    <select
      id={id}
      className="input select"
      value={value}
      disabled={disabled}
      onChange={(event) => onChange(event.target.value as T)}
    >
      {options.map((option) => (
        <option key={option.value} value={option.value}>
          {option.label}
        </option>
      ))}
    </select>
  );
}

export function Toggle({
  checked,
  onChange,
  label,
  disabled,
}: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: string;
  disabled?: boolean;
}): JSX.Element {
  const id = useId();
  return (
    <div className="toggle-row">
      <button
        id={id}
        type="button"
        role="switch"
        aria-checked={checked}
        aria-label={label}
        className={`toggle${checked ? " toggle-on" : ""}`}
        disabled={disabled}
        onClick={() => onChange(!checked)}
      >
        <span className="toggle-knob" />
      </button>
      <span className="toggle-label">{label}</span>
    </div>
  );
}

export function ColourPicker({
  value,
  onChange,
  id,
}: {
  value: string;
  onChange: (value: string) => void;
  id?: string;
}): JSX.Element {
  const safe = /^#[0-9a-fA-F]{6}$/.test(value) ? value : "#000000";
  return (
    <div className="colour-row">
      <input
        type="color"
        className="colour-swatch"
        aria-label="Shell colour"
        value={safe}
        onChange={(event) => onChange(event.target.value)}
      />
      <input
        id={id}
        className="input input-mono colour-text"
        value={value}
        spellCheck={false}
        onChange={(event) => onChange(event.target.value)}
      />
    </div>
  );
}

export function Chip({
  children,
  tone = "default",
  onClick,
  active,
  title,
}: {
  children: ReactNode;
  tone?: "default" | "ok" | "warn" | "error" | "note";
  onClick?: () => void;
  active?: boolean;
  title?: string;
}): JSX.Element {
  if (onClick) {
    return (
      <button
        type="button"
        className={`chip chip-${tone}${active ? " chip-active" : ""}`}
        onClick={onClick}
        aria-pressed={active}
        title={title}
      >
        {children}
      </button>
    );
  }
  return (
    <span className={`chip chip-${tone}`} title={title}>
      {children}
    </span>
  );
}

export function Card({
  title,
  subtitle,
  actions,
  children,
}: {
  title?: string;
  subtitle?: string;
  actions?: ReactNode;
  children: ReactNode;
}): JSX.Element {
  return (
    <section className="card">
      {title ? (
        <header className="card-head">
          <div>
            <h2>{title}</h2>
            {subtitle ? <p className="card-sub">{subtitle}</p> : null}
          </div>
          {actions ? <div className="card-actions">{actions}</div> : null}
        </header>
      ) : null}
      <div className="card-body">{children}</div>
    </section>
  );
}

export function EmptyState({
  title,
  body,
  action,
}: {
  title: string;
  body: string;
  action?: ReactNode;
}): JSX.Element {
  return (
    <div className="empty">
      <h3>{title}</h3>
      <p>{body}</p>
      {action ? <div className="empty-action">{action}</div> : null}
    </div>
  );
}

export function ErrorState({
  title,
  message,
  suggestion,
  onRetry,
  retryLabel = "Try again",
}: {
  title: string;
  message: string;
  suggestion?: string | null;
  onRetry?: () => void;
  retryLabel?: string;
}): JSX.Element {
  return (
    <div className="error-state" role="alert">
      <h3>{title}</h3>
      <p className="error-message">{message}</p>
      {suggestion ? <p className="error-suggestion">{suggestion}</p> : null}
      {onRetry ? (
        <Button onClick={onRetry} small>
          {retryLabel}
        </Button>
      ) : null}
    </div>
  );
}

export function Spinner({ label }: { label: string }): JSX.Element {
  return (
    <div className="spinner-row" role="status" aria-live="polite">
      <span className="spinner" aria-hidden="true" />
      <span>{label}</span>
    </div>
  );
}

export function ProgressBar({ value, label }: { value: number | null; label: string }): JSX.Element {
  const clamped = value === null ? null : Math.max(0, Math.min(1, value));
  return (
    <div
      className="progress"
      role="progressbar"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={clamped === null ? undefined : Math.round(clamped * 100)}
    >
      <div
        className={`progress-fill${clamped === null ? " progress-indeterminate" : ""}`}
        style={clamped === null ? undefined : { width: `${clamped * 100}%` }}
      />
    </div>
  );
}

export function CopyButton({ text, label = "Copy" }: { text: string; label?: string }): JSX.Element {
  const [copied, setCopied] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const onCopy = useCallback(async () => {
    const ok = await copyToClipboard(text);
    setCopied(ok);
    if (timer.current !== null) clearTimeout(timer.current);
    timer.current = setTimeout(() => setCopied(false), 1600);
  }, [text]);
  return (
    <Button small onClick={() => void onCopy()} ariaLabel={`${label} ${text}`}>
      {copied ? "Copied" : label}
    </Button>
  );
}

export function CommandRow({ command }: { command: string }): JSX.Element {
  return (
    <div className="command-row">
      <code>{command}</code>
      <CopyButton text={command} />
    </div>
  );
}

export function KeyValue({ label, children }: { label: string; children: ReactNode }): JSX.Element {
  return (
    <div className="kv">
      <span className="kv-label">{label}</span>
      <span className="kv-value">{children}</span>
    </div>
  );
}

export function Banner({
  tone,
  children,
}: {
  tone: "ok" | "warn" | "error" | "note";
  children: ReactNode;
}): JSX.Element {
  return (
    <div className={`banner banner-${tone}`} role={tone === "error" ? "alert" : undefined}>
      {children}
    </div>
  );
}
