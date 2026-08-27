import { useCallback, useMemo, useState } from "react";
import { FieldFindings } from "../components/Findings";
import { Banner, Button, Card, Chip, EmptyState, KeyValue } from "../components/ui";
import { api } from "../lib/backend";
import { LIMITS } from "../lib/constants";
import { shortSha } from "../lib/format";
import { moveItem, normalizeLoadOrder, sameOrder, shiftById } from "../lib/loadOrder";
import { usePointerReorder } from "../lib/pointerReorder";
import type { ModPin, OptionValue } from "../lib/types";
import { checkLoadOrder } from "../lib/validate";
import { useStore } from "../state/store";
import { AddModDialog } from "./AddModDialog";
import { OptionsEditor } from "./OptionsEditor";

export function ModsScreen(): JSX.Element {
  const { state, adopt, run, toast } = useStore();
  const project = state.project;
  const cart = project?.cart ?? null;
  const [adding, setAdding] = useState(false);
  const [editingOptions, setEditingOptions] = useState<ModPin | null>(null);

  const pins = useMemo(() => cart?.mods ?? [], [cart]);
  const order = useMemo(() => normalizeLoadOrder(cart?.load_order, pins.map((pin) => pin.id)), [cart?.load_order, pins]);
  const ordered = useMemo(
    () => order.map((id) => pins.find((pin) => pin.id === id)).filter((pin): pin is ModPin => pin !== undefined),
    [order, pins],
  );
  const orderFindings = useMemo(
    () => checkLoadOrder(pins.map((pin) => pin.id), cart?.load_order),
    [cart?.load_order, pins],
  );

  const commitOrder = useCallback(
    async (next: string[]) => {
      if (!project || sameOrder(next, order)) return;
      const updated = await run("Writing load_order", "Reorder mods", () => api.pins.reorder(project.dir, next));
      if (updated) adopt(updated);
    },
    [adopt, order, project, run],
  );

  const onAdd = useCallback(
    async (pin: ModPin) => {
      if (!project) return;
      const updated = await api.pins.add(project.dir, pin);
      adopt(updated);
      toast("ok", `${pin.id} pinned.`);
    },
    [adopt, project, toast],
  );

  const onRemove = useCallback(
    async (id: string) => {
      if (!project) return;
      const updated = await run(`Removing ${id}`, "Remove mod", () => api.pins.remove(project.dir, id));
      if (updated) {
        adopt(updated);
        toast("ok", `${id} removed.`);
      }
    },
    [adopt, project, run, toast],
  );

  const onToggle = useCallback(
    async (id: string, enabled: boolean) => {
      if (!project) return;
      const updated = await run(`${enabled ? "Enabling" : "Disabling"} ${id}`, "Toggle mod", () =>
        api.pins.setEnabled(project.dir, id, enabled),
      );
      if (updated) adopt(updated);
    },
    [adopt, project, run],
  );

  const onSaveOptions = useCallback(
    async (id: string, options: Record<string, OptionValue>) => {
      if (!project) return;
      const updated = await run(`Saving options for ${id}`, "Mod options", () =>
        api.pins.setOptions(project.dir, id, options),
      );
      if (updated) {
        adopt(updated);
        setEditingOptions(null);
        toast("ok", `${Object.keys(options).length} option${Object.keys(options).length === 1 ? "" : "s"} frozen into ${id}.`);
      }
    },
    [adopt, project, run, toast],
  );

  const reorder = usePointerReorder(ordered.length, (from, to) => {
    void commitOrder(moveItem(order, from, to));
  });

  if (!project || !cart) {
    return (
      <div className="screen">
        <Banner tone="note">Open or create a cart to pin mods.</Banner>
      </div>
    );
  }

  return (
    <div className="screen">
      <div className="screen-head">
        <div>
          <h1>Mods</h1>
          <p className="screen-sub">
            {pins.length} of {LIMITS.mods} pinned. Load order runs top to bottom; a later mod wins a collision.
          </p>
        </div>
        <Button variant="primary" onClick={() => setAdding(true)} disabled={pins.length >= LIMITS.mods}>
          Add mod
        </Button>
      </div>

      {orderFindings.length > 0 ? (
        <Card title="Load order">
          <FieldFindings findings={orderFindings} />
          <Button small onClick={() => void commitOrder(normalizeLoadOrder(cart.load_order, pins.map((pin) => pin.id)))}>
            Rebuild load_order from the pinned list
          </Button>
        </Card>
      ) : null}

      {ordered.length === 0 ? (
        <Card>
          <EmptyState
            title="No mods pinned"
            body="A cart with no mods is the base game with a new label. Pin a mod from the index, a GitHub release, or GameBanana."
            action={
              <Button variant="primary" onClick={() => setAdding(true)}>
                Add mod
              </Button>
            }
          />
        </Card>
      ) : (
        <ul className="pin-list">
          {ordered.map((pin, index) => (
            <li
              key={pin.id}
              ref={reorder.rowRef(index)}
              className={`pin-row${reorder.dragging === index ? " pin-dragging" : ""}${reorder.over === index && reorder.dragging !== index ? " pin-drop" : ""}`}
            >
              <div
                className="pin-handle"
                title="Drag to reorder"
                aria-label={`Reorder ${pin.id}`}
                {...reorder.handleProps(index)}
              >
                <span className="pin-index">{index + 1}</span>
                <span className="pin-grip">::</span>
              </div>

              <div className="pin-body">
                <div className="pin-title">
                  <strong>{pin.id}</strong>
                  <Chip tone={pin.source === "github" ? "note" : "default"}>{pin.source}</Chip>
                  {pin.enabled === false ? <Chip tone="warn">disabled</Chip> : null}
                  {pin.options && Object.keys(pin.options).length > 0 ? (
                    <Chip>{Object.keys(pin.options).length} option{Object.keys(pin.options).length === 1 ? "" : "s"}</Chip>
                  ) : null}
                </div>
                {pin.source === "github" ? (
                  <div className="pin-meta">
                    <KeyValue label="repo">
                      <code>{pin.repo ?? "-"}</code>
                    </KeyValue>
                    <KeyValue label="version">{pin.version ?? "-"}</KeyValue>
                    <KeyValue label="sha256">
                      <code title={pin.sha256 ?? ""}>{shortSha(pin.sha256, 20)}</code>
                    </KeyValue>
                  </div>
                ) : (
                  <div className="pin-meta">
                    <KeyValue label="mod">{pin.mod ?? "-"}</KeyValue>
                    <KeyValue label="file">{pin.file ?? "-"}</KeyValue>
                    <KeyValue label="md5">
                      <code title={pin.md5 ?? ""}>{shortSha(pin.md5, 20)}</code>
                    </KeyValue>
                  </div>
                )}
              </div>

              <div className="pin-actions">
                <div className="pin-move">
                  <Button
                    small
                    ariaLabel={`Move ${pin.id} up`}
                    disabled={index === 0}
                    onClick={() => void commitOrder(shiftById(order, pin.id, -1))}
                  >
                    Up
                  </Button>
                  <Button
                    small
                    ariaLabel={`Move ${pin.id} down`}
                    disabled={index === ordered.length - 1}
                    onClick={() => void commitOrder(shiftById(order, pin.id, 1))}
                  >
                    Down
                  </Button>
                </div>
                <Button small onClick={() => void onToggle(pin.id, pin.enabled === false)}>
                  {pin.enabled === false ? "Enable" : "Disable"}
                </Button>
                <Button small onClick={() => setEditingOptions(pin)}>
                  Options
                </Button>
                <Button small variant="danger" onClick={() => void onRemove(pin.id)}>
                  Remove
                </Button>
              </div>
            </li>
          ))}
        </ul>
      )}

      {adding ? (
        <AddModDialog
          base={cart.base}
          pinnedIds={pins.map((pin) => pin.id)}
          onClose={() => setAdding(false)}
          onAdd={onAdd}
        />
      ) : null}

      {editingOptions ? (
        <OptionsEditor
          pin={editingOptions}
          onClose={() => setEditingOptions(null)}
          onSave={(options) => void onSaveOptions(editingOptions.id, options)}
        />
      ) : null}
    </div>
  );
}
