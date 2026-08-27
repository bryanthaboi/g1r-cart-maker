import { Suspense, lazy, useCallback, useEffect, useMemo, useState } from "react";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { Dialog } from "./components/Dialog";
import { SeverityCount } from "./components/Findings";
import { Banner, Button, Spinner } from "./components/ui";
import { USING_FIXTURES, api, errorMessage, errorSuggestion } from "./lib/backend";
import { countBySeverity, mergeFindings, reportToFindings } from "./lib/findings";
import { baseName, shortenPath } from "./lib/format";
import type { ExportCheck, LabelDoc } from "./lib/types";
import { validateCart } from "./lib/validate";
import { CartScreen } from "./screens/CartScreen";
import { ExportScreen } from "./screens/ExportScreen";
import { HomeScreen } from "./screens/HomeScreen";
import { ModsScreen } from "./screens/ModsScreen";
import { NewCartScreen } from "./screens/NewCartScreen";
import { PublishScreen } from "./screens/PublishScreen";
import { SettingsScreen } from "./screens/SettingsScreen";
import { ValidateScreen } from "./screens/ValidateScreen";
import { requiresProject, type Route } from "./state/reducer";
import { useStore } from "./state/store";

// The label designer is owned by another module and is loaded on demand.
const LabelDesigner = lazy(() => import("./label/LabelDesigner"));

interface NavItem {
  route: Route;
  label: string;
  hint: string;
}

const PRIMARY_NAV: readonly NavItem[] = [
  { route: "home", label: "Home", hint: "Environment and recent carts" },
  { route: "cart", label: "Cart", hint: "Identity, base game, seal, speeds" },
  { route: "mods", label: "Mods", hint: "Pinned mods and load order" },
  { route: "label", label: "Label", hint: "Cartridge label designer" },
  { route: "validate", label: "Validate", hint: "Offline and online checks" },
  { route: "export", label: "Export", hint: "Write a .g1rcart bundle" },
  { route: "publish", label: "Publish", hint: "Prepare the GitHub repo" },
  { route: "settings", label: "Settings", hint: "Index sources, caches, theme" },
];

export function App(): JSX.Element {
  const { state, go, dispatch, saveDraft, closeProject, reloadProject } = useStore();
  const [pendingRoute, setPendingRoute] = useState<Route | null>(null);

  const findings = useMemo(() => {
    const live = state.draft ? validateCart(state.draft) : [];
    const saved = reportToFindings(state.project?.report ?? null);
    const online = reportToFindings(state.onlineReport);
    return mergeFindings(live, saved, online);
  }, [state.draft, state.project?.report, state.onlineReport]);

  const counts = useMemo(() => countBySeverity(findings), [findings]);

  const navigate = useCallback(
    (route: Route) => {
      if (route === state.route) return;
      if (state.dirty && requiresProject(state.route) && !requiresProject(route)) {
        setPendingRoute(route);
        return;
      }
      go(route);
    },
    [go, state.dirty, state.route],
  );

  useEffect(() => {
    const onKey = (event: globalThis.KeyboardEvent) => {
      const meta = event.metaKey || event.ctrlKey;
      if (meta && event.key.toLowerCase() === "s") {
        event.preventDefault();
        if (state.dirty) void saveDraft();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [saveDraft, state.dirty]);

  const availableNav = PRIMARY_NAV.filter((item) => item.route !== "new");

  return (
    <div className="shell">
      <header className="titlebar" data-tauri-drag-region>
        <div className="titlebar-name">G1R Cart Maker</div>
        <div className="titlebar-project">
          {state.project ? (
            <>
              <strong>{state.draft?.title ?? state.project.cart.title}</strong>
              <span className="titlebar-path" title={state.project.dir}>
                {shortenPath(state.project.dir)}
              </span>
              {state.dirty ? <span className="dot-dirty" title="Unsaved changes">Unsaved</span> : null}
            </>
          ) : (
            <span className="titlebar-path">No cart open</span>
          )}
        </div>
        <div className="titlebar-actions">
          {state.project ? (
            <>
              <Button small onClick={() => void saveDraft()} disabled={!state.dirty} variant={state.dirty ? "primary" : "default"}>
                Save
              </Button>
              <Button small onClick={() => void reloadProject()}>
                Reload
              </Button>
              <Button small onClick={() => (state.dirty ? setPendingRoute("home") : closeProject())}>
                Close
              </Button>
            </>
          ) : null}
        </div>
      </header>

      <div className="body">
        <nav className="sidebar" aria-label="Sections">
          <ul>
            {availableNav.map((item) => {
              const locked = requiresProject(item.route) && !state.project;
              return (
                <li key={item.route}>
                  <button
                    type="button"
                    className={`nav-item${state.route === item.route ? " nav-active" : ""}`}
                    aria-current={state.route === item.route ? "page" : undefined}
                    disabled={locked}
                    title={locked ? "Open or create a cart first" : item.hint}
                    onClick={() => navigate(item.route)}
                  >
                    <span className="nav-label">{item.label}</span>
                    <span className="nav-hint">{locked ? "Needs a cart" : item.hint}</span>
                  </button>
                </li>
              );
            })}
          </ul>
          <div className="sidebar-foot">
            {state.environment ? (
              <>
                <span>v{state.environment.appVersion}</span>
                <span>engine {state.environment.engineVersion}</span>
              </>
            ) : (
              <span>Starting up</span>
            )}
          </div>
        </nav>

        <main className="content" tabIndex={-1}>
          {USING_FIXTURES ? (
            <Banner tone="note">
              Browser preview. The Rust backend is not attached, so every command is answered from the local
              development fixtures.
            </Banner>
          ) : null}
          {state.error ? (
            <Banner tone="error">
              <strong>{state.error.context} failed.</strong> {state.error.message}
              {state.error.suggestion ? <span className="banner-hint">{state.error.suggestion}</span> : null}
              <Button small onClick={() => dispatch({ type: "error/clear" })}>
                Dismiss
              </Button>
            </Banner>
          ) : null}
          <ErrorBoundary key={state.route}>
            <Screen route={state.route} />
          </ErrorBoundary>
        </main>
      </div>

      <footer className="statusbar">
        <div className="statusbar-left">
          {state.busy ? <Spinner label={state.busy} /> : <span className="status-idle">Ready</span>}
        </div>
        <div className="statusbar-right">
          {state.project ? (
            <>
              <SeverityCount severity="error" count={counts.error} />
              <SeverityCount severity="warn" count={counts.warn} />
              <SeverityCount severity="note" count={counts.note} />
              <button type="button" className="status-link" onClick={() => navigate("validate")}>
                Open validation
              </button>
            </>
          ) : (
            <span className="status-idle">No cart open</span>
          )}
        </div>
      </footer>

      {state.toasts.length > 0 ? (
        <div className="toasts" role="status" aria-live="polite">
          {state.toasts.map((toast) => (
            <div key={toast.id} className={`toast toast-${toast.kind}`}>
              <div>
                <p>{toast.message}</p>
                {toast.suggestion ? <p className="toast-hint">{toast.suggestion}</p> : null}
              </div>
              <button type="button" onClick={() => dispatch({ type: "toast/dismiss", id: toast.id })} aria-label="Dismiss">
                &times;
              </button>
            </div>
          ))}
        </div>
      ) : null}

      {pendingRoute ? (
        <Dialog
          title="You have unsaved changes"
          description={`${baseName(state.project?.dir ?? "")} has edits that are not in cart.json yet.`}
          onClose={() => setPendingRoute(null)}
          footer={
            <>
              <Button onClick={() => setPendingRoute(null)}>Keep editing</Button>
              <Button
                variant="danger"
                onClick={() => {
                  const target = pendingRoute;
                  setPendingRoute(null);
                  dispatch({ type: "draft/reset" });
                  if (target === "home" && state.project) closeProject();
                  else if (target) go(target);
                }}
              >
                Discard changes
              </Button>
              <Button
                variant="primary"
                onClick={() => {
                  const target = pendingRoute;
                  setPendingRoute(null);
                  void saveDraft().then(() => {
                    if (target === "home" && state.project) closeProject();
                    else if (target) go(target);
                  });
                }}
              >
                Save and continue
              </Button>
            </>
          }
        >
          <p>Saving writes cart.json. Discarding restores the file as it is on disk.</p>
        </Dialog>
      ) : null}
    </div>
  );
}

function Screen({ route }: { route: Route }): JSX.Element {
  switch (route) {
    case "home":
      return <HomeScreen />;
    case "new":
      return <NewCartScreen />;
    case "cart":
      return <CartScreen />;
    case "mods":
      return <ModsScreen />;
    case "label":
      return <LabelTab />;
    case "validate":
      return <ValidateScreen />;
    case "export":
      return <ExportScreen />;
    case "publish":
      return <PublishScreen />;
    case "settings":
      return <SettingsScreen />;
  }
}

function LabelTab(): JSX.Element {
  const { state, toast, adopt } = useStore();
  const project = state.project;
  const [doc, setDoc] = useState<LabelDoc | null>(project?.labelDoc ?? null);

  useEffect(() => {
    setDoc(project?.labelDoc ?? null);
  }, [project?.labelDoc]);

  const onChange = useCallback(
    (next: LabelDoc) => {
      setDoc(next);
      if (!project) return;
      void api.label.writeDoc(project.dir, next).catch((problem: unknown) => {
        toast("error", errorMessage(problem), errorSuggestion(problem));
      });
    },
    [project, toast],
  );

  // The designer writes the PNG itself; the shell reloads so cart.json and the
  // label metadata agree again.
  const onExported = useCallback(
    (check: ExportCheck) => {
      if (!project) return;
      if (!check.ok) {
        toast("error", check.problems.join(" ") || "The manifest would reject that PNG.", "Reduce the size and export again.");
        return;
      }
      void api.projects
        .reload(project.dir)
        .then((reloaded) => adopt(reloaded))
        .catch((problem: unknown) => toast("error", errorMessage(problem), errorSuggestion(problem)));
      toast("ok", `Label written (${check.bytes} bytes).`, check.warnings[0] ?? null);
    },
    [adopt, project, toast],
  );

  if (!project) return <></>;
  const labelPath = typeof project.cart.label === "string" ? project.cart.label : "label.png";

  return (
    <Suspense fallback={<Spinner label="Loading the label designer" />}>
      <LabelDesigner
        doc={doc}
        onChange={onChange}
        cart={project.cart}
        labelPath={labelPath}
        dir={project.dir}
        onExported={onExported}
      />
    </Suspense>
  );
}
