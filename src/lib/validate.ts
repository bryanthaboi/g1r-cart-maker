// Client-side mirror of the backend's cart rules. It exists to explain a
// rejection in plain language before a save; the backend Report is the truth.

import {
  BASES,
  FINISHES,
  ID_PATTERN,
  LIMITS,
  REPO_PATTERN,
  RULES,
  SEALS,
  SHELL_PATTERN,
  SPEED_LADDER,
  VERSION_PATTERN,
} from "./constants";
import type { Cart, ModPin } from "./types";

export type UiSeverity = "error" | "warn" | "note";

export interface UiFinding {
  rule: string;
  severity: UiSeverity;
  message: string;
  path: string | null;
}

/** Per-field messages for a form, keyed by the field name the form uses. */
export type FieldErrors = Partial<Record<string, string>>;

export function validateId(value: string): string | null {
  const trimmed = value.trim();
  if (trimmed.length === 0) return "An id is required. Use letters, numbers, hyphen or underscore.";
  if (trimmed.length > LIMITS.id) return `An id is at most ${LIMITS.id} characters; this one is ${trimmed.length}.`;
  if (!ID_PATTERN.test(trimmed)) {
    return "An id may only contain letters, numbers, hyphen and underscore. No spaces, dots or slashes.";
  }
  return null;
}

export function validateTitle(value: string): string | null {
  const trimmed = value.trim();
  if (trimmed.length === 0) return "A title is required.";
  if (trimmed.length > LIMITS.title) {
    return `A title is at most ${LIMITS.title} characters; this one is ${trimmed.length}.`;
  }
  return null;
}

export function validateAuthor(value: string): string | null {
  if (value.length > LIMITS.author) {
    return `An author is at most ${LIMITS.author} characters; this one is ${value.length}.`;
  }
  return null;
}

export function validateSummary(value: string): string | null {
  if (value.length > LIMITS.summary) {
    return `A summary is at most ${LIMITS.summary} characters; this one is ${value.length}.`;
  }
  return null;
}

export function validateShell(value: string): string | null {
  if (value.trim().length === 0) return "A shell colour is required.";
  if (!SHELL_PATTERN.test(value.trim())) return "A shell colour must be a hex value like #d33a2c.";
  return null;
}

export function validateRepo(value: string): string | null {
  if (value.trim().length === 0) return null;
  if (!REPO_PATTERN.test(value.trim())) return "A repo is written owner/name, for example bryanthaboi/my-cart.";
  return null;
}

export function validateVersion(value: string): string | null {
  if (value.trim().length === 0) return "A version is required.";
  if (!VERSION_PATTERN.test(value.trim())) return "A version is three numbers, for example 1.0.0.";
  return null;
}

export function validateLabelPath(value: string): string | null {
  if (value.trim().length === 0) return null;
  if (value.length > LIMITS.labelPath) {
    return `A label path is at most ${LIMITS.labelPath} characters; this one is ${value.length}.`;
  }
  return null;
}

export function validateEngineRange(value: string): string | null {
  if (value.trim().length === 0) return null;
  // The comparators cartkit's range_problem accepts, and nothing else.
  const shape = /^(>=|<=|==|>|<|=|\^)?v?\d+(\.\d+){0,2}(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$/;
  for (const alternative of value.split("||")) {
    const parts = alternative.trim().split(/\s+/).filter((part) => part.length > 0);
    if (parts.length === 0) return "There is an empty alternative around \u007c\u007c.";
    for (const part of parts) {
      if (!shape.test(part)) {
        return `"${part}" is not a version range. Use something like ">=1.4.0 <2.0.0".`;
      }
    }
  }
  return null;
}

export interface NewCartForm {
  id: string;
  title: string;
  author: string;
  summary: string;
  base: string;
  shell: string;
  seal: string;
  github: string;
  parent: string;
}

export function validateNewCart(form: NewCartForm): FieldErrors {
  const errors: FieldErrors = {};
  const id = validateId(form.id);
  if (id) errors.id = id;
  const title = validateTitle(form.title);
  if (title) errors.title = title;
  const author = validateAuthor(form.author);
  if (author) errors.author = author;
  const summary = validateSummary(form.summary);
  if (summary) errors.summary = summary;
  const shell = validateShell(form.shell);
  if (shell) errors.shell = shell;
  const github = validateRepo(form.github);
  if (github) errors.github = github;
  if (!BASES.includes(form.base as (typeof BASES)[number])) errors.base = "Pick a base game.";
  if (!SEALS.includes(form.seal as (typeof SEALS)[number])) errors.seal = "Pick a seal.";
  if (form.parent.trim().length === 0) errors.parent = "Choose the folder the cart directory will be created in.";
  return errors;
}

export function hasErrors(errors: FieldErrors): boolean {
  return Object.values(errors).some((value) => typeof value === "string" && value.length > 0);
}

function pinKey(pin: ModPin): string {
  return pin.id;
}

/** Live findings for the editor, in the same shape the backend report uses. */
export function validateCart(cart: Cart): UiFinding[] {
  const out: UiFinding[] = [];
  const push = (rule: string, severity: UiSeverity, message: string, path: string | null) =>
    out.push({ rule, severity, message, path });

  const id = validateId(cart.id ?? "");
  if (id) push(RULES.identity, "error", id, "id");
  const title = validateTitle(cart.title ?? "");
  if (title) push(RULES.identity, "error", title, "title");
  const version = validateVersion(cart.version ?? "");
  if (version) push(RULES.identity, "error", version, "version");
  if (!cart.author || cart.author.trim().length === 0) {
    push(RULES.identity, "warn", "No author is set. The index lists an author for every cart.", "author");
  } else {
    const author = validateAuthor(cart.author);
    if (author) push(RULES.limits, "error", author, "author");
  }

  if (!BASES.includes(cart.base)) {
    push(RULES.vocabulary, "error", `"${String(cart.base)}" is not a base game.`, "base");
  }
  if (cart.seal !== undefined && !SEALS.includes(cart.seal)) {
    push(RULES.vocabulary, "error", `"${String(cart.seal)}" is not a seal.`, "seal");
  }
  if (cart.finish !== undefined && !FINISHES.includes(cart.finish)) {
    push(RULES.vocabulary, "error", `"${String(cart.finish)}" is not a finish.`, "finish");
  }

  const shell = validateShell(cart.shell ?? "");
  if (shell) push(RULES.appearance, "error", shell, "shell");
  const labelPath = validateLabelPath(cart.label ?? "");
  if (labelPath) push(RULES.label, "error", labelPath, "label");
  if (!cart.label) {
    push(RULES.label, "note", "No label art is set. The cart will show the generated placeholder.", "label");
  }

  const summary = validateSummary(cart.summary ?? "");
  if (summary) push(RULES.limits, "error", summary, "summary");
  if (!cart.summary || cart.summary.trim().length === 0) {
    push(RULES.limits, "note", "A summary is shown on the index listing. Carts without one read as unfinished.", "summary");
  }

  const repo = validateRepo(cart.repo ?? "");
  if (repo) push(RULES.references, "error", repo, "repo");
  if (!cart.repo) {
    push(RULES.references, "warn", "No repo is set. The index cannot list a cart it cannot fetch.", "repo");
  }
  const engine = validateEngineRange(cart.engine ?? "");
  if (engine) push(RULES.references, "error", engine, "engine");

  if (cart.speeds !== undefined) {
    if (cart.speeds.length === 0) {
      push(RULES.vocabulary, "error", "An empty speed list leaves the player no speed at all. Remove the list to allow every speed.", "speeds");
    }
    for (const speed of cart.speeds) {
      if (!SPEED_LADDER.includes(speed)) {
        push(RULES.vocabulary, "error", `${speed} is not on the speed ladder.`, "speeds");
      }
    }
  }

  const mods = cart.mods ?? [];
  if (mods.length === 0) {
    push(RULES.pinShape, "warn", "No mods are pinned. A cart with no mods is the base game with a new label.", "mods");
  }
  if (mods.length > LIMITS.mods) {
    push(RULES.pinIntegrity, "error", `A cart pins at most ${LIMITS.mods} mods; this one pins ${mods.length}.`, "mods");
  }

  const seen = new Set<string>();
  mods.forEach((pin, index) => {
    const path = `mods[${index}]`;
    const pinId = validateId(pin.id ?? "");
    if (pinId) push(RULES.pinShape, "error", `Pinned mod: ${pinId}`, path);
    if (seen.has(pinKey(pin))) {
      push(RULES.pinIntegrity, "error", `"${pin.id}" is pinned twice.`, path);
    }
    seen.add(pinKey(pin));

    if (pin.source === "github") {
      if (!pin.repo) push(RULES.pinShape, "error", `"${pin.id}" is a GitHub pin with no repo.`, path);
      if (!pin.version) push(RULES.pinShape, "error", `"${pin.id}" is a GitHub pin with no version.`, path);
      if (!pin.sha256) {
        push(RULES.pinIntegrity, "warn", `"${pin.id}" has no sha256. The download cannot be verified.`, path);
      }
    } else if (pin.source === "gamebanana") {
      if (pin.mod === undefined) push(RULES.pinShape, "error", `"${pin.id}" is a GameBanana pin with no mod id.`, path);
      if (pin.file === undefined) push(RULES.pinShape, "error", `"${pin.id}" is a GameBanana pin with no file id.`, path);
      if (!pin.md5) {
        push(RULES.pinIntegrity, "warn", `"${pin.id}" has no md5. The download cannot be verified.`, path);
      }
    } else {
      push(RULES.vocabulary, "error", `"${String(pin.source)}" is not a pin source.`, path);
    }

    const options = pin.options ?? {};
    const keys = Object.keys(options);
    if (keys.length > LIMITS.options) {
      push(RULES.limits, "error", `"${pin.id}" sets ${keys.length} options; the limit is ${LIMITS.options}.`, path);
    }
    for (const key of keys) {
      if (key.length > LIMITS.optionKey) {
        push(RULES.limits, "error", `Option key "${key}" is longer than ${LIMITS.optionKey} characters.`, path);
      }
      const value = options[key];
      if (typeof value === "string" && value.length > LIMITS.optionText) {
        push(RULES.limits, "error", `Option "${key}" is longer than ${LIMITS.optionText} characters.`, path);
      }
    }
  });

  out.push(...checkLoadOrder(mods.map((pin) => pin.id), cart.load_order));
  return out;
}

/** Mirrors cartkit check_load_order: every id present, no unknown id, no duplicate. */
export function checkLoadOrder(modIds: string[], order: string[] | undefined): UiFinding[] {
  if (order === undefined) return [];
  const out: UiFinding[] = [];
  const known = new Set(modIds);
  const seen = new Set<string>();
  for (const id of order) {
    if (!known.has(id)) {
      out.push({
        rule: RULES.loadOrderMembership,
        severity: "error",
        message: `load_order names "${id}", which is not a pinned mod.`,
        path: "load_order",
      });
    }
    if (seen.has(id)) {
      out.push({
        rule: RULES.loadOrderDuplicates,
        severity: "error",
        message: `load_order names "${id}" twice.`,
        path: "load_order",
      });
    }
    seen.add(id);
  }
  for (const id of modIds) {
    if (!seen.has(id)) {
      out.push({
        rule: RULES.loadOrderMembership,
        severity: "error",
        message: `load_order is missing "${id}".`,
        path: "load_order",
      });
    }
  }
  return out;
}
