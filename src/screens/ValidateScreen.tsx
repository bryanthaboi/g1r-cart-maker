import { useCallback, useMemo, useState } from "react";
import { FindingsList, SeverityCount } from "../components/Findings";
import { Banner, Button, Card, Spinner } from "../components/ui";
import { api } from "../lib/backend";
import { countBySeverity, mergeFindings, reportToFindings, summarize } from "../lib/findings";
import { formatRelative } from "../lib/format";
import { validateCart } from "../lib/validate";
import { useStore } from "../state/store";

export function ValidateScreen(): JSX.Element {
  const { state, dispatch, run } = useStore();
  const project = state.project;
  const [checkedAt, setCheckedAt] = useState<string | null>(null);
  const online = navigator.onLine;

  const offline = useMemo(() => {
    if (!project) return [];
    return mergeFindings(state.draft ? validateCart(state.draft) : [], reportToFindings(project.report));
  }, [project, state.draft]);

  const onlineFindings = useMemo(() => reportToFindings(state.onlineReport), [state.onlineReport]);
  const counts = countBySeverity(mergeFindings(offline, onlineFindings));

  const runOnline = useCallback(async () => {
    if (!project) return;
    const report = await run("Checking every pin against its source", "Online validation", () =>
      api.projects.validateOnline(project.dir),
    );
    if (report) {
      dispatch({ type: "online/set", report });
      setCheckedAt(new Date().toISOString());
    }
  }, [dispatch, project, run]);

  const runOffline = useCallback(async () => {
    if (!project) return;
    const report = await run("Re-reading cart.json", "Validation", () => api.projects.validate(project.dir));
    if (report) {
      const reloaded = await api.projects.reload(project.dir).catch(() => null);
      if (reloaded) dispatch({ type: "project/loaded", project: reloaded, route: "validate" });
    }
  }, [dispatch, project, run]);

  if (!project) {
    return (
      <div className="screen">
        <Banner tone="note">Open or create a cart to validate it.</Banner>
      </div>
    );
  }

  return (
    <div className="screen">
      <div className="screen-head">
        <div>
          <h1>Validate</h1>
          <p className="screen-sub">
            {summarize(mergeFindings(offline, onlineFindings))}. An error fails the cart. A warning fails an export,
            because packing is always strict. A note never fails anything.
          </p>
        </div>
        <div className="screen-head-actions">
          <SeverityCount severity="error" count={counts.error} />
          <SeverityCount severity="warn" count={counts.warn} />
          <SeverityCount severity="note" count={counts.note} />
        </div>
      </div>

      {state.dirty ? (
        <Banner tone="warn">
          These results describe cart.json on disk plus your live edits. Save to make the two agree.
        </Banner>
      ) : null}

      <Card
        title="Offline checks"
        subtitle="Always available. Schema, vocabulary, limits, pin shape and load order."
        actions={
          <Button small onClick={() => void runOffline()}>
            Re-run
          </Button>
        }
      >
        <FindingsList findings={offline} />
      </Card>

      <Card
        title="Online checks"
        subtitle="Resolves every pin against GitHub and GameBanana and compares the published hash."
        actions={
          <Button variant="primary" small onClick={() => void runOnline()} disabled={!online || state.busy !== null}>
            Also check every pin online
          </Button>
        }
      >
        {!online ? (
          <Banner tone="warn">
            This machine is offline. Offline validation, label design and local export all still work; only pin
            resolution needs the network.
          </Banner>
        ) : null}

        {state.busy === "Checking every pin against its source" ? (
          <Spinner label="Contacting GitHub and GameBanana" />
        ) : null}

        {state.onlineReport === null ? (
          <p className="field-hint">
            Not run yet. An API this app could not reach is reported as a note, so a rate limit or a dropped
            connection never fails a cart that is fine.
          </p>
        ) : (
          <>
            <p className="field-hint">Last checked {formatRelative(checkedAt)}.</p>
            <FindingsList
              findings={onlineFindings}
              emptyTitle="Every pin matched"
              emptyBody="Each pinned build resolved and its published hash matched what the cart records."
            />
          </>
        )}
      </Card>
    </div>
  );
}
