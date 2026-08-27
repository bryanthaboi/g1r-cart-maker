import { describe, expect, it } from "vitest";
import type { Cart, ProjectState } from "../lib/types";
import { initialState, patchCart, reducer, requiresProject, type AppState } from "./reducer";

const cart: Cart = {
  schema: 1,
  id: "example",
  title: "Example",
  version: "1.0.0",
  author: "me",
  shell: "#ffffff",
  base: "red",
  seal: "sealed",
  finish: "holo",
  mods: [],
};

const project: ProjectState = {
  dir: "/carts/example",
  cart,
  label: { path: "label.png", exists: true, bytes: 100, width: 500, height: 441, dataUrl: null },
  labelDoc: null,
  report: { findings: [], notes: [] },
  hasWorkflow: true,
  isGitRepo: false,
};

function loaded(): AppState {
  return reducer(initialState, { type: "project/loaded", project });
}

describe("patchCart", () => {
  it("removes a key set to undefined", () => {
    const next = patchCart(cart, { finish: undefined });
    expect("finish" in next).toBe(false);
  });

  it("does not mutate the original", () => {
    patchCart(cart, { title: "Other" });
    expect(cart.title).toBe("Example");
  });
});

describe("project lifecycle", () => {
  it("adopts the returned state as the truth and clears dirty", () => {
    const state = loaded();
    expect(state.project).toBe(project);
    expect(state.draft).toEqual(cart);
    expect(state.dirty).toBe(false);
    expect(state.route).toBe("cart");
  });

  it("keeps the current route when already inside a project", () => {
    const state = reducer({ ...loaded(), route: "mods" }, { type: "project/loaded", project });
    expect(state.route).toBe("mods");
  });

  it("honours an explicit route", () => {
    const state = reducer(initialState, { type: "project/loaded", project, route: "validate" });
    expect(state.route).toBe("validate");
  });

  it("clears everything on close", () => {
    const state = reducer(loaded(), { type: "project/closed" });
    expect(state.project).toBeNull();
    expect(state.draft).toBeNull();
    expect(state.route).toBe("home");
  });
});

describe("draft editing", () => {
  it("marks the draft dirty when it diverges from the saved cart", () => {
    const state = reducer(loaded(), { type: "draft/patch", patch: { title: "Renamed" } });
    expect(state.dirty).toBe(true);
    expect(state.draft?.title).toBe("Renamed");
    expect(state.project?.cart.title).toBe("Example");
  });

  it("goes clean again when the edit is undone by hand", () => {
    const edited = reducer(loaded(), { type: "draft/patch", patch: { title: "Renamed" } });
    const restored = reducer(edited, { type: "draft/patch", patch: { title: "Example" } });
    expect(restored.dirty).toBe(false);
  });

  it("resets the draft to the file on disk", () => {
    const edited = reducer(loaded(), { type: "draft/patch", patch: { title: "Renamed" } });
    const reset = reducer(edited, { type: "draft/reset" });
    expect(reset.draft).toEqual(cart);
    expect(reset.dirty).toBe(false);
  });

  it("ignores an edit when no project is open", () => {
    const state = reducer(initialState, { type: "draft/patch", patch: { title: "x" } });
    expect(state).toBe(initialState);
  });
});

describe("busy, errors and toasts", () => {
  it("clears busy when an error arrives", () => {
    const busy = reducer(initialState, { type: "busy/set", busy: "Working" });
    const failed = reducer(busy, {
      type: "error/set",
      error: { message: "no", suggestion: null, context: "Save" },
    });
    expect(failed.busy).toBeNull();
    expect(failed.error?.context).toBe("Save");
    expect(reducer(failed, { type: "error/clear" }).error).toBeNull();
  });

  it("gives every toast a distinct id and caps the stack", () => {
    let state = initialState;
    for (let index = 0; index < 6; index += 1) {
      state = reducer(state, { type: "toast/push", kind: "ok", message: `m${index}` });
    }
    expect(state.toasts).toHaveLength(4);
    expect(new Set(state.toasts.map((toast) => toast.id)).size).toBe(4);
    const first = state.toasts[0];
    if (!first) throw new Error("expected a toast");
    expect(reducer(state, { type: "toast/dismiss", id: first.id }).toasts).toHaveLength(3);
  });
});

describe("requiresProject", () => {
  it("gates the cart-bound routes only", () => {
    expect(requiresProject("home")).toBe(false);
    expect(requiresProject("new")).toBe(false);
    expect(requiresProject("settings")).toBe(false);
    expect(requiresProject("cart")).toBe(true);
    expect(requiresProject("mods")).toBe(true);
    expect(requiresProject("label")).toBe(true);
    expect(requiresProject("publish")).toBe(true);
  });
});
