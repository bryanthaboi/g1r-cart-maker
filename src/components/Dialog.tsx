import { useCallback, useEffect, useRef, type KeyboardEvent, type ReactNode } from "react";

const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

/** Escape always closes; Enter submits unless the focus is in a multi-line field. */
export function Dialog({
  title,
  description,
  onClose,
  onSubmit,
  children,
  footer,
  wide,
  closeLabel = "Close",
}: {
  title: string;
  description?: string;
  onClose: () => void;
  onSubmit?: () => void;
  children: ReactNode;
  footer?: ReactNode;
  wide?: boolean;
  closeLabel?: string;
}): JSX.Element {
  const panel = useRef<HTMLDivElement>(null);
  const restoreTo = useRef<Element | null>(null);

  useEffect(() => {
    restoreTo.current = document.activeElement;
    const first = panel.current?.querySelector<HTMLElement>(FOCUSABLE);
    if (first) first.focus();
    else panel.current?.focus();
    return () => {
      const target = restoreTo.current;
      if (target instanceof HTMLElement) target.focus();
    };
  }, []);

  const onKeyDown = useCallback(
    (event: KeyboardEvent<HTMLDivElement>) => {
      if (event.key === "Escape") {
        event.stopPropagation();
        onClose();
        return;
      }
      if (event.key === "Enter" && onSubmit) {
        const target = event.target;
        const multiline = target instanceof HTMLTextAreaElement;
        const isButton = target instanceof HTMLButtonElement;
        if (!multiline && !isButton && !event.shiftKey) {
          event.preventDefault();
          onSubmit();
        }
        return;
      }
      if (event.key !== "Tab") return;
      const nodes = panel.current?.querySelectorAll<HTMLElement>(FOCUSABLE);
      if (!nodes || nodes.length === 0) return;
      const first = nodes[0];
      const last = nodes[nodes.length - 1];
      if (!first || !last) return;
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    },
    [onClose, onSubmit],
  );

  return (
    <div className="dialog-backdrop" onMouseDown={(event) => event.target === event.currentTarget && onClose()}>
      <div
        ref={panel}
        className={`dialog${wide ? " dialog-wide" : ""}`}
        role="dialog"
        aria-modal="true"
        aria-label={title}
        tabIndex={-1}
        onKeyDown={onKeyDown}
      >
        <header className="dialog-head">
          <div>
            <h2>{title}</h2>
            {description ? <p className="dialog-sub">{description}</p> : null}
          </div>
          <button type="button" className="dialog-close" onClick={onClose} aria-label={closeLabel}>
            &times;
          </button>
        </header>
        <div className="dialog-body">{children}</div>
        {footer ? <footer className="dialog-foot">{footer}</footer> : null}
      </div>
    </div>
  );
}
