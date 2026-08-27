// In-memory stand-in for the Rust command layer, used only by `vite dev` in a
// plain browser. See backend.ts for the single switch that selects it.

import type { UnlistenFn } from "@tauri-apps/api/event";
import type { ScaffoldRequest } from "./ipc";
import { normalizeLoadOrder } from "./loadOrder";
import { validateCart } from "./validate";
import type {
  CacheUsage,
  Cart,
  Environment,
  ExportCheck,
  GameBananaFile,
  IndexFeed,
  IndexModEntry,
  IndexSource,
  InstallInstructions,
  LabelDoc,
  LabelTemplate,
  ModPin,
  OptionDiscovery,
  OptionValue,
  ProjectState,
  PublishProgress,
  PublishRequest,
  PublishStep,
  IndexEntry,
  ReadinessReport,
  Release,
  Report,
  Resolution,
  Settings,
  SubmissionPlan,
} from "./types";

const LATENCY = 220;

function delay<T>(value: T, ms = LATENCY): Promise<T> {
  return new Promise((resolve) => setTimeout(() => resolve(value), ms));
}

function clone<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}

const PLACEHOLDER_PNG =
  "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

let environment: Environment = {
  os: "macos",
  arch: "aarch64",
  appVersion: "0.1.0",
  engineVersion: "0.2.26",
  paths: {
    config: "/Users/dev/Library/Application Support/g1r-cart-maker/config",
    cache: "/Users/dev/Library/Caches/g1r-cart-maker",
    feeds: "/Users/dev/Library/Caches/g1r-cart-maker/feeds",
    archives: "/Users/dev/Library/Caches/g1r-cart-maker/archives",
    logs: "/Users/dev/Library/Logs/g1r-cart-maker",
    projects: "/Users/dev/Carts",
  },
  git: { found: true, version: "2.45.2", path: "/usr/bin/git" },
  gh: {
    found: true,
    version: "2.62.0",
    path: "/opt/homebrew/bin/gh",
    authenticated: true,
    account: "example-user",
    tokenEnv: null,
    protocol: "https",
  },
  identity: { name: "Example Author", email: "author@example.invalid" },
};

const BUILTIN_SOURCE: IndexSource = {
  id: "builtin",
  url: "https://bryanthaboi.github.io/gen1recomp-mod-index/",
  feed: "https://bryanthaboi.github.io/gen1recomp-mod-index/data/index.json",
  base: "https://bryanthaboi.github.io/gen1recomp-mod-index/",
  fallback: "https://raw.githubusercontent.com/bryanthaboi/gen1recomp-mod-index/main/site/data/index.json",
  label: "gen1recomp mod index",
  enabled: true,
  builtin: true,
};

let sources: IndexSource[] = [clone(BUILTIN_SOURCE)];

let settingsState: Settings = {
  engineVersion: "0.2.26",
  modApi: 2,
  indexSources: sources,
  recentProjects: [],
  cacheTtlHours: 24,
  theme: "system",
  gamePath: null,
};

function newCart(request: ScaffoldRequest): Cart {
  return {
    schema: 1,
    id: request.id,
    title: request.title ?? request.id,
    version: "0.1.0",
    author: request.author ?? environment.identity.name ?? "",
    repo: request.github ?? undefined,
    summary: request.summary ?? undefined,
    shell: request.shell ?? "#d33a2c",
    finish: undefined,
    label: "label.png",
    base: (request.base as Cart["base"]) ?? "red",
    engine: ">=0.2.26 <1.0.0",
    seal: (request.seal as Cart["seal"]) ?? "sealed",
    speeds: undefined,
    mods: [],
    load_order: [],
  };
}

const projectsById = new Map<string, Cart>();
const workflowByDir = new Set<string>();
const gitByDir = new Set<string>();
const labelDocByDir = new Map<string, LabelDoc>();

function reportFor(cart: Cart): Report {
  const findings = validateCart(cart);
  return {
    findings: findings
      .filter((finding) => finding.severity !== "note")
      .map((finding) => ({
        rule: finding.rule,
        severity: finding.severity === "error" ? ("error" as const) : ("warn" as const),
        message: finding.message,
        path: finding.path,
      })),
    notes: findings.filter((finding) => finding.severity === "note").map((finding) => finding.message),
  };
}

function stateFor(dir: string): ProjectState {
  const cart = projectsById.get(dir);
  if (!cart) throw new Error(`No project at ${dir}. Open one first.`);
  return {
    dir,
    cart: clone(cart),
    label: {
      path: typeof cart.label === "string" ? cart.label : null,
      exists: true,
      bytes: 41_207,
      width: 500,
      height: 441,
      dataUrl: PLACEHOLDER_PNG,
    },
    labelDoc: labelDocByDir.get(dir) ?? null,
    report: reportFor(cart),
    hasWorkflow: workflowByDir.has(dir),
    isGitRepo: gitByDir.has(dir),
  };
}

function remember(dir: string, cart: Cart): void {
  const existing = settingsState.recentProjects.filter((entry) => entry.path !== dir);
  settingsState = {
    ...settingsState,
    recentProjects: [
      { path: dir, id: cart.id, title: cart.title, base: cart.base, openedAt: new Date().toISOString() },
      ...existing,
    ].slice(0, 12),
  };
}

const MOD_ENTRIES: IndexModEntry[] = [
  {
    id: "hard-mode",
    title: "Hard Mode",
    author: "shanemcgovern",
    version: "2.1.0",
    summary: "Trainers use held items, better AI and levelled parties throughout the main line.",
    categories: ["Difficulty"],
    tags: ["balance", "ai", "trainers"],
    games: ["red", "blue"],
    repo: "shanemcgovern/g1r-hard-mode",
    github: "https://github.com/shanemcgovern/g1r-hard-mode",
    api: 2,
    game_version: ">=1.8.0 <2.0.0",
    profile: "content",
    affects_link: false,
    experimental: false,
    permissions: [],
    thumbnail: "https://bryanthaboi.github.io/gen1recomp-mod-index/thumbs/hard-mode.png",
    description_url: "https://bryanthaboi.github.io/gen1recomp-mod-index/mods/hard-mode.html",
    downloads: { total: 18_402, recent: 512 },
    first_release: "2025-02-11T00:00:00Z",
    last_release: "2026-06-03T00:00:00Z",
    update_check: "automatic",
    latest: { version: "2.1.0", tag: "v2.1.0", zip: { url: "https://example.invalid/hard-mode-2.1.0.zip", name: "hard-mode-2.1.0.zip" } },
  },
  {
    id: "sprite-revival",
    title: "Sprite Revival",
    author: "kanto-art",
    version: "1.4.2",
    summary: "Redrawn front and back sprites for all 151, matched to the original palette ramp.",
    categories: ["Graphics"],
    tags: ["sprites", "art"],
    games: ["red", "blue", "yellow"],
    repo: "kanto-art/sprite-revival",
    github: "https://github.com/kanto-art/sprite-revival",
    api: 2,
    game_version: ">=1.6.0 <2.0.0",
    profile: "content",
    affects_link: false,
    experimental: false,
    permissions: [],
    thumbnail: "https://bryanthaboi.github.io/gen1recomp-mod-index/thumbs/sprite-revival.png",
    description_url: null,
    downloads: { total: 44_190, recent: 2_310 },
    first_release: "2024-11-02T00:00:00Z",
    last_release: "2026-07-19T00:00:00Z",
    update_check: "automatic",
    latest: { version: "1.4.2", tag: "v1.4.2", zip: { url: "https://example.invalid/sprite-revival-1.4.2.zip", name: "sprite-revival-1.4.2.zip" } },
  },
  {
    id: "randomiser",
    title: "Encounter Randomiser",
    author: "tessa",
    version: "0.9.0",
    summary: "Seeded randomisation of wild encounters, trainer parties and starters.",
    categories: ["Gameplay"],
    tags: ["random", "replay"],
    games: ["red", "blue", "gold", "silver", "crystal"],
    repo: "tessa/g1r-randomiser",
    github: "https://github.com/tessa/g1r-randomiser",
    api: 2,
    game_version: ">=0.2.26 <1.0.0",
    profile: "overhaul",
    affects_link: true,
    experimental: true,
    permissions: ["steps"],
    thumbnail: "https://example.invalid/broken-thumbnail.png",
    description_url: null,
    downloads: { total: 7_720, recent: 980 },
    first_release: "2026-01-08T00:00:00Z",
    last_release: "2026-08-01T00:00:00Z",
    update_check: "automatic",
    latest: { version: "0.9.0", tag: "v0.9.0", zip: { url: "https://example.invalid/randomiser-0.9.0.zip", name: "randomiser-0.9.0.zip" } },
  },
  {
    id: "jukebox",
    title: "Jukebox",
    author: "moonlit",
    version: "3.0.1",
    summary: "Adds a music player to the start menu with every track in the game.",
    categories: ["Audio"],
    tags: ["music", "ui"],
    games: ["gold", "silver", "crystal"],
    repo: "moonlit/jukebox",
    github: "https://github.com/moonlit/jukebox",
    api: 3,
    game_version: ">=2.1.0 <3.0.0",
    profile: "content",
    affects_link: false,
    experimental: false,
    permissions: ["filesystem"],
    thumbnail: null,
    description_url: null,
    downloads: { total: 2_004, recent: 41 },
    first_release: "2025-05-20T00:00:00Z",
    last_release: "2026-04-14T00:00:00Z",
    update_check: "fixed",
    latest: { version: "3.0.1", tag: "v3.0.1", zip: null },
  },
];

const OPTION_ROWS: Record<string, OptionDiscovery> = {
  "hard-mode": {
    source: "archive",
    error: null,
    rows: [
      { key: "enabled", label: "Enabled", type: "toggle", default: true },
      {
        key: "mode",
        label: "Difficulty",
        type: "choice",
        default: "standard",
        choices: [
          ["Standard", "standard"],
          ["Brutal", "brutal"],
          ["Custom", "custom"],
        ],
      },
      {
        key: "level_bump",
        label: "Level bump",
        type: "number",
        default: 3,
        min: 0,
        max: 15,
        step: 1,
        visible_if: { key: "mode", equals: "custom" },
      },
      {
        key: "banner",
        label: "Battle banner text",
        type: "text",
        default: "",
        maxLen: 24,
        visible_if: { key: "enabled", not_equals: false },
      },
    ],
  },
  "sprite-revival": {
    source: "install",
    error: null,
    rows: [
      { key: "backsprites", label: "Replace back sprites", type: "toggle", default: true },
      { key: "menu_icons", label: "Replace menu icons", type: "toggle", default: false },
    ],
  },
};

const RELEASES: Release[] = [
  {
    tag: "v2.1.0",
    name: "Hard Mode 2.1.0",
    publishedAt: "2026-06-03T10:00:00Z",
    prerelease: false,
    assets: [
      { name: "hard-mode-2.1.0.zip", size: 812_004, url: "https://example.invalid/hard-mode-2.1.0.zip" },
      { name: "sha256sums.txt", size: 122, url: "https://example.invalid/sha256sums.txt" },
    ],
  },
  {
    tag: "v2.0.0",
    name: "Hard Mode 2.0.0",
    publishedAt: "2026-02-19T10:00:00Z",
    prerelease: false,
    assets: [{ name: "hard-mode-2.0.0.zip", size: 790_112, url: "https://example.invalid/hard-mode-2.0.0.zip" }],
  },
  {
    tag: "v2.2.0-rc1",
    name: "Hard Mode 2.2.0 rc1",
    publishedAt: "2026-08-10T10:00:00Z",
    prerelease: true,
    assets: [{ name: "hard-mode-2.2.0-rc1.zip", size: 818_400, url: "https://example.invalid/rc1.zip" }],
  },
];

const GB_FILES: GameBananaFile[] = [
  {
    id: 1_204_551,
    file: "kanto-remix-v3.zip",
    size: 4_210_884,
    md5: "9f2c0b1de4a7c6f0a1b2c3d4e5f60718",
    description: "Main package, all regions",
    downloads: 8_112,
  },
  {
    id: 1_204_552,
    file: "kanto-remix-lite.zip",
    size: 1_004_112,
    md5: "31ab77cd90ef1122334455667788990a",
    description: "Audio only",
    downloads: 1_450,
  },
];

const publishHandlers = new Set<(progress: PublishProgress) => void>();
const publishRuns = new Map<string, PublishProgress>();
const publishTimers = new Map<string, ReturnType<typeof setTimeout>>();

function publishSteps(request: PublishRequest): PublishStep[] {
  const owner = request.owner ?? environment.gh.account ?? "you";
  return [
    { id: "write", label: "Write cart directory", state: "pending", detail: "cart.json, label.png, README.md, CHANGELOG.md", log: "" },
    { id: "workflow", label: "Write release workflow", state: "pending", detail: ".github/workflows/release.yml", log: "" },
    { id: "commit", label: "git init, add, commit", state: "pending", detail: "", log: "" },
    { id: "create", label: `Create ${owner}/${request.name}`, state: "pending", detail: request.isPrivate ? "private" : "public", log: "" },
    { id: "tag", label: `Tag ${request.tag} and push`, state: "pending", detail: "", log: "" },
    { id: "run", label: "Watch the release workflow", state: "pending", detail: "", log: "" },
    { id: "asset", label: "Confirm the .g1rcart asset", state: "pending", detail: "", log: "" },
  ];
}

function emitPublish(progress: PublishProgress): void {
  publishRuns.set(progress.runId, progress);
  for (const handler of publishHandlers) handler(clone(progress));
}

function advancePublish(runId: string, request: PublishRequest, index: number): void {
  const current = publishRuns.get(runId);
  if (!current || current.done) return;
  const steps = current.steps.map((step, position) => {
    if (position < index) return { ...step, state: "done" as const };
    if (position === index) {
      return { ...step, state: "running" as const, log: `${step.log}$ ${step.id}\n` };
    }
    return step;
  });
  const done = index >= steps.length;
  const owner = request.owner ?? environment.gh.account ?? "you";
  emitPublish({
    ...current,
    steps: done ? steps.map((step) => ({ ...step, state: "done" as const })) : steps,
    done,
    failed: false,
    error: null,
    repoUrl: done || index > 3 ? `https://github.com/${owner}/${request.name}` : null,
    releaseUrl: done ? `https://github.com/${owner}/${request.name}/releases/tag/${request.tag}` : null,
    assetName: done ? `${request.name}-${request.tag.replace(/^v/, "")}.g1rcart` : null,
  });
  if (done) return;
  const timer = setTimeout(() => advancePublish(runId, request, index + 1), 900);
  publishTimers.set(runId, timer);
}

export const fixtureEnv = {
  environment: () => delay(clone(environment)),
  recheckTools: () => delay(clone(environment), 500),
  instructions: (tool: "git" | "gh"): Promise<InstallInstructions> =>
    delay({
      tool,
      os: environment.os,
      steps:
        tool === "git"
          ? [
              { label: "Install the command line tools", command: "xcode-select --install", url: null },
              { label: "Or install through Homebrew", command: "brew install git", url: null },
              { label: "No Homebrew?", command: null, url: "https://brew.sh" },
            ]
          : [
              { label: "Install through Homebrew", command: "brew install gh", url: null },
              { label: "Or download the installer", command: null, url: "https://cli.github.com" },
            ],
    }),
  setGitIdentity: (name: string, email: string, _dir: string | null): Promise<void> => {
    environment = { ...environment, identity: { name, email } };
    return delay(undefined);
  },
  revealPath: (_path: string): Promise<void> => delay(undefined, 60),
  openUrl: (url: string): Promise<void> => {
    window.open(url, "_blank", "noopener");
    return delay(undefined, 60);
  },
};

export const fixtureSettings = {
  get: () => delay(clone(settingsState)),
  refreshEngineVersion: (): Promise<Settings> => delay(clone(settingsState)),
  save: (next: Settings) => {
    settingsState = clone(next);
    sources = settingsState.indexSources;
    return delay(clone(settingsState));
  },
  cacheUsage: (): Promise<CacheUsage> =>
    delay({ feeds: 411_223, archives: 18_004_112, logs: 92_004, total: 18_507_339 }),
  clearCache: (kind: "feeds" | "archives" | "logs" | "all"): Promise<CacheUsage> => {
    const full = { feeds: 411_223, archives: 18_004_112, logs: 92_004 };
    const next = {
      feeds: kind === "feeds" || kind === "all" ? 0 : full.feeds,
      archives: kind === "archives" || kind === "all" ? 0 : full.archives,
      logs: kind === "logs" || kind === "all" ? 0 : full.logs,
    };
    return delay({ ...next, total: next.feeds + next.archives + next.logs }, 400);
  },
  exportData: (outPath: string) => delay(outPath, 500),
};

export const fixtureProjects = {
  scaffold: (request: ScaffoldRequest): Promise<ProjectState> => {
    const dir = `${request.parent.replace(/[\\/]+$/, "")}/${request.id}`;
    if (projectsById.has(dir) && !request.force) {
      return Promise.reject(new Error(`${dir} already exists. Choose another id or another folder.`));
    }
    const cart = newCart(request);
    projectsById.set(dir, cart);
    workflowByDir.add(dir);
    remember(dir, cart);
    return delay(stateFor(dir), 500);
  },
  open: (dir: string): Promise<ProjectState> => {
    if (!projectsById.has(dir)) {
      return Promise.reject(new Error(`No cart.json in ${dir}. Point at a cart directory.`));
    }
    const cart = projectsById.get(dir);
    if (cart) remember(dir, cart);
    return delay(stateFor(dir));
  },
  reload: (dir: string): Promise<ProjectState> => fixtureProjects.open(dir),
  save: (dir: string, cart: Cart): Promise<ProjectState> => {
    projectsById.set(dir, clone(cart));
    remember(dir, cart);
    return delay(stateFor(dir), 300);
  },
  forget: (path: string): Promise<Settings> => {
    settingsState = {
      ...settingsState,
      recentProjects: settingsState.recentProjects.filter((entry) => entry.path !== path),
    };
    return delay(clone(settingsState), 80);
  },
  validate: (dir: string): Promise<Report> => delay(stateFor(dir).report, 250),
  validateOnline: (dir: string): Promise<Report> => {
    const base = stateFor(dir).report;
    return delay(
      {
        findings: base.findings,
        notes: [
          ...base.notes,
          "GameBanana apiv11 did not answer in time; those pins were not checked online.",
          "Every GitHub pin matched the published sha256.",
        ],
      },
      1400,
    );
  },
  bundleName: (dir: string): Promise<string> => {
    const cart = projectsById.get(dir);
    if (!cart) return Promise.reject(new Error(`No project at ${dir}.`));
    return delay(`${cart.id}-${cart.version}.g1rcart`, 80);
  },
  exportBundle: (dir: string, outPath: string) => {
    const cart = projectsById.get(dir);
    if (!cart) return Promise.reject(new Error(`No project at ${dir}.`));
    return delay({ path: outPath, bytes: 62_144 }, 700);
  },
  readiness: (dir: string): Promise<ReadinessReport> => {
    const cart = projectsById.get(dir);
    if (!cart) return Promise.reject(new Error(`No project at ${dir}.`));
    const pins = cart.mods ?? [];
    return delay(
      [
        { id: "public", label: "Repository is public", ok: Boolean(cart.repo), blocking: true, detail: cart.repo ? `github.com/${cart.repo}` : "No repo is set yet.", fix: cart.repo ? null : "Set a repo on the Cart tab." , fixId: "edit_cart" },
        { id: "schema", label: "cart.json at the repo root, schema 1", ok: cart.schema === 1, blocking: true, detail: `schema ${String(cart.schema)}`, fix: null , fixId: null },
        { id: "fields", label: "Required fields present", ok: Boolean(cart.id && cart.title && cart.author && cart.version && cart.base && cart.seal && cart.repo), blocking: true, detail: "id, title, author, version, base, seal, repo", fix: "Fill the missing identity fields on the Cart tab." , fixId: "edit_cart" },
        { id: "pins", label: "At least one valid pin", ok: pins.length > 0, blocking: true, detail: `${pins.length} pinned`, fix: pins.length > 0 ? null : "Add a mod on the Mods tab." , fixId: "add_mod" },
        { id: "release", label: `Release tagged v${cart.version}`, ok: false, blocking: true, detail: "No release found for this version yet.", fix: "Run Prepare GitHub Repo." , fixId: "publish_release" },
        { id: "asset", label: `${cart.id}-${cart.version}.g1rcart on the release`, ok: false, blocking: true, detail: "The release workflow attaches it.", fix: null , fixId: "publish_release" },
        { id: "sums", label: "sha256sums.txt on the release", ok: false, blocking: false, detail: "Recommended so players can verify the bundle.", fix: null , fixId: "publish_release" },
        { id: "summary", label: "Summary within 120 characters", ok: (cart.summary ?? "").length > 0 && (cart.summary ?? "").length <= 120, blocking: false, detail: `${(cart.summary ?? "").length} characters`, fix: (cart.summary ?? "").length === 0 ? "Write a one-line summary on the Cart tab." : null , fixId: "edit_cart" },
        { id: "thumbnail", label: "Thumbnail", ok: Boolean(cart.label), blocking: false, detail: "The label doubles as the index thumbnail.", fix: null , fixId: "edit_entry" },
        { id: "license", label: "License", ok: false, blocking: false, detail: "No LICENSE file in the cart directory.", fix: null , fixId: "edit_entry" },
      ].reduce<ReadinessReport>(
        (report, item) => {
          report.items.push(item);
          if (!item.ok && item.blocking) report.ready = false;
          return report;
        },
        { items: [], ready: true, unknown: [] },
      ),
      600,
    );
  },
  readIndexEntry: (_dir: string): Promise<IndexEntry> =>
    delay(
      {
        thumbnail: null,
        description_url: null,
        license: null,
        tags: [],
        automatic_version_check: null,
        fixed_release_tag: null,
      },
      120,
    ),
  writeIndexEntry: (_dir: string, entry: IndexEntry): Promise<IndexEntry> => delay(entry, 160),
  writeLicense: (_dir: string, spdx: string, _holder: string): Promise<string> => delay(spdx, 160),
  writeWorkflow: (dir: string): Promise<void> => {
    workflowByDir.add(dir);
    return delay(undefined, 200);
  },
};

export const fixturePins = {
  resolve: (spec: string, modId: string | null, fileId: number | null): Promise<Resolution> => {
    const trimmed = spec.trim();
    if (/^gamebanana:/i.test(trimmed) || /gamebanana\.com/i.test(trimmed) || /^\d+$/.test(trimmed)) {
      const digits = trimmed.match(/(\d+)\s*$/);
      const id = Number(digits ? digits[1] : 0);
      if (fileId === null) return delay({ kind: "chooseFile", modId: id, files: GB_FILES }, 800);
      const file = GB_FILES.find((entry) => entry.id === fileId) ?? GB_FILES[0];
      if (!file) return Promise.reject(new Error("That GameBanana mod publishes no files."));
      return delay(
        {
          kind: "pin",
          pin: { id: modId ?? "kanto-remix", source: "gamebanana", mod: id, file: file.id, md5: file.md5, enabled: true },
          note: `Resolved through apiv11: mod ${id}, file ${file.file}.`,
        },
        800,
      );
    }
    const at = trimmed.lastIndexOf("@");
    const slug = at > 0 ? trimmed.slice(0, at) : trimmed;
    const version = at > 0 ? trimmed.slice(at + 1) : null;
    const name = slug.split("/")[1] ?? "mod";
    if (!version) return delay({ kind: "chooseRelease", slug, releases: RELEASES }, 800);
    return delay(
      {
        kind: "pin",
        pin: {
          id: modId ?? name,
          source: "github",
          repo: slug,
          version,
          sha256: "4f1c6e0a9b2d3f8071a5c4e6d7b8a90123456789abcdef0123456789abcdef01",
          enabled: true,
        },
        note: `Picked ${name}-${version}.zip and read its digest from sha256sums.txt.`,
      },
      800,
    );
  },
  releases: (_slug: string): Promise<Release[]> => delay(clone(RELEASES), 600),
  gamebananaFiles: (_modId: number): Promise<GameBananaFile[]> => delay(clone(GB_FILES), 600),
  add: (dir: string, pin: ModPin): Promise<ProjectState> => {
    const cart = projectsById.get(dir);
    if (!cart) return Promise.reject(new Error(`No project at ${dir}.`));
    const mods = (cart.mods ?? []).filter((entry) => entry.id !== pin.id).concat([pin]);
    const next: Cart = { ...cart, mods, load_order: normalizeLoadOrder(cart.load_order, mods.map((entry) => entry.id)) };
    projectsById.set(dir, next);
    return delay(stateFor(dir), 300);
  },
  remove: (dir: string, id: string): Promise<ProjectState> => {
    const cart = projectsById.get(dir);
    if (!cart) return Promise.reject(new Error(`No project at ${dir}.`));
    const mods = (cart.mods ?? []).filter((entry) => entry.id !== id);
    projectsById.set(dir, { ...cart, mods, load_order: normalizeLoadOrder(cart.load_order, mods.map((entry) => entry.id)) });
    return delay(stateFor(dir), 200);
  },
  reorder: (dir: string, order: string[]): Promise<ProjectState> => {
    const cart = projectsById.get(dir);
    if (!cart) return Promise.reject(new Error(`No project at ${dir}.`));
    projectsById.set(dir, { ...cart, load_order: normalizeLoadOrder(order, (cart.mods ?? []).map((entry) => entry.id)) });
    return delay(stateFor(dir), 120);
  },
  setOptions: (dir: string, id: string, options: Record<string, OptionValue>): Promise<ProjectState> => {
    const cart = projectsById.get(dir);
    if (!cart) return Promise.reject(new Error(`No project at ${dir}.`));
    const mods = (cart.mods ?? []).map((entry) => (entry.id === id ? { ...entry, options } : entry));
    projectsById.set(dir, { ...cart, mods });
    return delay(stateFor(dir), 200);
  },
  setEnabled: (dir: string, id: string, enabled: boolean): Promise<ProjectState> => {
    const cart = projectsById.get(dir);
    if (!cart) return Promise.reject(new Error(`No project at ${dir}.`));
    const mods = (cart.mods ?? []).map((entry) => (entry.id === id ? { ...entry, enabled } : entry));
    projectsById.set(dir, { ...cart, mods });
    return delay(stateFor(dir), 120);
  },
  options: (pin: ModPin): Promise<OptionDiscovery> => {
    const known = OPTION_ROWS[pin.id];
    if (known) return delay(clone(known), 500);
    return delay(
      { rows: [], source: "none", error: `No options_schema in ${pin.id}'s manifest, and the archive had no readable schema.` },
      500,
    );
  },
  optionsFromInstall: (_saveDir: string): Promise<Record<string, OptionDiscovery>> => delay(clone(OPTION_ROWS), 600),
};

export const fixtureFeeds = {
  sources: () => delay(clone(sources)),
  addSource: (url: string): Promise<IndexSource[]> => {
    const id = `custom-${sources.length}`;
    const base = url.endsWith("/") ? url : `${url}/`;
    sources = [...sources, { id, url, feed: `${base}data/index.json`, base, fallback: null, label: url, enabled: true, builtin: false }];
    settingsState = { ...settingsState, indexSources: sources };
    return delay(clone(sources), 400);
  },
  removeSource: (id: string): Promise<IndexSource[]> => {
    sources = sources.filter((source) => source.id !== id);
    settingsState = { ...settingsState, indexSources: sources };
    return delay(clone(sources), 150);
  },
  fetch: (sourceId: string, refresh: boolean): Promise<IndexFeed> =>
    delay(
      {
        sourceId,
        fetchedAt: new Date(Date.now() - (refresh ? 0 : 3_600_000)).toISOString(),
        stale: !refresh,
        fromFallback: false,
        mods: clone(MOD_ENTRIES),
        carts: [],
        categories: ["Difficulty", "Graphics", "Gameplay", "Audio"],
        baseGames: ["red", "blue", "yellow", "gold", "silver", "crystal"],
      },
      refresh ? 1200 : 400,
    ),
  thumbnail: (url: string): Promise<string> => {
    if (url.includes("broken")) return Promise.reject(new Error("404 fetching the thumbnail."));
    return delay(PLACEHOLDER_PNG, 300);
  },
};

export const fixtureLabel = {
  templates: (): Promise<LabelTemplate[]> =>
    delay(
      (["red", "blue", "yellow", "gold", "silver", "crystal"] as const).map((base) => ({
        id: base,
        name: base.charAt(0).toUpperCase() + base.slice(1),
        base,
        width: 500,
        height: 441,
        dataUrl: PLACEHOLDER_PNG,
      })),
      300,
    ),
  readDoc: (dir: string): Promise<LabelDoc | null> => delay(labelDocByDir.get(dir) ?? null, 150),
  writeDoc: (dir: string, doc: LabelDoc): Promise<void> => {
    labelDocByDir.set(dir, clone(doc));
    return delay(undefined, 150);
  },
  checkExport: (_dataUrl: string, labelPath: string): Promise<ExportCheck> =>
    delay({ ok: labelPath.length <= 128, bytes: 180_224, width: 500, height: 441, problems: [], warnings: [] }, 200),
  writePng: (_dir: string, labelPath: string, _dataUrl: string): Promise<ExportCheck> =>
    delay({ ok: labelPath.length <= 128, bytes: 180_224, width: 500, height: 441, problems: [], warnings: [] }, 300),
  placeholder: (_shell: string): Promise<string> => delay(PLACEHOLDER_PNG, 120),
  readImage: (_path: string): Promise<string> => delay(PLACEHOLDER_PNG, 120),
};

export const fixturePublish = {
  start: (request: PublishRequest): Promise<string> => {
    const runId = `run-${Date.now()}`;
    emitPublish({
      runId,
      steps: publishSteps(request),
      done: false,
      failed: false,
      error: null,
      repoUrl: null,
      releaseUrl: null,
      assetName: null,
    });
    const timer = setTimeout(() => advancePublish(runId, request, 0), 400);
    publishTimers.set(runId, timer);
    return delay(runId, 100);
  },
  cancel: (runId: string): Promise<void> => {
    const timer = publishTimers.get(runId);
    if (timer !== undefined) clearTimeout(timer);
    publishTimers.delete(runId);
    const current = publishRuns.get(runId);
    if (current) {
      emitPublish({
        ...current,
        steps: current.steps.map((step) => (step.state === "running" || step.state === "pending" ? { ...step, state: "skipped" as const } : step)),
        done: true,
        failed: true,
        error: "Cancelled.",
      });
    }
    return delay(undefined, 80);
  },
  state: (runId: string): Promise<PublishProgress> => {
    const current = publishRuns.get(runId);
    if (!current) return Promise.reject(new Error(`No publish run ${runId}.`));
    return delay(clone(current), 80);
  },
  onProgress: (handler: (progress: PublishProgress) => void): Promise<UnlistenFn> => {
    publishHandlers.add(handler);
    return Promise.resolve(() => {
      publishHandlers.delete(handler);
    });
  },
  submissionPlan: (dir: string): Promise<SubmissionPlan> => {
    const cart = projectsById.get(dir);
    if (!cart) return Promise.reject(new Error(`No project at ${dir}.`));
    return delay(
      {
        kind: "issue",
        repo: "bryanthaboi/gen1recomp-mod-index",
        url: "https://github.com/bryanthaboi/gen1recomp-mod-index/issues/new?template=add-cart.yml",
        title: `Add cart: ${cart.title}`,
        fields: [
          { id: "repo", label: "Cart repository", value: cart.repo ?? "", multiline: false, required: true },
          { id: "id", label: "Cart id", value: cart.id, multiline: false, required: true },
          { id: "base", label: "Base game", value: cart.base, multiline: false, required: true },
          { id: "summary", label: "Summary", value: cart.summary ?? "", multiline: true, required: false },
        ],
        body: `Submitting ${cart.title} (${cart.id}) for listing.`,
        guidance: "Read from the index repo's own issue template at submit time; the fields follow whatever it asks for.",
      },
      700,
    );
  },
  submit: (_dir: string, plan: SubmissionPlan) =>
    delay({ url: `https://github.com/${plan.repo}/issues/412` }, 900),
};
