import { useCallback, useEffect, useMemo, useState } from "react";
import { Thumbnail } from "../components/Thumbnail";
import { useListNavigation } from "../components/useListNavigation";
import { Banner, Button, Chip, EmptyState, ErrorState, Select, Spinner, TextInput } from "../components/ui";
import { api, errorMessage, errorSuggestion } from "../lib/backend";
import { compatIssues, matchesBase, worstLevel } from "../lib/compat";
import { BASE_LABELS } from "../lib/constants";
import { formatCount, formatDate, formatRelative } from "../lib/format";
import type { Base, IndexFeed, IndexModEntry, IndexSource } from "../lib/types";
import { useStore } from "../state/store";

type SortKey = "downloads" | "updated" | "title" | "released";

const SORTS: readonly { value: SortKey; label: string }[] = [
  { value: "downloads", label: "Most downloaded" },
  { value: "updated", label: "Recently updated" },
  { value: "released", label: "Newest" },
  { value: "title", label: "Title" },
];

export function IndexBrowser({
  base,
  pinnedIds,
  onPick,
}: {
  base: Base;
  pinnedIds: readonly string[];
  onPick: (entry: IndexModEntry) => void;
}): JSX.Element {
  const { state } = useStore();
  const [sources, setSources] = useState<IndexSource[]>([]);
  const [sourceId, setSourceId] = useState<string>("");
  const [feed, setFeed] = useState<IndexFeed | null>(null);
  const [loading, setLoading] = useState(true);
  const [failure, setFailure] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [category, setCategory] = useState("");
  const [tag, setTag] = useState("");
  const [onlyThisBase, setOnlyThisBase] = useState(true);
  const [sort, setSort] = useState<SortKey>("downloads");
  const [cursor, setCursor] = useState(0);

  useEffect(() => {
    let cancelled = false;
    api.feeds
      .sources()
      .then((list) => {
        if (cancelled) return;
        setSources(list);
        const first = list.find((source) => source.enabled) ?? list[0];
        if (first) setSourceId(first.id);
        else {
          setLoading(false);
          setFailure("No index sources are configured. Add one under Settings.");
        }
      })
      .catch((problem: unknown) => {
        if (cancelled) return;
        setLoading(false);
        setFailure(errorMessage(problem));
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const load = useCallback(
    (refresh: boolean) => {
      if (!sourceId) return;
      setLoading(true);
      setFailure(null);
      api.feeds
        .fetch(sourceId, refresh)
        .then((result) => {
          setFeed(result);
          setCursor(0);
        })
        .catch((problem: unknown) => setFailure(errorMessage(problem)))
        .finally(() => setLoading(false));
    },
    [sourceId],
  );

  useEffect(() => {
    load(false);
  }, [load]);

  const engine = useMemo(
    () => ({
      engineVersion: state.settings?.engineVersion ?? state.environment?.engineVersion ?? "0.0.0",
      modApi: state.settings?.modApi ?? 2,
    }),
    [state.environment?.engineVersion, state.settings?.engineVersion, state.settings?.modApi],
  );

  const tags = useMemo(() => {
    const set = new Set<string>();
    for (const entry of feed?.mods ?? []) for (const value of entry.tags) set.add(value);
    return [...set].sort();
  }, [feed]);

  const rows = useMemo(() => {
    const needle = query.trim().toLowerCase();
    const filtered = (feed?.mods ?? []).filter((entry) => {
      if (onlyThisBase && !matchesBase(entry, base)) return false;
      if (category && !entry.categories.includes(category)) return false;
      if (tag && !entry.tags.includes(tag)) return false;
      if (needle.length === 0) return true;
      const haystack = [entry.id, entry.title, entry.author ?? "", entry.summary, entry.tags.join(" "), entry.categories.join(" ")]
        .join(" ")
        .toLowerCase();
      return haystack.includes(needle);
    });
    const sorted = filtered.slice();
    sorted.sort((a, b) => {
      switch (sort) {
        case "title":
          return a.title.localeCompare(b.title);
        case "updated":
          return (b.last_release ?? "").localeCompare(a.last_release ?? "");
        case "released":
          return (b.first_release ?? "").localeCompare(a.first_release ?? "");
        case "downloads":
        default:
          return (b.downloads?.total ?? 0) - (a.downloads?.total ?? 0);
      }
    });
    return sorted;
  }, [base, category, feed, onlyThisBase, query, sort, tag]);

  const nav = useListNavigation(rows.length, cursor, setCursor, (position) => {
    const entry = rows[position];
    if (entry) onPick(entry);
  });

  const selectedSource = sources.find((source) => source.id === sourceId) ?? null;

  return (
    <div className="index-browser">
      <div className="index-toolbar">
        <TextInput value={query} onChange={setQuery} placeholder="Search mods" />
        <Select
          value={sourceId}
          onChange={setSourceId}
          options={sources.map((source) => ({ value: source.id, label: source.label }))}
        />
        <Select value={sort} onChange={setSort} options={SORTS} />
        <Button onClick={() => load(true)} disabled={loading}>
          Refresh
        </Button>
      </div>

      <div className="index-filters">
        <Chip active={onlyThisBase} onClick={() => setOnlyThisBase(!onlyThisBase)}>
          {BASE_LABELS[base]} only
        </Chip>
        <Select
          value={category}
          onChange={setCategory}
          options={[{ value: "", label: "All categories" }, ...(feed?.categories ?? []).map((value) => ({ value, label: value }))]}
        />
        <Select
          value={tag}
          onChange={setTag}
          options={[{ value: "", label: "All tags" }, ...tags.map((value) => ({ value, label: value }))]}
        />
        {feed ? (
          <span className="index-meta">
            fetched {formatRelative(feed.fetchedAt)}
            {feed.stale ? ", cached" : ""}
            {feed.fromFallback ? ", via the raw fallback" : ""}
          </span>
        ) : null}
      </div>

      {selectedSource && !selectedSource.enabled ? (
        <Banner tone="warn">This source is disabled in Settings. Results are from its cache.</Banner>
      ) : null}

      {loading ? <Spinner label="Loading the index" /> : null}

      {!loading && failure ? (
        <ErrorState
          title="The index could not be read"
          message={failure}
          suggestion={errorSuggestion(failure) ?? "The feed and its raw fallback were both unreachable. Cached entries are used when there are any."}
          onRetry={() => load(true)}
        />
      ) : null}

      {!loading && !failure && rows.length === 0 ? (
        <EmptyState
          title="Nothing matches"
          body={
            query || category || tag
              ? "Clear a filter, or search for something else."
              : "This source lists no mods for that base game."
          }
          action={
            query || category || tag ? (
              <Button
                onClick={() => {
                  setQuery("");
                  setCategory("");
                  setTag("");
                }}
              >
                Clear filters
              </Button>
            ) : undefined
          }
        />
      ) : null}

      {!loading && rows.length > 0 ? (
        <ul className="index-list" onKeyDown={nav.onKeyDown}>
          {rows.map((entry, index) => {
            const issues = compatIssues(entry, engine);
            const worst = worstLevel(issues);
            const pinned = pinnedIds.includes(entry.id);
            return (
              <li key={entry.id} className="index-row">
                <button
                  type="button"
                  data-nav-item
                  className="index-main"
                  {...nav.itemProps(index)}
                  onClick={() => onPick(entry)}
                  disabled={pinned}
                >
                  <Thumbnail url={entry.thumbnail} alt={entry.title} />
                  <div className="index-text">
                    <div className="index-title">
                      <strong>{entry.title}</strong>
                      <code>{entry.id}</code>
                      {entry.version ? <span className="muted">{entry.version}</span> : null}
                      {pinned ? <Chip tone="ok">pinned</Chip> : null}
                      {worst ? <Chip tone={worst}>{issues.length} compatibility note{issues.length === 1 ? "" : "s"}</Chip> : null}
                    </div>
                    <p className="index-summary">{entry.summary}</p>
                    <div className="index-meta-row">
                      <span>{entry.author ?? "unknown author"}</span>
                      <span>{formatCount(entry.downloads?.total)} downloads</span>
                      <span>first {formatDate(entry.first_release)}</span>
                      <span>updated {formatDate(entry.last_release)}</span>
                      <span>{entry.games.map((game) => BASE_LABELS[game as Base] ?? game).join(", ")}</span>
                    </div>
                    {issues.length > 0 ? (
                      <ul className="compat-list">
                        {issues.map((issue, position) => (
                          <li key={position} className={`compat compat-${issue.level}`}>
                            {issue.text}
                          </li>
                        ))}
                      </ul>
                    ) : null}
                  </div>
                </button>
                {entry.description_url ? (
                  <button
                    type="button"
                    className="link index-more"
                    onClick={() => void api.env.openUrl(entry.description_url ?? "")}
                  >
                    Details
                  </button>
                ) : null}
              </li>
            );
          })}
        </ul>
      ) : null}
    </div>
  );
}
