import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useReducer,
  type Dispatch,
  type ReactNode,
} from "react";
import { api, errorMessage, errorSuggestion } from "../lib/backend";
import type { Cart, ProjectState, Settings } from "../lib/types";
import { initialState, reducer, type Action, type AppState, type Route } from "./reducer";

interface StoreValue {
  state: AppState;
  dispatch: Dispatch<Action>;
  go: (route: Route) => void;
  patch: (patch: Partial<Cart>) => void;
  refreshEnvironment: (recheck: boolean) => Promise<void>;
  refreshSettings: () => Promise<void>;
  applySettings: (next: Settings) => Promise<void>;
  openProject: (dir: string) => Promise<void>;
  closeProject: () => void;
  saveDraft: () => Promise<void>;
  reloadProject: () => Promise<void>;
  adopt: (project: ProjectState, route?: Route) => void;
  run: <T>(label: string, context: string, task: () => Promise<T>) => Promise<T | null>;
  toast: (kind: "ok" | "info" | "error", message: string, suggestion?: string | null) => void;
}

const StoreContext = createContext<StoreValue | null>(null);

export function StoreProvider({ children }: { children: ReactNode }): JSX.Element {
  const [state, dispatch] = useReducer(reducer, initialState);

  const toast = useCallback(
    (kind: "ok" | "info" | "error", message: string, suggestion?: string | null) => {
      dispatch({ type: "toast/push", kind, message, suggestion: suggestion ?? null });
    },
    [],
  );

  const run = useCallback(
    async <T,>(label: string, context: string, task: () => Promise<T>): Promise<T | null> => {
      dispatch({ type: "busy/set", busy: label });
      dispatch({ type: "error/clear" });
      try {
        const result = await task();
        dispatch({ type: "busy/set", busy: null });
        return result;
      } catch (problem) {
        dispatch({
          type: "error/set",
          error: { message: errorMessage(problem), suggestion: errorSuggestion(problem), context },
        });
        return null;
      }
    },
    [],
  );

  const refreshEnvironment = useCallback(
    async (recheck: boolean) => {
      const environment = await run(
        recheck ? "Re-checking git and gh" : "Checking your environment",
        "Environment check",
        () => (recheck ? api.env.recheckTools() : api.env.environment()),
      );
      if (environment) dispatch({ type: "environment/loaded", environment });
    },
    [run],
  );

  const refreshSettings = useCallback(async () => {
    const settings = await run("Loading settings", "Settings", () => api.settings.get());
    if (settings) dispatch({ type: "settings/loaded", settings });
  }, [run]);

  const applySettings = useCallback(
    async (next: Settings) => {
      const saved = await run("Saving settings", "Settings", () => api.settings.save(next));
      if (saved) {
        dispatch({ type: "settings/loaded", settings: saved });
        toast("ok", "Settings saved.");
      }
    },
    [run, toast],
  );

  const adopt = useCallback((project: ProjectState, route?: Route) => {
    dispatch({ type: "project/loaded", project, ...(route ? { route } : {}) });
  }, []);

  const openProject = useCallback(
    async (dir: string) => {
      const project = await run("Opening the cart", "Open cart", () => api.projects.open(dir));
      if (project) {
        dispatch({ type: "project/loaded", project, route: "cart" });
        const settings = await api.settings.get().catch(() => null);
        if (settings) dispatch({ type: "settings/loaded", settings });
      }
    },
    [run],
  );

  const reloadProject = useCallback(async () => {
    if (!state.project) return;
    const project = await run("Reloading from disk", "Reload", () => api.projects.reload(state.project?.dir ?? ""));
    if (project) dispatch({ type: "project/loaded", project });
  }, [run, state.project]);

  const closeProject = useCallback(() => {
    dispatch({ type: "project/closed" });
  }, []);

  const saveDraft = useCallback(async () => {
    if (!state.project || !state.draft) return;
    const dir = state.project.dir;
    const draft = state.draft;
    const project = await run("Writing cart.json", "Save cart", () => api.projects.save(dir, draft));
    if (project) {
      dispatch({ type: "project/loaded", project });
      toast("ok", "cart.json saved.");
    }
  }, [run, state.draft, state.project, toast]);

  const go = useCallback((route: Route) => {
    dispatch({ type: "route/set", route });
  }, []);

  const patch = useCallback((next: Partial<Cart>) => {
    dispatch({ type: "draft/patch", patch: next });
  }, []);

  useEffect(() => {
    void refreshEnvironment(false);
    void refreshSettings();
  }, [refreshEnvironment, refreshSettings]);

  useEffect(() => {
    const theme = state.settings?.theme ?? "system";
    const root = document.documentElement;
    if (theme === "system") root.removeAttribute("data-theme");
    else root.setAttribute("data-theme", theme);
  }, [state.settings?.theme]);

  // The window must not close on an unsaved draft without a word.
  useEffect(() => {
    if (!state.dirty) return undefined;
    const guard = (event: BeforeUnloadEvent) => {
      event.preventDefault();
      event.returnValue = "";
    };
    window.addEventListener("beforeunload", guard);
    return () => window.removeEventListener("beforeunload", guard);
  }, [state.dirty]);

  const value = useMemo<StoreValue>(
    () => ({
      state,
      dispatch,
      go,
      patch,
      refreshEnvironment,
      refreshSettings,
      applySettings,
      openProject,
      closeProject,
      saveDraft,
      reloadProject,
      adopt,
      run,
      toast,
    }),
    [
      state,
      go,
      patch,
      refreshEnvironment,
      refreshSettings,
      applySettings,
      openProject,
      closeProject,
      saveDraft,
      reloadProject,
      adopt,
      run,
      toast,
    ],
  );

  return <StoreContext.Provider value={value}>{children}</StoreContext.Provider>;
}

export function useStore(): StoreValue {
  const value = useContext(StoreContext);
  if (!value) throw new Error("useStore was called outside StoreProvider.");
  return value;
}
