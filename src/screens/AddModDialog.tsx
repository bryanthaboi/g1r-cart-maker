import { useCallback, useState } from "react";
import { Dialog } from "../components/Dialog";
import { Banner, Button, Chip, Field, KeyValue, Spinner, TextInput } from "../components/ui";
import { api, errorMessage, errorSuggestion } from "../lib/backend";
import { formatBytes, formatDate, shortSha } from "../lib/format";
import { describeSpec, parseSpec } from "../lib/spec";
import type { Base, GameBananaFile, IndexModEntry, ModPin, Release, Resolution } from "../lib/types";
import { IndexBrowser } from "./IndexBrowser";

type Tab = "index" | "github" | "gamebanana";

const TABS: readonly { id: Tab; label: string; hint: string }[] = [
  { id: "index", label: "Browse the index", hint: "Search the configured mod index." },
  { id: "github", label: "GitHub repo", hint: "owner/repo, owner/repo@1.2.3, or a github.com link." },
  { id: "gamebanana", label: "GameBanana", hint: "A gamebanana.com link, gamebanana:N, or a bare id." },
];

export function AddModDialog({
  base,
  pinnedIds,
  onClose,
  onAdd,
}: {
  base: Base;
  pinnedIds: readonly string[];
  onClose: () => void;
  onAdd: (pin: ModPin) => Promise<void>;
}): JSX.Element {
  const [tab, setTab] = useState<Tab>("index");
  const [text, setText] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [failure, setFailure] = useState<{ message: string; suggestion: string | null } | null>(null);
  const [resolution, setResolution] = useState<Resolution | null>(null);
  const [preferredId, setPreferredId] = useState<string | null>(null);

  const parsed = parseSpec(text);
  const specTab: Tab | null = parsed.kind === "github" ? "github" : parsed.kind === "gamebanana" ? "gamebanana" : null;

  const resolve = useCallback(
    async (spec: string, modId: string | null, fileId: number | null) => {
      setBusy(`Resolving ${spec}`);
      setFailure(null);
      try {
        const result = await api.pins.resolve(spec, modId, fileId);
        setResolution(result);
      } catch (problem) {
        setResolution(null);
        setFailure({ message: errorMessage(problem), suggestion: errorSuggestion(problem) });
      } finally {
        setBusy(null);
      }
    },
    [],
  );

  const onPickIndexEntry = useCallback(
    (entry: IndexModEntry) => {
      setPreferredId(entry.id);
      const slug = entry.repo ?? entry.github ?? "";
      if (slug.length === 0) {
        setFailure({
          message: `${entry.title} publishes no repository in the index, so it cannot be pinned from here.`,
          suggestion: "Paste its download source under the GitHub or GameBanana tab.",
        });
        return;
      }
      const clean = slug.replace(/^https?:\/\/(www\.)?github\.com\//i, "");
      const spec = entry.latest?.version ? `${clean}@${entry.latest.version}` : clean;
      setText(spec);
      void resolve(spec, entry.id, null);
    },
    [resolve],
  );

  const confirm = useCallback(async () => {
    if (!resolution || resolution.kind !== "pin") return;
    setBusy(`Adding ${resolution.pin.id}`);
    try {
      await onAdd(resolution.pin);
      onClose();
    } catch (problem) {
      setFailure({ message: errorMessage(problem), suggestion: errorSuggestion(problem) });
    } finally {
      setBusy(null);
    }
  }, [onAdd, onClose, resolution]);

  return (
    <Dialog
      title="Add a mod"
      description="Every entry path lands in the same pinned entry. Nothing is written until you confirm."
      onClose={onClose}
      wide
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" disabled={!resolution || resolution.kind !== "pin" || busy !== null} onClick={() => void confirm()}>
            Add this pin
          </Button>
        </>
      }
    >
      <div className="tabs" role="tablist" aria-label="How to add a mod">
        {TABS.map((entry) => (
          <button
            key={entry.id}
            type="button"
            role="tab"
            aria-selected={tab === entry.id}
            className={`tab${tab === entry.id ? " tab-active" : ""}`}
            onClick={() => setTab(entry.id)}
          >
            {entry.label}
          </button>
        ))}
      </div>

      {tab === "index" ? (
        <IndexBrowser base={base} pinnedIds={pinnedIds} onPick={onPickIndexEntry} />
      ) : (
        <div className="paste-panel">
          <Field
            label={tab === "github" ? "GitHub repository" : "GameBanana mod"}
            hint={TABS.find((entry) => entry.id === tab)?.hint}
            error={text.trim().length > 0 && parsed.kind === "unknown" ? parsed.reason : null}
          >
            <div className="path-row">
              <TextInput
                value={text}
                onChange={(value) => {
                  setText(value);
                  setResolution(null);
                }}
                mono
                autoFocus
                placeholder={tab === "github" ? "bryanthaboi/example-mod@1.2.3" : "https://gamebanana.com/mods/546899"}
                onEnter={() => parsed.kind !== "unknown" && void resolve(parsed.normalized, preferredId, null)}
              />
              <Button
                variant="primary"
                disabled={parsed.kind === "unknown" || busy !== null}
                onClick={() => parsed.kind !== "unknown" && void resolve(parsed.normalized, preferredId, null)}
              >
                Resolve
              </Button>
            </div>
          </Field>
          {text.trim().length > 0 && parsed.kind !== "unknown" ? (
            <p className="field-hint">
              Read as: {describeSpec(parsed)}
              {specTab && specTab !== tab ? ` (that is a ${specTab === "github" ? "GitHub" : "GameBanana"} spec; the tab does not matter)` : ""}
            </p>
          ) : null}
        </div>
      )}

      {busy ? <Spinner label={busy} /> : null}

      {failure ? (
        <Banner tone="error">
          <strong>Could not resolve.</strong> {failure.message}
          {failure.suggestion ? <span className="banner-hint">{failure.suggestion}</span> : null}
        </Banner>
      ) : null}

      {resolution?.kind === "chooseRelease" ? (
        <ReleasePicker
          slug={resolution.slug}
          releases={resolution.releases}
          onPick={(release) => void resolve(`${resolution.slug}@${release.tag.replace(/^v/, "")}`, preferredId, null)}
        />
      ) : null}

      {resolution?.kind === "chooseFile" ? (
        <FilePicker
          files={resolution.files}
          onPick={(file) => void resolve(`gamebanana:${resolution.modId}`, preferredId, file.id)}
        />
      ) : null}

      {resolution?.kind === "pin" ? <PinPreview pin={resolution.pin} note={resolution.note} /> : null}
    </Dialog>
  );
}

function ReleasePicker({
  slug,
  releases,
  onPick,
}: {
  slug: string;
  releases: Release[];
  onPick: (release: Release) => void;
}): JSX.Element {
  if (releases.length === 0) {
    return <Banner tone="error">{slug} has published no releases. A pin needs a release with a .zip asset.</Banner>;
  }
  return (
    <div className="picker">
      <h4>Pick a release of {slug}</h4>
      <ul className="picker-list">
        {releases.map((release) => (
          <li key={release.tag}>
            <button type="button" className="picker-row" onClick={() => onPick(release)}>
              <span className="picker-main">
                <strong>{release.tag}</strong>
                {release.name ? <span className="muted">{release.name}</span> : null}
                {release.prerelease ? <Chip tone="warn">prerelease</Chip> : null}
              </span>
              <span className="picker-meta">
                {formatDate(release.publishedAt)} &middot; {release.assets.length} asset
                {release.assets.length === 1 ? "" : "s"}
                {release.assets.some((asset) => asset.name === "sha256sums.txt") ? " (with sha256sums.txt)" : ""}
              </span>
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}

function FilePicker({ files, onPick }: { files: GameBananaFile[]; onPick: (file: GameBananaFile) => void }): JSX.Element {
  if (files.length === 0) {
    return <Banner tone="error">That GameBanana mod publishes no downloadable files.</Banner>;
  }
  return (
    <div className="picker">
      <h4>This mod publishes {files.length} file{files.length === 1 ? "" : "s"}</h4>
      <ul className="picker-list">
        {files.map((file) => (
          <li key={file.id}>
            <button type="button" className="picker-row" onClick={() => onPick(file)}>
              <span className="picker-main">
                <strong>{file.file}</strong>
                <span className="muted">{formatBytes(file.size)}</span>
              </span>
              <span className="picker-meta">
                {file.description ?? "no description"} &middot; md5 {shortSha(file.md5)}
                {file.downloads !== null ? ` · ${file.downloads} downloads` : ""}
              </span>
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}

function PinPreview({ pin, note }: { pin: ModPin; note: string }): JSX.Element {
  return (
    <div className="pin-preview">
      <h4>This is what will be added</h4>
      <p className="field-hint">{note}</p>
      <KeyValue label="Mod id">
        <code>{pin.id}</code>
      </KeyValue>
      <KeyValue label="Source">{pin.source}</KeyValue>
      {pin.source === "github" ? (
        <>
          <KeyValue label="Repository">
            <code>{pin.repo ?? "-"}</code>
          </KeyValue>
          <KeyValue label="Version">{pin.version ?? "-"}</KeyValue>
          <KeyValue label="sha256">
            <code title={pin.sha256 ?? ""}>{shortSha(pin.sha256, 24)}</code>
          </KeyValue>
        </>
      ) : (
        <>
          <KeyValue label="GameBanana mod">{pin.mod ?? "-"}</KeyValue>
          <KeyValue label="File">{pin.file ?? "-"}</KeyValue>
          <KeyValue label="md5">
            <code title={pin.md5 ?? ""}>{shortSha(pin.md5, 24)}</code>
          </KeyValue>
        </>
      )}
    </div>
  );
}
