import { useCallback, useEffect, useState } from "react";
import { Banner, Button, Card, Chip, EmptyState, Field, KeyValue, Select, Spinner, TextInput } from "../components/ui";
import { api, errorMessage, errorSuggestion } from "../lib/backend";
import { pickDirectory, pickSavePath } from "../lib/dialogs";
import { formatBytes } from "../lib/format";
import type { CacheUsage, IndexSource, Settings } from "../lib/types";
import { useStore } from "../state/store";

const THEMES = [
  { value: "system" as const, label: "Match the system" },
  { value: "light" as const, label: "Light" },
  { value: "dark" as const, label: "Dark" },
];

export function SettingsScreen(): JSX.Element {
  const { state, applySettings, dispatch, run, toast } = useStore();
  const settings = state.settings;
  const environment = state.environment;

  const [sources, setSources] = useState<IndexSource[]>([]);
  const [sourceUrl, setSourceUrl] = useState("");
  const [sourceError, setSourceError] = useState<string | null>(null);
  const [usage, setUsage] = useState<CacheUsage | null>(null);
  const [usageError, setUsageError] = useState<string | null>(null);
  const [engineVersion, setEngineVersion] = useState("");
  const [checkingEngine, setCheckingEngine] = useState(false);

  useEffect(() => {
    setEngineVersion(settings?.engineVersion ?? "");
  }, [settings?.engineVersion]);

  useEffect(() => {
    api.feeds
      .sources()
      .then(setSources)
      .catch((problem: unknown) => setSourceError(errorMessage(problem)));
  }, []);

  const loadUsage = useCallback(() => {
    setUsageError(null);
    api.settings
      .cacheUsage()
      .then(setUsage)
      .catch((problem: unknown) => setUsageError(errorMessage(problem)));
  }, []);

  useEffect(loadUsage, [loadUsage]);

  const addSource = useCallback(async () => {
    const url = sourceUrl.trim();
    if (url.length === 0) {
      setSourceError("Paste a Pages root, a feed URL, a GitHub repo page, or owner/repo.");
      return;
    }
    try {
      const next = await api.feeds.addSource(url);
      setSources(next);
      setSourceUrl("");
      setSourceError(null);
      toast("ok", "Index source added.");
    } catch (problem) {
      setSourceError(errorMessage(problem));
    }
  }, [sourceUrl, toast]);

  const removeSource = useCallback(
    async (id: string) => {
      try {
        setSources(await api.feeds.removeSource(id));
      } catch (problem) {
        setSourceError(errorMessage(problem));
      }
    },
    [],
  );

  const clearCache = useCallback(
    async (kind: "feeds" | "archives" | "logs" | "all") => {
      try {
        setUsage(await api.settings.clearCache(kind));
        toast("ok", kind === "all" ? "Caches cleared." : `${kind} cache cleared.`);
      } catch (problem) {
        toast("error", errorMessage(problem), errorSuggestion(problem));
      }
    },
    [toast],
  );

  const exportData = useCallback(async () => {
    const target = await pickSavePath({
      title: "Export app data",
      defaultPath: "g1r-cart-maker-data.zip",
      extension: "zip",
      extensionName: "Zip archive",
    });
    if (!target) return;
    try {
      const written = await api.settings.exportData(target);
      toast("ok", `Data exported to ${written}.`);
    } catch (problem) {
      toast("error", errorMessage(problem), errorSuggestion(problem));
    }
  }, [toast]);

  const update = useCallback(
    (patch: Partial<Settings>) => {
      if (!settings) return;
      void applySettings({ ...settings, ...patch });
    },
    [applySettings, settings],
  );

  const chooseGamePath = useCallback(async () => {
    const dir = await pickDirectory("Choose your game's save directory", settings?.gamePath ?? undefined);
    if (dir) update({ gamePath: dir });
  }, [settings?.gamePath, update]);

  if (!settings || !environment) {
    return (
      <div className="screen">
        <Spinner label="Loading settings" />
      </div>
    );
  }

  return (
    <div className="screen">
      <div className="screen-head">
        <div>
          <h1>Settings</h1>
          <p className="screen-sub">Index sources, engine target, appearance and where this app keeps its data.</p>
        </div>
      </div>

      <Card title="Index sources" subtitle="The launcher accepts a Pages root, a feed URL, a GitHub repo page, or owner/repo.">
        {sourceError ? <Banner tone="error">{sourceError}</Banner> : null}
        {sources.length === 0 ? (
          <EmptyState title="No sources" body="Add at least one index source to browse mods." />
        ) : (
          <ul className="source-list">
            {sources.map((source) => (
              <li key={source.id} className="source-row">
                <div className="source-text">
                  <div className="source-title">
                    <strong>{source.label}</strong>
                    {source.builtin ? <Chip tone="note">built in</Chip> : null}
                    {!source.enabled ? <Chip tone="warn">disabled</Chip> : null}
                  </div>
                  <code className="source-feed">{source.feed}</code>
                  {source.fallback ? <code className="source-feed muted">fallback {source.fallback}</code> : null}
                </div>
                <Button
                  small
                  variant="danger"
                  title={
                    source.builtin
                      ? "Removes the shipped index. Add it back with bryanthaboi/gen1recomp-mod-index."
                      : undefined
                  }
                  onClick={() => void removeSource(source.id)}
                >
                  Remove
                </Button>
              </li>
            ))}
          </ul>
        )}
        <Field label="Add a source" htmlFor="settings-source">
          <div className="path-row">
            <TextInput
              id="settings-source"
              mono
              value={sourceUrl}
              onChange={setSourceUrl}
              onEnter={() => void addSource()}
              placeholder="owner/repo or https://example.github.io/index/"
            />
            <Button onClick={() => void addSource()}>Add</Button>
          </div>
        </Field>
      </Card>

      <div className="grid-2">
        <Card title="Engine">
          <Field
            label="Engine version"
            htmlFor="settings-engine"
            hint="Used to judge whether a mod's declared range is satisfied, and to scaffold a new cart's engine range."
          >
            <div className="path-row">
              <TextInput id="settings-engine" mono value={engineVersion} onChange={setEngineVersion} />
              <Button
                disabled={engineVersion.trim() === settings.engineVersion}
                onClick={() => update({ engineVersion: engineVersion.trim() })}
              >
                Apply
              </Button>
              <Button
                disabled={checkingEngine}
                onClick={async () => {
                  setCheckingEngine(true);
                  const next = await run("Reading the engine's latest release", "Engine version", () =>
                    api.settings.refreshEngineVersion(),
                  );
                  setCheckingEngine(false);
                  if (next) {
                    setEngineVersion(next.engineVersion);
                    await applySettings(next);
                    toast("ok", `The engine's latest release is ${next.engineVersion}.`);
                  }
                }}
              >
                {checkingEngine ? "Checking" : "Use latest"}
              </Button>
            </div>
          </Field>
          <KeyValue label="Mod API">{settings.modApi}</KeyValue>
          <KeyValue label="Feed cache">{settings.cacheTtlHours} hours, refreshed manually with Refresh</KeyValue>
          <Field label="Game save directory" hint="Used to read mod_option_schemas.json from a real install.">
            <div className="path-row">
              <TextInput mono value={settings.gamePath ?? ""} onChange={(value) => update({ gamePath: value || null })} />
              <Button onClick={() => void chooseGamePath()}>Browse</Button>
            </div>
          </Field>
        </Card>

        <Card title="Appearance">
          <Field label="Theme" htmlFor="settings-theme">
            <Select
              id="settings-theme"
              value={settings.theme}
              onChange={(value) => update({ theme: value })}
              options={THEMES}
            />
          </Field>
          <KeyValue label="App version">{environment.appVersion}</KeyValue>
          <KeyValue label="Platform">
            {environment.os} {environment.arch}
          </KeyValue>
        </Card>
      </div>

      <Card
        title="Storage"
        subtitle="Everything this app writes lives in these directories and nowhere else."
        actions={
          <Button small onClick={loadUsage}>
            Refresh sizes
          </Button>
        }
      >
        {usageError ? <Banner tone="error">{usageError}</Banner> : null}
        <div className="path-list">
          <PathRow label="Config" path={environment.paths.config} />
          <PathRow label="Feed cache" path={environment.paths.feeds} bytes={usage?.feeds} />
          <PathRow label="Archive cache" path={environment.paths.archives} bytes={usage?.archives} />
          <PathRow label="Logs" path={environment.paths.logs} bytes={usage?.logs} />
          <PathRow label="Default project folder" path={environment.paths.projects} />
        </div>
        <div className="form-actions">
          <Button onClick={() => void clearCache("feeds")}>Clear feed cache</Button>
          <Button onClick={() => void clearCache("archives")}>Clear archive cache</Button>
          <Button onClick={() => void clearCache("logs")}>Clear logs</Button>
          <Button variant="danger" onClick={() => void clearCache("all")}>
            Clear everything
          </Button>
        </div>
        {usage ? <p className="field-hint">{formatBytes(usage.total)} in total.</p> : null}
        <div className="form-actions">
          <Button onClick={() => void exportData()}>Export my data</Button>
          <Button onClick={() => void api.settings.get().then((next) => dispatch({ type: "settings/loaded", settings: next }))}>
            Reload settings
          </Button>
        </div>
      </Card>
    </div>
  );
}

function PathRow({ label, path, bytes }: { label: string; path: string; bytes?: number }): JSX.Element {
  return (
    <div className="path-entry">
      <span className="path-label">{label}</span>
      <code className="path-value" title={path}>
        {path}
      </code>
      <span className="path-size">{bytes === undefined ? "" : formatBytes(bytes)}</span>
      <Button small onClick={() => void api.env.revealPath(path)} ariaLabel={`Reveal ${label}`}>
        Reveal
      </Button>
    </div>
  );
}
