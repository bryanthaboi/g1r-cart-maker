import { useCallback, useEffect, useMemo, useState } from "react";
import { Dialog } from "../components/Dialog";
import {
  Banner,
  Button,
  Card,
  Chip,
  CommandRow,
  CopyButton,
  Field,
  KeyValue,
  ProgressBar,
  Spinner,
  TextArea,
  TextInput,
  Toggle,
} from "../components/ui";
import { api, errorMessage, errorSuggestion } from "../lib/backend";
import { readinessHeadline, summarizeReadiness } from "../lib/readiness";
import { IndexEntryDialog, type EntryFocus } from "./IndexEntryDialog";
import type { PublishProgress, PublishStep, ReadinessItem, ReadinessReport, SubmissionPlan } from "../lib/types";
import { validateRepo } from "../lib/validate";
import { useStore } from "../state/store";

const STEP_GUIDANCE: Record<string, string> = {
  write: "Check the cart directory is writable and not open in another program.",
  workflow: "The workflow file could not be written. Check write access to .github/workflows.",
  commit: "A commit needs a git identity. Set user.name and user.email on the Home tab.",
  create: "The name may already be taken on that account, or gh's credential may have expired. Run gh auth status.",
  tag: "That tag may already exist. Bump the cart version, or delete the tag on the remote.",
  run: "Actions may be disabled for the account, or the workflow failed validation. Open the run to read its log.",
  asset: "The workflow finished without attaching the bundle. Open the run log and check the pack step.",
};

export function PublishScreen(): JSX.Element {
  const { state, toast, go } = useStore();
  const project = state.project;
  const cart = project?.cart ?? null;
  const environment = state.environment;

  const [name, setName] = useState("");
  const [owner, setOwner] = useState("");
  const [description, setDescription] = useState("");
  const [isPrivate, setIsPrivate] = useState(false);
  const [runId, setRunId] = useState<string | null>(null);
  const [progress, setProgress] = useState<PublishProgress | null>(null);
  const [openLogs, setOpenLogs] = useState<Record<string, boolean>>({});
  const [readiness, setReadiness] = useState<ReadinessReport | null>(null);
  const [readinessError, setReadinessError] = useState<string | null>(null);
  const [showCompletion, setShowCompletion] = useState(false);
  const [plan, setPlan] = useState<SubmissionPlan | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [submitted, setSubmitted] = useState<string | null>(null);
  const [failure, setFailure] = useState<string | null>(null);

  useEffect(() => {
    if (!cart) return;
    const slug = cart.repo ?? "";
    const slash = slug.indexOf("/");
    setOwner(slash > 0 ? slug.slice(0, slash) : (environment?.gh.account ?? ""));
    setName(slash > 0 ? slug.slice(slash + 1) : cart.id);
    setDescription(cart.summary ?? "");
  }, [cart, environment?.gh.account]);

  const loadReadiness = useCallback(() => {
    if (!project) return;
    setReadinessError(null);
    api.projects
      .readiness(project.dir)
      .then(setReadiness)
      .catch((problem: unknown) => setReadinessError(errorMessage(problem)));
  }, [project]);

  useEffect(loadReadiness, [loadReadiness]);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let cancelled = false;
    api.publish
      .onProgress((next) => {
        setProgress(next);
        if (next.done && !next.failed) setShowCompletion(true);
      })
      .then((fn) => {
        if (cancelled) fn();
        else unlisten = fn;
      })
      .catch((problem: unknown) => setFailure(errorMessage(problem)));
    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, []);

  const preconditions = useMemo(() => {
    const items: { ok: boolean; label: string; detail: string }[] = [];
    if (environment) {
      items.push({
        ok: environment.git.found,
        label: "git",
        detail: environment.git.found ? (environment.git.version ?? "found") : "Not on PATH. See the Home tab.",
      });
      items.push({
        ok: environment.gh.found,
        label: "gh",
        detail: environment.gh.found ? (environment.gh.version ?? "found") : "Not on PATH. See the Home tab.",
      });
      items.push({
        ok: environment.gh.authenticated,
        label: "GitHub authentication",
        detail: environment.gh.authenticated
          ? `${environment.gh.account ?? "signed in"}${environment.gh.tokenEnv ? ` (${environment.gh.tokenEnv} takes precedence)` : ""}`
          : "Run gh auth login in your own terminal.",
      });
      items.push({
        ok: Boolean(environment.identity.name && environment.identity.email),
        label: "git identity",
        detail:
          environment.identity.name && environment.identity.email
            ? `${environment.identity.name} <${environment.identity.email}>`
            : "Unset. A commit will fail.",
      });
    }
    return items;
  }, [environment]);

  const ready = preconditions.length > 0 && preconditions.every((item) => item.ok);
  const nameError = name.trim().length === 0 ? "A repository name is required." : validateRepo(`${owner || "owner"}/${name}`);
  const running = progress !== null && !progress.done;

  const start = useCallback(async () => {
    if (!project || !cart) return;
    setFailure(null);
    setSubmitted(null);
    try {
      const id = await api.publish.start({
        dir: project.dir,
        owner: owner.trim().length > 0 ? owner.trim() : null,
        name: name.trim(),
        description: description.trim(),
        isPrivate,
        tag: `v${cart.version}`,
      });
      setRunId(id);
      const current = await api.publish.state(id).catch(() => null);
      if (current) setProgress(current);
    } catch (problem) {
      setFailure(errorMessage(problem));
    }
  }, [cart, description, isPrivate, name, owner, project]);

  const cancel = useCallback(async () => {
    if (!runId) return;
    try {
      await api.publish.cancel(runId);
      toast("info", "Publish cancelled.");
    } catch (problem) {
      setFailure(errorMessage(problem));
    }
  }, [runId, toast]);

  const openPlan = useCallback(async () => {
    if (!project) return;
    setShowCompletion(false);
    try {
      const next = await api.publish.submissionPlan(project.dir);
      setPlan(next);
    } catch (problem) {
      setFailure(errorMessage(problem));
    }
  }, [project]);

  const submit = useCallback(async () => {
    if (!project || !plan) return;
    setSubmitting(true);
    try {
      const result = await api.publish.submit(project.dir, plan);
      setSubmitted(result.url);
      setPlan(null);
      toast("ok", "Submission opened on the index repository.");
    } catch (problem) {
      setFailure(errorMessage(problem));
    } finally {
      setSubmitting(false);
    }
  }, [plan, project, toast]);

  const [entryFocus, setEntryFocus] = useState<EntryFocus>(null);
  const [entryOpen, setEntryOpen] = useState(false);
  const [fixing, setFixing] = useState(false);

  /// Every readiness fix id the backend can hand back lands here. An id with no
  /// case is a backend that grew a fix the window has not learned yet, so it
  /// says so rather than doing nothing.
  const runFix = useCallback(
    (item: ReadinessItem) => {
      if (!project || !cart) return;
      switch (item.fixId) {
        case "edit_entry":
          setEntryFocus(
            item.id === "thumbnail" || item.id === "description_url" || item.id === "license" || item.id === "tags"
              ? item.id
              : null,
          );
          setEntryOpen(true);
          return;
        case "edit_cart":
          go("cart");
          return;
        case "add_mod":
          go("mods");
          return;
        case "set_schema":
          setFixing(true);
          api.projects
            .save(project.dir, { ...cart, schema: 1 })
            .then(() => {
              toast("ok", "cart.json is schema 1 now.");
              loadReadiness();
            })
            .catch((problem: unknown) => setReadinessError(errorMessage(problem)))
            .finally(() => setFixing(false));
          return;
        case "recheck":
          loadReadiness();
          return;
        case "publish_release":
        case "rerun_release":
          document.getElementById("pub-owner")?.scrollIntoView({ behavior: "smooth", block: "center" });
          toast("info", "Fill in the repository above, then press Prepare GitHub repo.");
          return;
        default:
          setReadinessError(`This build has no action for "${item.fix ?? item.id}" yet.`);
      }
    },
    [project, cart, go, toast, loadReadiness],
  );

  if (!project || !cart) {
    return (
      <div className="screen">
        <Banner tone="note">Open or create a cart before preparing its repository.</Banner>
      </div>
    );
  }


  const summary = readiness ? summarizeReadiness(readiness.items) : null;
  const doneCount = progress ? progress.steps.filter((step) => step.state === "done").length : 0;

  return (
    <div className="screen">
      <div className="screen-head">
        <div>
          <h1>Publish</h1>
          <p className="screen-sub">
            Creates the repository, commits the cart source, pushes, tags v{cart.version}, and lets the release
            workflow publish the .g1rcart.
          </p>
        </div>
      </div>

      {failure ? (
        <Banner tone="error">
          {failure}
          {errorSuggestion(failure) ? <span className="banner-hint">{errorSuggestion(failure)}</span> : null}
        </Banner>
      ) : null}

      <Card title="Preconditions" subtitle="Everything below must be green before the flow can start.">
        {preconditions.length === 0 ? (
          <Spinner label="Checking your environment" />
        ) : (
          preconditions.map((item) => (
            <div key={item.label} className="precondition">
              <Chip tone={item.ok ? "ok" : "error"}>{item.ok ? "ok" : "missing"}</Chip>
              <strong>{item.label}</strong>
              <span className="tool-detail">{item.detail}</span>
            </div>
          ))
        )}
        {!ready && environment && !environment.gh.authenticated ? <CommandRow command="gh auth login" /> : null}
      </Card>

      {entryOpen && project ? (
        <IndexEntryDialog
          dir={project.dir}
          focus={entryFocus}
          onClose={() => setEntryOpen(false)}
          onSaved={() => {
            toast("ok", "index-entry.json saved.");
            loadReadiness();
          }}
        />
      ) : null}

      <Card title="Repository">
        <div className="grid-2">
          <Field label="Owner" htmlFor="pub-owner" hint="Leave blank to use the signed-in account.">
            <TextInput id="pub-owner" mono value={owner} onChange={setOwner} placeholder={environment?.gh.account ?? "owner"} />
          </Field>
          <Field label="Name" htmlFor="pub-name" error={nameError}>
            <TextInput id="pub-name" mono value={name} onChange={setName} invalid={Boolean(nameError)} />
          </Field>
        </div>
        <Field label="Description" htmlFor="pub-desc">
          <TextArea id="pub-desc" rows={2} value={description} onChange={setDescription} />
        </Field>
        <Toggle checked={isPrivate} onChange={setIsPrivate} label="Private repository" />
        {isPrivate ? (
          <Banner tone="warn">
            A private repository cannot be indexed. The community index needs to read cart.json and the release asset,
            so a private cart will never appear in the launcher's browser.
          </Banner>
        ) : null}
        <div className="form-actions">
          <Button
            variant="primary"
            disabled={!ready || Boolean(nameError) || running}
            onClick={() => void start()}
          >
            {running ? "Running" : "Prepare GitHub repo"}
          </Button>
          {running ? (
            <Button variant="danger" onClick={() => void cancel()}>
              Cancel
            </Button>
          ) : null}
        </div>
      </Card>

      {progress ? (
        <Card
          title="Progress"
          subtitle={`${doneCount} of ${progress.steps.length} steps complete`}
          actions={running ? <Button small variant="danger" onClick={() => void cancel()}>Cancel</Button> : undefined}
        >
          <ProgressBar value={progress.steps.length === 0 ? null : doneCount / progress.steps.length} label="Publish progress" />
          <ul className="step-list">
            {progress.steps.map((step) => (
              <StepRow
                key={step.id}
                step={step}
                open={openLogs[step.id] === true}
                onToggle={() => setOpenLogs((current) => ({ ...current, [step.id]: !current[step.id] }))}
              />
            ))}
          </ul>
          {progress.failed && progress.error ? (
            <Banner tone="error">
              {progress.error}
              {(() => {
                const failed = progress.steps.find((step) => step.state === "failed");
                const guidance = failed ? STEP_GUIDANCE[failed.id] : undefined;
                return guidance ? <span className="banner-hint">{guidance}</span> : null;
              })()}
            </Banner>
          ) : null}
        </Card>
      ) : null}

      <Card
        title="Index readiness"
        subtitle={summary ? readinessHeadline(summary) : undefined}
        actions={
          <Button small onClick={loadReadiness}>
            Re-check
          </Button>
        }
      >
        {readinessError ? (
          <Banner tone="error">{readinessError}</Banner>
        ) : !readiness ? (
          <Spinner label="Checking what the index needs" />
        ) : (
          <>
            {summary && summary.blocking.length > 0 ? (
              <div className="readiness-group">
                <h4>Blocking</h4>
                <ul className="readiness-list">
                  {summary.blocking.map((item) => (
                    <ReadinessRow key={item.id} item={item} onFix={runFix} busy={fixing} />
                  ))}
                </ul>
              </div>
            ) : null}
            {summary && summary.recommended.length > 0 ? (
              <div className="readiness-group">
                <h4>Recommended</h4>
                <ul className="readiness-list">
                  {summary.recommended.map((item) => (
                    <ReadinessRow key={item.id} item={item} onFix={runFix} busy={fixing} />
                  ))}
                </ul>
              </div>
            ) : null}
            {summary && summary.met.length > 0 ? (
              <div className="readiness-group">
                <h4>Met</h4>
                <ul className="readiness-list">
                  {summary.met.map((item) => (
                    <ReadinessRow key={item.id} item={item} onFix={runFix} busy={fixing} />
                  ))}
                </ul>
              </div>
            ) : null}
          </>
        )}
      </Card>

      {submitted ? (
        <Card title="Submitted">
          <Banner tone="ok">
            The submission is open at <code>{submitted}</code>.
            <Button small onClick={() => void api.env.openUrl(submitted)}>
              Open it
            </Button>
          </Banner>
        </Card>
      ) : null}

      {showCompletion && progress ? (
        <Dialog
          title="Repo created."
          onClose={() => setShowCompletion(false)}
          footer={
            <>
              <Button onClick={() => setShowCompletion(false)}>Not now</Button>
              <Button variant="primary" onClick={() => void openPlan()}>
                Submit
              </Button>
            </>
          }
        >
          <p className="completion-line">
            <strong>Repo created.</strong>{" "}
            <code>{(progress.repoUrl ?? "").replace(/^https?:\/\//, "")}</code>
          </p>
          <p className="completion-line">
            <strong>{progress.assetName ?? `${cart.id}-${cart.version}.g1rcart`} published</strong> on release{" "}
            <code>v{cart.version}</code>.
          </p>
          <div className="completion-actions">
            <Button onClick={() => progress.repoUrl && void api.env.openUrl(progress.repoUrl)} disabled={!progress.repoUrl}>
              Open repo
            </Button>
            <Button
              onClick={() => progress.releaseUrl && void api.env.openUrl(progress.releaseUrl)}
              disabled={!progress.releaseUrl}
            >
              Open release
            </Button>
            <CopyButton
              label="Copy install URL"
              text={`${progress.releaseUrl ?? progress.repoUrl ?? ""}/download/${progress.assetName ?? `${cart.id}-${cart.version}.g1rcart`}`.replace(
                "/tag/",
                "/download/",
              )}
            />
          </div>
          <p className="completion-question">
            <strong>Submit this cart repo to the index?</strong>
          </p>
        </Dialog>
      ) : null}

      {plan ? (
        <Dialog
          title="Review the submission"
          description={`${plan.kind === "issue" ? "An issue" : plan.kind === "pull_request" ? "A pull request" : "A manual submission"} against ${plan.repo}.`}
          onClose={() => setPlan(null)}
          wide
          footer={
            <>
              <Button onClick={() => setPlan(null)}>Cancel</Button>
              <Button variant="primary" disabled={submitting} onClick={() => void submit()}>
                {submitting ? "Submitting" : "Submit"}
              </Button>
            </>
          }
        >
          <Banner tone="note">{plan.guidance}</Banner>
          <Field label="Title">
            <TextInput value={plan.title} onChange={(value) => setPlan({ ...plan, title: value })} />
          </Field>
          {plan.fields.map((field, index) => (
            <Field
              key={field.id}
              label={`${field.label}${field.required ? " (required)" : ""}`}
              error={field.required && field.value.trim().length === 0 ? "This field is required." : null}
            >
              {field.multiline ? (
                <TextArea
                  value={field.value}
                  rows={3}
                  onChange={(value) => {
                    const fields = plan.fields.slice();
                    fields[index] = { ...field, value };
                    setPlan({ ...plan, fields });
                  }}
                />
              ) : (
                <TextInput
                  value={field.value}
                  onChange={(value) => {
                    const fields = plan.fields.slice();
                    fields[index] = { ...field, value };
                    setPlan({ ...plan, fields });
                  }}
                />
              )}
            </Field>
          ))}
          <Field label="Body">
            <TextArea value={plan.body} rows={6} onChange={(value) => setPlan({ ...plan, body: value })} />
          </Field>
          <KeyValue label="Destination">
            <code>{plan.url}</code>
          </KeyValue>
        </Dialog>
      ) : null}
    </div>
  );
}

function StepRow({ step, open, onToggle }: { step: PublishStep; open: boolean; onToggle: () => void }): JSX.Element {
  const tone =
    step.state === "done" ? "ok" : step.state === "failed" ? "error" : step.state === "running" ? "note" : "default";
  return (
    <li className={`step step-${step.state}`}>
      <div className="step-head">
        <Chip tone={tone === "default" ? "default" : tone}>{step.state}</Chip>
        <strong>{step.label}</strong>
        {step.detail ? <span className="tool-detail">{step.detail}</span> : null}
        <button
          type="button"
          className="link step-toggle"
          aria-expanded={open}
          onClick={onToggle}
          disabled={step.log.length === 0}
        >
          {step.log.length === 0 ? "no output" : open ? "Hide log" : "Show log"}
        </button>
      </div>
      {open && step.log.length > 0 ? <pre className="step-log">{step.log}</pre> : null}
      {step.state === "failed" && STEP_GUIDANCE[step.id] ? (
        <p className="step-guidance">{STEP_GUIDANCE[step.id]}</p>
      ) : null}
    </li>
  );
}

function ReadinessRow({
  item,
  onFix,
  busy,
}: {
  item: ReadinessItem;
  onFix: (item: ReadinessItem) => void;
  busy: boolean;
}): JSX.Element {
  return (
    <li className={`readiness readiness-${item.ok ? "ok" : item.blocking ? "error" : "warn"}`}>
      <Chip tone={item.ok ? "ok" : item.blocking ? "error" : "warn"}>{item.ok ? "met" : item.blocking ? "blocking" : "recommended"}</Chip>
      <div>
        <strong>{item.label}</strong>
        <p className="readiness-detail">{item.detail}</p>
        {!item.ok && item.fix ? (
          <Button small onClick={() => onFix(item)} disabled={busy}>
            {item.fix}
          </Button>
        ) : null}
      </div>
    </li>
  );
}
