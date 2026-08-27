// Vocabulary and limits mirrored from gen1recomp CartManifest.lua / cartkit.py.
// The backend is authoritative; these exist so the UI can explain a rejection
// before a round trip.

import type { Base, Finish, Seal } from "./types";

export const BASES: readonly Base[] = ["red", "blue", "yellow", "gold", "silver", "crystal"];
export const SEALS: readonly Seal[] = ["sealed", "sealed+", "open"];
export const FINISHES: readonly Finish[] = ["sparkle", "holo", "sparkle+holo"];
export const SPEED_LADDER: readonly number[] = [1, 2, 3, 4, 10, 20, 30, 50, 75, 100, 200];

export const LIMITS = {
  id: 64,
  title: 48,
  author: 64,
  summary: 120,
  labelPath: 128,
  optionKey: 64,
  optionText: 256,
  mods: 64,
  options: 64,
} as const;

export const ID_PATTERN = /^[A-Za-z0-9_-]{1,64}$/;
export const SHELL_PATTERN = /^#[0-9a-fA-F]{6}$/;
export const REPO_PATTERN = /^[A-Za-z0-9._-]+\/[A-Za-z0-9._-]+$/;
export const VERSION_PATTERN = /^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/;

// cartkit's rule ids, so a preview here reads the same as a backend finding.
// CK001 file, CK002 identity, CK003 label, CK004 pins, CK005 load order;
// CK100/CK101/CK110/CK111 are online only and never raised client side.
export const RULES = {
  file: "CK001",
  identity: "CK002",
  vocabulary: "CK002",
  appearance: "CK002",
  limits: "CK002",
  references: "CK002",
  label: "CK003",
  pinShape: "CK004",
  pinIntegrity: "CK004",
  loadOrder: "CK005",
  loadOrderMembership: "CK005",
  loadOrderDuplicates: "CK005",
} as const;

export const SEAL_HELP: Record<Seal, string> = {
  sealed: "Runs exactly as pinned. The player cannot toggle or add mods.",
  "sealed+": "Same fixed set of mods, but the player may turn any pinned mod on or off.",
  open: "The player may install further mods on top of the pinned set.",
};

export const BASE_LABELS: Record<Base, string> = {
  red: "Red",
  blue: "Blue",
  yellow: "Yellow",
  gold: "Gold",
  silver: "Silver",
  crystal: "Crystal",
};

export const FINISH_HELP: Record<Finish, string> = {
  sparkle: "Glittered shell, as on a first-print cartridge.",
  holo: "Holographic sheen across the label art.",
  "sparkle+holo": "Both effects layered.",
};

export const INSTALL_STEPS: readonly string[] = [
  "Drag the .g1rcart file onto the launcher window.",
  "Or use Import cart on the game's page in the launcher.",
  "Or copy it into the launcher's carts/ folder in your save directory.",
];
