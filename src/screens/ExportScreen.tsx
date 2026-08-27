import { useCallback, useMemo, useState } from "react";
import { FindingsList } from "../components/Findings";
import { Banner, Button, Card, KeyValue } from "../components/ui";
import { api } from "../lib/backend";
import { INSTALL_STEPS } from "../lib/constants";
import { pickSavePath } from "../lib/dialogs";
import { blocksExport, countBySeverity, mergeFindings, reportToFindings } from "../lib/findings";
import { formatBytes } from "../lib/format";
import { validateCart } from "../lib/validate";
import { useStore } from "../state/store";

interface ExportResult {
  path: string;
  bytes: number;
}

export function ExportScreen(): JSX.Element {
  const { state, run, go, saveDraft } = useStore();
  const project = state.project;
  const [result, setResult] = useState<ExportResult | null>(null);
  const [blocked, setBlocked] = useState<boolean>(false);

  const findings = useMemo(() => {
    if (!project) return [];
    return mergeFindings(state.draft ? validateCart(state.draft) : [], reportToFindings(project.report));
  }, [project, state.draft]);

  const counts = countBySeverity(findings);
  const strictFails = blocksExport(findings);

  const onExport = useCallback(async () => {
    if (!project) return;
    setResult(null);
    setBlocked(false);

    const report = await run("Running strict validation", "Export", () => api.projects.validate(project.dir));
    if (!report) return;
    const fresh = reportToFindings(report);
    if (blocksExport(fresh)) {
      setBlocked(true);
      return;
    }

    const name = await run("Naming the bundle", "Export", () => api.projects.bundleName(project.dir));
    if (!name) return;

    const target = await pickSavePath({
      title: "Export .g1rcart",
      defaultPath: name,
      extension: "g1rcart",
      extensionName: "G1R cart",
    });
    if (!target) return;

    const written = await run("Packing the bundle", "Export", () => api.projects.exportBundle(project.dir, target));
    if (written) setResult(written);
  }, [project, run]);

  if (!project) {
    return (
      <div className="screen">
        <Banner tone="note">Open or create a cart to export it.</Banner>
      </div>
    );
  }

  const cart = state.draft ?? project.cart;

  return (
    <div className="screen">
      <div className="screen-head">
        <div>
          <h1>Export</h1>
          <p className="screen-sub">
            Packing is always strict: an error or a warning refuses the bundle. Notes do not.
          </p>
        </div>
        <Button variant="primary" onClick={() => void onExport()} disabled={state.busy !== null}>
          Export .g1rcart
        </Button>
      </div>

      {state.dirty ? (
        <Banner tone="warn">
          Unsaved edits are not in cart.json, and the bundle is built from the file on disk.
          <Button small onClick={() => void saveDraft()}>
            Save first
          </Button>
        </Banner>
      ) : null}

      <Card title="Bundle">
        <KeyValue label="File name">
          <code>
            {cart.id}-{cart.version}.g1rcart
          </code>
        </KeyValue>
        <KeyValue label="Base game">{cart.base}</KeyValue>
        <KeyValue label="Seal">{cart.seal ?? "sealed"}</KeyValue>
        <KeyValue label="Pinned mods">{(cart.mods ?? []).length}</KeyValue>
        <KeyValue label="Label">
          {project.label.exists
            ? `${project.label.path ?? "label.png"}, ${formatBytes(project.label.bytes)}`
            : "none, the placeholder will be embedded"}
        </KeyValue>
      </Card>

      {strictFails || blocked ? (
        <Card title="Strict validation refuses this bundle" subtitle={`${counts.error} error(s), ${counts.warn} warning(s)`}>
          <Banner tone="error">
            Fix every error and warning first. Notes can be left alone.
            <Button small onClick={() => go("validate")}>
              Open validation
            </Button>
          </Banner>
          <FindingsList findings={findings.filter((finding) => finding.severity !== "note")} />
        </Card>
      ) : null}

      {result ? (
        <Card title="Exported">
          <Banner tone="ok">
            Written to <code>{result.path}</code> ({formatBytes(result.bytes)}).
          </Banner>
          <h4>How to install it</h4>
          <ol className="install-steps">
            {INSTALL_STEPS.map((step) => (
              <li key={step}>{step}</li>
            ))}
          </ol>
          <div className="form-actions">
            <Button onClick={() => void api.env.revealPath(result.path)}>Reveal in file manager</Button>
            <Button onClick={() => void onExport()}>Export again</Button>
          </div>
        </Card>
      ) : null}
    </div>
  );
}
