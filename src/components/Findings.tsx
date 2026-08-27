import { groupFindings, SEVERITY_ORDER, SEVERITY_PLURAL } from "../lib/findings";
import type { UiFinding, UiSeverity } from "../lib/validate";
import { EmptyState } from "./ui";

const TONE: Record<UiSeverity, string> = { error: "error", warn: "warn", note: "note" };

export function SeverityCount({ severity, count }: { severity: UiSeverity; count: number }): JSX.Element {
  return (
    <span className={`sev-count sev-${TONE[severity]}${count === 0 ? " sev-zero" : ""}`}>
      <span className="sev-dot" aria-hidden="true" />
      {count} {SEVERITY_PLURAL[severity].toLowerCase()}
    </span>
  );
}

export function FindingRow({ finding }: { finding: UiFinding }): JSX.Element {
  return (
    <li className={`finding finding-${TONE[finding.severity]}`}>
      <span className="finding-badge">{finding.rule.length > 0 ? finding.rule : "note"}</span>
      <div className="finding-text">
        <p>{finding.message}</p>
        {finding.path ? <code className="finding-path">{finding.path}</code> : null}
      </div>
    </li>
  );
}

/** The three severities are always rendered as three sections and never merged. */
export function FindingsList({
  findings,
  emptyTitle = "Nothing to report",
  emptyBody = "This cart passes every offline check.",
}: {
  findings: UiFinding[];
  emptyTitle?: string;
  emptyBody?: string;
}): JSX.Element {
  const groups = groupFindings(findings);
  if (findings.length === 0) return <EmptyState title={emptyTitle} body={emptyBody} />;
  return (
    <div className="findings">
      {SEVERITY_ORDER.map((severity) => {
        const list = groups[severity];
        if (list.length === 0) return null;
        return (
          <section key={severity} className={`findings-group findings-${TONE[severity]}`}>
            <h4>
              {SEVERITY_PLURAL[severity]} <span className="findings-count">{list.length}</span>
            </h4>
            <ul>
              {list.map((finding, index) => (
                <FindingRow key={`${severity}-${finding.rule}-${index}`} finding={finding} />
              ))}
            </ul>
          </section>
        );
      })}
    </div>
  );
}

export function FieldFindings({ findings }: { findings: UiFinding[] }): JSX.Element | null {
  if (findings.length === 0) return null;
  return (
    <ul className="field-findings">
      {findings.map((finding, index) => (
        <li key={index} className={`field-finding field-finding-${TONE[finding.severity]}`}>
          <span className="finding-badge">{finding.rule.length > 0 ? finding.rule : "note"}</span>
          {finding.message}
        </li>
      ))}
    </ul>
  );
}
