// The single switch between the real command layer and the dev fixtures.
// Fixtures serve `vite dev` in a plain browser only; a packaged Tauri window
// always has __TAURI_INTERNALS__ and so always talks to Rust.

import { env, feeds, IpcError, label, pins, projects, publish, settings } from "./ipc";
import {
  fixtureEnv,
  fixtureFeeds,
  fixtureLabel,
  fixturePins,
  fixtureProjects,
  fixturePublish,
  fixtureSettings,
} from "./devFixtures";

export const IS_TAURI = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
export const USING_FIXTURES = import.meta.env.DEV && !IS_TAURI;

export interface Backend {
  env: typeof env;
  settings: typeof settings;
  projects: typeof projects;
  pins: typeof pins;
  feeds: typeof feeds;
  label: typeof label;
  publish: typeof publish;
}

const real: Backend = { env, settings, projects, pins, feeds, label, publish };
const fixtures: Backend = {
  env: fixtureEnv,
  settings: fixtureSettings,
  projects: fixtureProjects,
  pins: fixturePins,
  feeds: fixtureFeeds,
  label: fixtureLabel,
  publish: fixturePublish,
};

export const api: Backend = USING_FIXTURES ? fixtures : real;

/** Every rejection reaching the UI becomes one sentence plus optional detail. */
export function errorMessage(problem: unknown): string {
  if (problem instanceof IpcError) return problem.detail ? `${problem.message} (${problem.detail})` : problem.message;
  if (problem instanceof Error) return problem.message;
  if (typeof problem === "string") return problem;
  return "Something went wrong and the backend gave no message.";
}

/** A short, actionable next step for a failure the user can do something about. */
export function errorSuggestion(problem: unknown): string | null {
  const message = errorMessage(problem).toLowerCase();
  if (message.includes("not authenticated") || message.includes("auth")) {
    return "Run gh auth login in a terminal, then use Re-check.";
  }
  if (message.includes("rate limit")) return "Wait for the GitHub rate limit to reset, or set a GH_TOKEN in your environment.";
  if (message.includes("already exists")) return "Pick a different name, or open the existing directory instead.";
  if (message.includes("no cart.json")) return "Choose the folder that contains cart.json, not its parent.";
  if (message.includes("network") || message.includes("dns") || message.includes("connect") || message.includes("timed out")) {
    return "Check your connection and try again. Offline editing and local export still work.";
  }
  if (message.includes("permission") || message.includes("denied")) return "Choose a folder you can write to.";
  if (message.includes("not found") || message.includes("404")) return "Check the spelling of the owner and repository name.";
  return null;
}
