// The index-only fields. They live in index-entry.json beside cart.json,
// because cart.json has a closed key set and an extra key there is a CK001
// warning that would refuse the pack.

import { useEffect, useState } from "react";
import { Dialog } from "../components/Dialog";
import { Banner, Button, Field, TextInput, Toggle } from "../components/ui";
import { api, errorMessage } from "../lib/backend";
import type { IndexEntry } from "../lib/types";

const LIMITS = { tags: 12, tag: 24, url: 512, license: 64 };

/// Which field the dialog should put the caret in, by readiness item id.
export type EntryFocus = "thumbnail" | "description_url" | "license" | "tags" | null;

export function IndexEntryDialog(props: {
  dir: string;
  focus: EntryFocus;
  onClose: () => void;
  onSaved: (entry: IndexEntry) => void;
}): JSX.Element {
  const [entry, setEntry] = useState<IndexEntry | null>(null);
  const [tagText, setTagText] = useState("");
  const [failure, setFailure] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    let cancelled = false;
    api.projects
      .readIndexEntry(props.dir)
      .then((got) => {
        if (cancelled) return;
        setEntry(got);
        setTagText(got.tags.join(", "));
      })
      .catch((problem: unknown) => {
        if (!cancelled) setFailure(errorMessage(problem));
      });
    return () => {
      cancelled = true;
    };
  }, [props.dir]);

  const edit = (patch: Partial<IndexEntry>): void => {
    setEntry((current) => (current ? { ...current, ...patch } : current));
  };

  const save = (): void => {
    if (!entry) return;
    setSaving(true);
    setFailure(null);
    const tags = tagText
      .split(",")
      .map((tag) => tag.trim())
      .filter((tag) => tag.length > 0)
      .slice(0, LIMITS.tags);
    api.projects
      .writeIndexEntry(props.dir, { ...entry, tags })
      .then((saved) => {
        props.onSaved(saved);
        props.onClose();
      })
      .catch((problem: unknown) => setFailure(errorMessage(problem)))
      .finally(() => setSaving(false));
  };

  return (
    <Dialog
      title="Index entry"
      description="What the community index shows beside the cart. Saved next to cart.json in index-entry.json, never inside the bundle."
      onClose={props.onClose}
      footer={
        <>
          <Button onClick={props.onClose}>Cancel</Button>
          <Button variant="primary" onClick={save} disabled={!entry || saving}>
            {saving ? "Saving..." : "Save entry"}
          </Button>
        </>
      }
    >
      {failure ? <Banner tone="error">{failure}</Banner> : null}
      {!entry ? (
        <p className="muted">Reading index-entry.json...</p>
      ) : (
        <>
          <Field
            htmlFor="entry-thumbnail"
            label="Thumbnail URL"
            hint="Shown on the listing. The label art is a good choice; a raw.githubusercontent.com link to it works."
          >
            <TextInput
              id="entry-thumbnail"
              mono
              autoFocus={props.focus === "thumbnail"}
              value={entry.thumbnail ?? ""}
              onChange={(value) => edit({ thumbnail: value })}
              placeholder="https://raw.githubusercontent.com/owner/repo/main/label.png"
            />
          </Field>

          <Field
            htmlFor="entry-description"
            label="Description URL"
            hint="A longer write-up the listing links to. Your README works."
          >
            <TextInput
              id="entry-description"
              mono
              autoFocus={props.focus === "description_url"}
              value={entry.description_url ?? ""}
              onChange={(value) => edit({ description_url: value })}
              placeholder="https://github.com/owner/repo#readme"
            />
          </Field>

          <Field htmlFor="entry-license" label="Licence" hint="An SPDX id, such as MIT, Apache-2.0 or CC0-1.0.">
            <TextInput
              id="entry-license"
              mono
              autoFocus={props.focus === "license"}
              value={entry.license ?? ""}
              onChange={(value) => edit({ license: value })}
              placeholder="MIT"
            />
          </Field>

          <Field
            htmlFor="entry-tags"
            label="Tags"
            hint={`Comma separated, up to ${LIMITS.tags}. Without them the cart is only found by name.`}
          >
            <TextInput
              id="entry-tags"
              autoFocus={props.focus === "tags"}
              value={tagText}
              onChange={setTagText}
              placeholder="hard mode, randomizer, quality of life"
            />
          </Field>

          <Toggle
            checked={entry.automatic_version_check !== false}
            onChange={(checked) => edit({ automatic_version_check: checked })}
            label="Let the index follow new releases"
          />
        </>
      )}
    </Dialog>
  );
}
