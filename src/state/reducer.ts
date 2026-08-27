// One project state store. The backend's returned ProjectState is the truth;
// `draft` holds edits that have not been written to cart.json yet.

import type { Cart, Environment, ProjectState, Report, Settings } from "../lib/types";

export type Route = "home" | "new" | "cart" | "mods" | "label" | "validate" | "export" | "publish" | "settings";

export type ToastKind = "ok" | "info" | "error";

export interface Toast {
  id: number;
  kind: ToastKind;
  message: string;
  suggestion: string | null;
}

export interface AppError {
  message: string;
  suggestion: string | null;
  context: string;
}

export interface AppState {
  route: Route;
  environment: Environment | null;
  settings: Settings | null;
  project: ProjectState | null;
  draft: Cart | null;
  dirty: boolean;
  busy: string | null;
  error: AppError | null;
  onlineReport: Report | null;
  toasts: Toast[];
  nextToastId: number;
}

export const initialState: AppState = {
  route: "home",
  environment: null,
  settings: null,
  project: null,
  draft: null,
  dirty: false,
  busy: null,
  error: null,
  onlineReport: null,
  toasts: [],
  nextToastId: 1,
};

export type Action =
  | { type: "environment/loaded"; environment: Environment }
  | { type: "settings/loaded"; settings: Settings }
  | { type: "project/loaded"; project: ProjectState; route?: Route }
  | { type: "project/closed" }
  | { type: "draft/patch"; patch: Partial<Cart> }
  | { type: "draft/reset" }
  | { type: "route/set"; route: Route }
  | { type: "busy/set"; busy: string | null }
  | { type: "error/set"; error: AppError }
  | { type: "error/clear" }
  | { type: "online/set"; report: Report | null }
  | { type: "toast/push"; kind: ToastKind; message: string; suggestion?: string | null }
  | { type: "toast/dismiss"; id: number };

/** Keys set to undefined are removed, so clearing an optional field really clears it. */
export function patchCart(cart: Cart, patch: Partial<Cart>): Cart {
  const next: Cart = { ...cart };
  for (const [key, value] of Object.entries(patch)) {
    if (value === undefined) delete next[key];
    else next[key] = value;
  }
  return next;
}

function sameCart(a: Cart | null, b: Cart | null): boolean {
  if (a === null || b === null) return a === b;
  return JSON.stringify(a) === JSON.stringify(b);
}

export function reducer(state: AppState, action: Action): AppState {
  switch (action.type) {
    case "environment/loaded":
      return { ...state, environment: action.environment };
    case "settings/loaded":
      return { ...state, settings: action.settings };
    case "project/loaded":
      return {
        ...state,
        project: action.project,
        draft: action.project.cart,
        dirty: false,
        onlineReport: null,
        error: null,
        route: action.route ?? (state.route === "home" || state.route === "new" ? "cart" : state.route),
      };
    case "project/closed":
      return { ...state, project: null, draft: null, dirty: false, onlineReport: null, route: "home" };
    case "draft/patch": {
      if (!state.draft) return state;
      const draft = patchCart(state.draft, action.patch);
      return { ...state, draft, dirty: !sameCart(draft, state.project?.cart ?? null) };
    }
    case "draft/reset":
      if (!state.project) return state;
      return { ...state, draft: state.project.cart, dirty: false };
    case "route/set":
      return { ...state, route: action.route };
    case "busy/set":
      return { ...state, busy: action.busy };
    case "error/set":
      return { ...state, error: action.error, busy: null };
    case "error/clear":
      return { ...state, error: null };
    case "online/set":
      return { ...state, onlineReport: action.report };
    case "toast/push":
      return {
        ...state,
        nextToastId: state.nextToastId + 1,
        toasts: [
          ...state.toasts,
          {
            id: state.nextToastId,
            kind: action.kind,
            message: action.message,
            suggestion: action.suggestion ?? null,
          },
        ].slice(-4),
      };
    case "toast/dismiss":
      return { ...state, toasts: state.toasts.filter((toast) => toast.id !== action.id) };
  }
}

export function requiresProject(route: Route): boolean {
  return route !== "home" && route !== "new" && route !== "settings";
}
