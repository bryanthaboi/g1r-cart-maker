import { useCallback, useMemo, useState } from "react";
import { Banner, Button, Card, ColourPicker, Field, Select, TextArea, TextInput } from "../components/ui";
import { api } from "../lib/backend";
import { BASES, BASE_LABELS, LIMITS, SEALS, SEAL_HELP } from "../lib/constants";
import { pickDirectory } from "../lib/dialogs";
import type { Base, Seal } from "../lib/types";
import { hasErrors, validateNewCart, type NewCartForm } from "../lib/validate";
import { useStore } from "../state/store";

const DEFAULT_SHELL = "#d33a2c";

function suggestId(title: string): string {
  return title
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, LIMITS.id);
}

export function NewCartScreen(): JSX.Element {
  const { state, go, adopt, run, toast } = useStore();
  const [form, setForm] = useState<NewCartForm>({
    id: "",
    title: "",
    author: state.environment?.identity.name ?? "",
    summary: "",
    base: "red",
    shell: DEFAULT_SHELL,
    seal: "sealed",
    github: "",
    parent: state.environment?.paths.projects ?? "",
  });
  const [idEdited, setIdEdited] = useState(false);
  const [submitted, setSubmitted] = useState(false);

  const errors = useMemo(() => validateNewCart(form), [form]);
  const invalid = hasErrors(errors);

  const set = useCallback(<K extends keyof NewCartForm>(key: K, value: NewCartForm[K]) => {
    setForm((current) => ({ ...current, [key]: value }));
  }, []);

  const show = useCallback(
    (key: keyof NewCartForm): string | undefined => {
      if (!submitted) return undefined;
      return errors[key];
    },
    [errors, submitted],
  );

  const onTitle = useCallback(
    (value: string) => {
      setForm((current) => ({
        ...current,
        title: value,
        id: idEdited ? current.id : suggestId(value),
      }));
    },
    [idEdited],
  );

  const onBrowse = useCallback(async () => {
    const dir = await pickDirectory("Choose where the cart directory will be created", form.parent || undefined);
    if (dir) set("parent", dir);
  }, [form.parent, set]);

  const submit = useCallback(async () => {
    setSubmitted(true);
    if (hasErrors(validateNewCart(form))) return;
    const project = await run("Scaffolding the cart", "New cart", () =>
      api.projects.scaffold({
        parent: form.parent.trim(),
        id: form.id.trim(),
        title: form.title.trim().length > 0 ? form.title.trim() : null,
        author: form.author.trim().length > 0 ? form.author.trim() : null,
        summary: form.summary.trim().length > 0 ? form.summary.trim() : null,
        base: form.base,
        shell: form.shell.trim(),
        seal: form.seal,
        github: form.github.trim().length > 0 ? form.github.trim() : null,
        force: false,
      }),
    );
    if (project) {
      adopt(project, "cart");
      toast("ok", `Created ${project.dir}.`, "cart.json, label.png, README.md, CHANGELOG.md and the release workflow are written.");
    }
  }, [adopt, form, run, toast]);

  return (
    <div className="screen screen-narrow">
      <div className="screen-head">
        <div>
          <h1>New cart</h1>
          <p className="screen-sub">
            This creates a directory holding cart.json, a placeholder label, README.md, CHANGELOG.md and the release
            workflow. Everything here can be changed afterwards.
          </p>
        </div>
        <Button onClick={() => go("home")}>Cancel</Button>
      </div>

      <Card title="Identity">
        <Field
          label="Title"
          htmlFor="new-title"
          error={show("title")}
          hint="Shown on the cartridge and in the launcher."
          counter={`${form.title.length}/${LIMITS.title}`}
        >
          <TextInput
            id="new-title"
            value={form.title}
            onChange={onTitle}
            invalid={Boolean(show("title"))}
            placeholder="Kanto Hard Mode"
            autoFocus
          />
        </Field>
        <Field
          label="Id"
          htmlFor="new-id"
          error={show("id")}
          hint="Letters, numbers, hyphen and underscore. It names the directory, the bundle and the save scope."
          counter={`${form.id.length}/${LIMITS.id}`}
        >
          <TextInput
            id="new-id"
            value={form.id}
            mono
            onChange={(value) => {
              setIdEdited(true);
              set("id", value);
            }}
            invalid={Boolean(show("id"))}
            placeholder="kanto-hard-mode"
          />
        </Field>
        <Field label="Author" htmlFor="new-author" error={show("author")} counter={`${form.author.length}/${LIMITS.author}`}>
          <TextInput id="new-author" value={form.author} onChange={(value) => set("author", value)} />
        </Field>
        <Field
          label="Summary"
          htmlFor="new-summary"
          error={show("summary")}
          hint="One line. The index shows it under the title."
          counter={`${form.summary.length}/${LIMITS.summary}`}
        >
          <TextArea
            id="new-summary"
            value={form.summary}
            rows={2}
            onChange={(value) => set("summary", value)}
            invalid={Boolean(show("summary"))}
          />
        </Field>
      </Card>

      <Card title="Cartridge">
        <Field label="Base game" htmlFor="new-base" error={show("base")}>
          <Select
            id="new-base"
            value={form.base as Base}
            onChange={(value) => set("base", value)}
            options={BASES.map((base) => ({ value: base, label: BASE_LABELS[base] }))}
          />
        </Field>
        <Field
          label="Shell colour"
          htmlFor="new-shell"
          error={show("shell")}
          hint="The plastic behind the label. Written to cart.json as #rrggbb."
        >
          <ColourPicker id="new-shell" value={form.shell} onChange={(value) => set("shell", value)} />
        </Field>
        <Field label="Seal" error={show("seal")}>
          <div className="seal-options" role="radiogroup" aria-label="Seal">
            {SEALS.map((seal) => (
              <button
                key={seal}
                type="button"
                role="radio"
                aria-checked={form.seal === seal}
                className={`seal-option${form.seal === seal ? " seal-selected" : ""}`}
                onClick={() => set("seal", seal as Seal)}
              >
                <span className="seal-name">{seal}</span>
                <span className="seal-help">{SEAL_HELP[seal]}</span>
              </button>
            ))}
          </div>
        </Field>
      </Card>

      <Card title="Destination">
        <Field
          label="GitHub owner/repo"
          htmlFor="new-github"
          error={show("github")}
          hint="Optional. It can be filled in later, or created for you by Prepare GitHub Repo."
        >
          <TextInput
            id="new-github"
            mono
            value={form.github}
            onChange={(value) => set("github", value)}
            invalid={Boolean(show("github"))}
            placeholder="owner/name"
          />
        </Field>
        <Field label="Parent directory" htmlFor="new-parent" error={show("parent")}>
          <div className="path-row">
            <TextInput
              id="new-parent"
              mono
              value={form.parent}
              onChange={(value) => set("parent", value)}
              invalid={Boolean(show("parent"))}
              placeholder="Choose a folder"
            />
            <Button onClick={() => void onBrowse()}>Browse</Button>
          </div>
        </Field>
        {form.parent && form.id ? (
          <p className="field-hint">
            The cart will be created at <code>{`${form.parent.replace(/[\\/]+$/, "")}/${form.id}`}</code>.
          </p>
        ) : null}
      </Card>

      {submitted && invalid ? (
        <Banner tone="error">
          Fix the fields marked above. These are the same rules the backend enforces, so nothing here will be
          rejected twice.
        </Banner>
      ) : null}

      <div className="form-actions">
        <Button onClick={() => go("home")}>Cancel</Button>
        <Button
          variant="primary"
          onClick={() => void submit()}
          disabled={state.busy !== null}
        >
          Create cart
        </Button>
      </div>
    </div>
  );
}
