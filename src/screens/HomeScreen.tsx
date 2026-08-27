import { useCallback, useEffect, useState } from "react";
import { Dialog } from "../components/Dialog";
import { useListNavigation } from "../components/useListNavigation";
import {
  Banner,
  Button,
  Card,
  Chip,
  CommandRow,
  EmptyState,
  Field,
  KeyValue,
  Spinner,
  TextInput,
} from "../components/ui";
import { api, errorMessage, errorSuggestion } from "../lib/backend";
import { BASE_LABELS } from "../lib/constants";
import { pickDirectory } from "../lib/dialogs";
import { useFileDrop } from "../lib/fileDrop";
import { formatRelative, shortenPath } from "../lib/format";
import type { GhStatus, InstallInstructions, ToolStatus } from "../lib/types";
import { useStore } from "../state/store";

export function HomeScreen(): JSX.Element {
  const { state, go, openProject, refreshEnvironment, dispatch, toast } = useStore();
  const environment = state.environment;
  const recents = state.settings?.recentProjects ?? [];
  const [cursor, setCursor] = useState(0);

  const onOpen = useCallback(async () => {
    const dir = await pickDirectory("Choose a cart directory", environment?.paths.projects);
    if (!dir) return;
    await openProject(dir);
  }, [environment?.paths.projects, openProject]);

  const onDrop = useCallback(
    (paths: string[]) => {
      const first = paths[0];
      if (first) void openProject(first);
    },
    [openProject],
  );
  const { hovering } = useFileDrop(onDrop);

  const nav = useListNavigation(recents.length, cursor, setCursor, (position) => {
    const entry = recents[position];
    if (entry) void openProject(entry.path);
  });

  const onForget = useCallback(
    async (path: string) => {
      try {
        const settings = await api.projects.forget(path);
        dispatch({ type: "settings/loaded", settings });
      } catch (problem) {
        toast("error", errorMessage(problem), errorSuggestion(problem));
      }
    },
    [dispatch, toast],
  );

  if (!environment) {
    return (
      <div className="screen">
        <Spinner label="Checking your environment" />
      </div>
    );
  }

  return (
    <div className={`screen${hovering ? " screen-dropping" : ""}`}>
      {hovering ? <Banner tone="note">Drop a cart directory to open it.</Banner> : null}
      <div className="screen-head">
        <div>
          <h1>Cart Maker</h1>
          <p className="screen-sub">
            Assemble a cart for the G1R engine, design its label, and either export a .g1rcart or publish
            its repository.
          </p>
        </div>
        <div className="screen-head-actions">
          <Button variant="primary" onClick={() => go("new")}>
            New Cart
          </Button>
          <Button onClick={() => void onOpen()}>Open Cart</Button>
        </div>
      </div>

      <div className="grid-2">
        <Card
          title="Toolchain"
          subtitle={`Detected on ${environment.os}, ${environment.arch}`}
          actions={<Button small onClick={() => void refreshEnvironment(true)}>Re-check</Button>}
        >
          <ToolRow name="git" tool={environment.git} />
          <ToolRow name="gh" tool={environment.gh} />
          <AuthRow gh={environment.gh} />
        </Card>

        <Card title="Git identity" subtitle="A commit with no identity fails confusingly.">
          <IdentityPanel />
        </Card>
      </div>

      <Card
        title="Recent carts"
        subtitle={recents.length > 0 ? `${recents.length} cart${recents.length === 1 ? "" : "s"}` : undefined}
      >
        {recents.length === 0 ? (
          <EmptyState
            title="No carts yet"
            body="A cart is a directory holding cart.json, its label, and a release workflow. Create one, or open a directory you already have."
            action={
              <>
                <Button variant="primary" onClick={() => go("new")}>
                  New Cart
                </Button>
                <Button onClick={() => void onOpen()}>Open Cart</Button>
              </>
            }
          />
        ) : (
          <ul className="recent-list" onKeyDown={nav.onKeyDown} role="list">
            {recents.map((entry, index) => (
              <li key={entry.path} className="recent-row">
                <button
                  type="button"
                  data-nav-item
                  className="recent-main"
                  {...nav.itemProps(index)}
                  onClick={() => void openProject(entry.path)}
                >
                  <span className="recent-title">{entry.title}</span>
                  <span className="recent-meta">
                    {entry.id} &middot; {BASE_LABELS[entry.base] ?? entry.base} &middot; opened {formatRelative(entry.openedAt)}
                  </span>
                  <span className="recent-path" title={entry.path}>
                    {shortenPath(entry.path, 64)}
                  </span>
                </button>
                <Button small onClick={() => void onForget(entry.path)} ariaLabel={`Forget ${entry.title}`}>
                  Forget
                </Button>
              </li>
            ))}
          </ul>
        )}
      </Card>
    </div>
  );
}

function ToolRow({ name, tool }: { name: "git" | "gh"; tool: ToolStatus }): JSX.Element {
  const [instructions, setInstructions] = useState<InstallInstructions | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const { refreshEnvironment } = useStore();

  useEffect(() => {
    if (tool.found) {
      setInstructions(null);
      return;
    }
    let cancelled = false;
    api.env
      .instructions(name)
      .then((value) => {
        if (!cancelled) setInstructions(value);
      })
      .catch((problem: unknown) => {
        if (!cancelled) setFailure(errorMessage(problem));
      });
    return () => {
      cancelled = true;
    };
  }, [name, tool.found]);

  return (
    <div className="tool-row">
      <div className="tool-head">
        <Chip tone={tool.found ? "ok" : "error"}>{tool.found ? "Found" : "Missing"}</Chip>
        <strong>{name}</strong>
        {tool.found ? (
          <span className="tool-detail">
            {tool.version ?? "unknown version"}
            {tool.path ? ` at ${tool.path}` : ""}
          </span>
        ) : (
          <span className="tool-detail">Not on PATH.</span>
        )}
      </div>
      {!tool.found ? (
        <div className="tool-instructions">
          {failure ? (
            <Banner tone="warn">Could not load install steps: {failure}</Banner>
          ) : instructions ? (
            <>
              <p className="tool-instructions-head">Install it, then use Re-check.</p>
              {instructions.steps.map((step, index) => (
                <div key={index} className="tool-step">
                  <span className="tool-step-label">{step.label}</span>
                  {step.command ? <CommandRow command={step.command} /> : null}
                  {step.url ? (
                    <button type="button" className="link" onClick={() => void api.env.openUrl(step.url ?? "")}>
                      {step.url}
                    </button>
                  ) : null}
                </div>
              ))}
              <p className="tool-note">
                A newly installed tool may need a new terminal or an app restart before it appears on PATH.
              </p>
              <Button small onClick={() => void refreshEnvironment(true)}>
                Re-check
              </Button>
            </>
          ) : (
            <Spinner label={`Loading ${name} install steps`} />
          )}
        </div>
      ) : null}
    </div>
  );
}

function AuthRow({ gh }: { gh: GhStatus }): JSX.Element {
  if (!gh.found) {
    return <Banner tone="warn">Install gh to create a repository from this app. Export to a file works without it.</Banner>;
  }
  const credential = gh.tokenEnv
    ? `${gh.tokenEnv} from your environment takes precedence over gh's stored credential.`
    : gh.authenticated
      ? "gh's own stored credential will be used."
      : "No credential is available yet.";
  return (
    <div className="auth-row">
      <KeyValue label="Authentication">
        <Chip tone={gh.authenticated ? "ok" : "warn"}>{gh.authenticated ? "Signed in" : "Not signed in"}</Chip>
        {gh.account ? <span className="tool-detail">as {gh.account}</span> : null}
      </KeyValue>
      <KeyValue label="Credential">{credential}</KeyValue>
      {gh.protocol ? <KeyValue label="Git protocol">{gh.protocol}</KeyValue> : null}
      {!gh.authenticated ? (
        <>
          <p className="tool-instructions-head">
            Sign in from your own terminal. This app never handles or stores a token.
          </p>
          <CommandRow command="gh auth login" />
        </>
      ) : null}
    </div>
  );
}

function IdentityPanel(): JSX.Element {
  const { state, refreshEnvironment, run, toast } = useStore();
  const identity = state.environment?.identity ?? { name: null, email: null };
  const [editing, setEditing] = useState(false);
  const [name, setName] = useState(identity.name ?? "");
  const [email, setEmail] = useState(identity.email ?? "");

  const complete = Boolean(identity.name && identity.email);

  const submit = useCallback(async () => {
    if (name.trim().length === 0 || !email.includes("@")) {
      toast("error", "A name and a valid email address are both required.");
      return;
    }
    const done = await run("Setting the git identity", "Git identity", () =>
      api.env.setGitIdentity(name.trim(), email.trim(), state.project?.dir ?? null),
    );
    if (done !== null) {
      setEditing(false);
      toast("ok", "Git identity set.");
      await refreshEnvironment(true);
    }
  }, [email, name, refreshEnvironment, run, state.project?.dir, toast]);

  return (
    <>
      <KeyValue label="Name">{identity.name ?? <span className="muted">not set</span>}</KeyValue>
      <KeyValue label="Email">{identity.email ?? <span className="muted">not set</span>}</KeyValue>
      {complete ? (
        <Button small onClick={() => setEditing(true)}>
          Change
        </Button>
      ) : (
        <>
          <Banner tone="warn">Git has no identity here. A commit will fail until one is set.</Banner>
          <Button variant="primary" small onClick={() => setEditing(true)}>
            Set identity
          </Button>
        </>
      )}
      {editing ? (
        <Dialog
          title="Git identity"
          description={
            state.project
              ? "Set locally for the open cart directory when it becomes a repository."
              : "Set globally, since no cart directory is open."
          }
          onClose={() => setEditing(false)}
          onSubmit={() => void submit()}
          footer={
            <>
              <Button onClick={() => setEditing(false)}>Cancel</Button>
              <Button variant="primary" onClick={() => void submit()}>
                Save identity
              </Button>
            </>
          }
        >
          <Field label="Name" htmlFor="identity-name">
            <TextInput id="identity-name" value={name} onChange={setName} placeholder="Your name" autoFocus />
          </Field>
          <Field label="Email" htmlFor="identity-email">
            <TextInput id="identity-email" value={email} onChange={setEmail} placeholder="you@example.com" />
          </Field>
        </Dialog>
      ) : null}
    </>
  );
}
