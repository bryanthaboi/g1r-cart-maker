// The IPC contract. Every type here mirrors a Rust type in src-tauri; changing
// one side without the other breaks the app at runtime, not at compile time.

export type Severity = "error" | "warn";

export interface Finding {
  rule: string;
  severity: Severity;
  message: string;
  path: string | null;
}

export interface Report {
  findings: Finding[];
  notes: string[];
}

export type Base = "red" | "blue" | "yellow" | "gold" | "silver" | "crystal";
export type Seal = "sealed" | "sealed+" | "open";
export type Finish = "sparkle" | "holo" | "sparkle+holo";
export type PinSource = "github" | "gamebanana";

export type OptionValue = string | number | boolean;

export interface ModPin {
  id: string;
  source: PinSource;
  repo?: string;
  version?: string;
  sha256?: string;
  mod?: number;
  file?: number;
  md5?: string;
  enabled?: boolean;
  options?: Record<string, OptionValue>;
}

export interface Cart {
  schema: number;
  id: string;
  title: string;
  version: string;
  author: string;
  repo?: string;
  summary?: string;
  shell: string;
  finish?: Finish;
  label?: string;
  base: Base;
  engine?: string;
  seal?: Seal;
  speeds?: number[];
  mods: ModPin[];
  load_order?: string[];
  [key: string]: unknown;
}

export interface ToolStatus {
  found: boolean;
  version: string | null;
  path: string | null;
}

export interface GhStatus extends ToolStatus {
  authenticated: boolean;
  account: string | null;
  /** Set when GH_TOKEN or GITHUB_TOKEN is in the environment. */
  tokenEnv: string | null;
  protocol: string | null;
}

export interface GitIdentity {
  name: string | null;
  email: string | null;
}

export interface AppPaths {
  config: string;
  cache: string;
  feeds: string;
  archives: string;
  logs: string;
  projects: string;
}

export interface Environment {
  os: "macos" | "windows" | "linux";
  arch: string;
  appVersion: string;
  engineVersion: string;
  paths: AppPaths;
  git: ToolStatus;
  gh: GhStatus;
  identity: GitIdentity;
}

export interface InstallStep {
  label: string;
  command: string | null;
  url: string | null;
}

export interface InstallInstructions {
  tool: "git" | "gh";
  os: string;
  steps: InstallStep[];
}

export interface IndexSource {
  id: string;
  url: string;
  feed: string;
  base: string;
  fallback: string | null;
  label: string;
  enabled: boolean;
  builtin: boolean;
}

export interface Settings {
  engineVersion: string;
  modApi: number;
  indexSources: IndexSource[];
  recentProjects: RecentProject[];
  cacheTtlHours: number;
  theme: "system" | "light" | "dark";
  gamePath: string | null;
}

export interface RecentProject {
  path: string;
  id: string;
  title: string;
  base: Base;
  openedAt: string;
}

export interface LabelInfo {
  path: string | null;
  exists: boolean;
  bytes: number;
  width: number | null;
  height: number | null;
  dataUrl: string | null;
}

export interface ProjectState {
  dir: string;
  cart: Cart;
  label: LabelInfo;
  labelDoc: LabelDoc | null;
  report: Report;
  hasWorkflow: boolean;
  isGitRepo: boolean;
}

export type FitMode = "contain" | "cover" | "crop" | "scale" | "stretch";
export type TextAlign = "left" | "center" | "right";

export interface LayerBase {
  id: string;
  name: string;
  x: number;
  y: number;
  width: number;
  height: number;
  rotation: number;
  hidden: boolean;
  locked: boolean;
  from_template: string | null;
}

export type Layer =
  | (LayerBase & { kind: "image"; source: string; fit: FitMode; opacity?: number | null })
  | (LayerBase & {
      kind: "text";
      text: string;
      font: string;
      size: number;
      colour: string;
      align: TextAlign;
      weight?: string | null;
      letter_spacing?: number | null;
      line_height?: number | null;
      stroke?: string | null;
      stroke_width?: number | null;
    })
  | (LayerBase & {
      kind: "rect";
      fill: string;
      radius?: number | null;
      stroke?: string | null;
      stroke_width?: number | null;
    });

export interface LabelDoc {
  schema: number;
  width: number;
  height: number;
  template: string;
  background: string;
  layers: Layer[];
}

export interface LabelTemplate {
  id: string;
  name: string;
  base: Base | null;
  width: number;
  height: number;
  dataUrl: string;
}

export interface ExportCheck {
  ok: boolean;
  bytes: number;
  width: number | null;
  height: number | null;
  problems: string[];
  warnings: string[];
}

export interface ReleaseAsset {
  name: string;
  size: number;
  url: string;
}

export interface Release {
  tag: string;
  name: string | null;
  publishedAt: string | null;
  prerelease: boolean;
  assets: ReleaseAsset[];
}

export interface GameBananaFile {
  id: number;
  file: string;
  size: number;
  md5: string;
  description: string | null;
  downloads: number | null;
}

/** A resolved pin, or the choice the user still has to make. */
export type Resolution =
  | { kind: "pin"; pin: ModPin; note: string }
  | { kind: "chooseFile"; modId: number; files: GameBananaFile[] }
  | { kind: "chooseRelease"; slug: string; releases: Release[] };

export interface CompatIssue {
  level: string;
  text: string;
}

export interface IndexModEntry {
  id: string;
  title: string;
  author: string | null;
  version: string | null;
  summary: string;
  categories: string[];
  tags: string[];
  games: string[];
  repo: string | null;
  github: string | null;
  api: number | null;
  game_version: string | null;
  profile: string | null;
  affects_link: boolean;
  experimental: boolean;
  permissions: string[];
  thumbnail: string | null;
  description_url: string | null;
  downloads: { total: number | null; recent: number | null } | null;
  first_release: string | null;
  last_release: string | null;
  update_check: string;
  latest: { version: string | null; tag: string | null; zip: { url: string; name: string | null } | null } | null;
}

export interface IndexCartEntry {
  id: string;
  title: string;
  author: string;
  version: string;
  base: string;
  seal: string;
  summary: string;
  repo: string;
  tags: string[];
  thumbnail: string | null;
  mods: ModPin[];
}

export interface IndexFeed {
  sourceId: string;
  fetchedAt: string;
  stale: boolean;
  fromFallback: boolean;
  mods: IndexModEntry[];
  carts: IndexCartEntry[];
  categories: string[];
  baseGames: string[];
}

export type OptionRow =
  | { key: string; label: string; type: "toggle"; default: boolean; visible_if?: VisibleIf | null }
  | {
      key: string;
      label: string;
      type: "choice";
      default: OptionValue;
      choices: [string, OptionValue][];
      visible_if?: VisibleIf | null;
    }
  | {
      key: string;
      label: string;
      type: "number";
      default: number;
      min?: number | null;
      max?: number | null;
      step?: number | null;
      visible_if?: VisibleIf | null;
    }
  | {
      key: string;
      label: string;
      type: "text";
      default: string;
      maxLen?: number | null;
      visible_if?: VisibleIf | null;
    };

export interface VisibleIf {
  key: string;
  equals?: OptionValue | null;
  not_equals?: OptionValue | null;
}

export interface OptionDiscovery {
  rows: OptionRow[];
  source: "archive" | "probe" | "install" | "none";
  error: string | null;
}

/// What `index_readiness` returns. `unknown` names the checks that could not be
/// determined, which is not the same as a check that failed.
export interface ReadinessReport {
  items: ReadinessItem[];
  ready: boolean;
  unknown: string[];
}

export interface ReadinessItem {
  id: string;
  label: string;
  ok: boolean;
  blocking: boolean;
  detail: string;
  fix: string | null;
  /// Which action to offer. The label alone cannot be wired to a button.
  fixId: string | null;
}

/// index-entry.json: what the index shows that cart.json has no key for.
export interface IndexEntry {
  thumbnail: string | null;
  description_url: string | null;
  license: string | null;
  tags: string[];
  automatic_version_check: boolean | null;
  fixed_release_tag: string | null;
}

export type PublishStepState = "pending" | "running" | "done" | "failed" | "skipped";

export interface PublishStep {
  id: string;
  label: string;
  state: PublishStepState;
  detail: string;
  log: string;
}

export interface PublishProgress {
  runId: string;
  steps: PublishStep[];
  done: boolean;
  failed: boolean;
  error: string | null;
  repoUrl: string | null;
  releaseUrl: string | null;
  assetName: string | null;
}

export interface PublishRequest {
  dir: string;
  owner: string | null;
  name: string;
  description: string;
  isPrivate: boolean;
  tag: string;
}

export type SubmissionKind = "issue" | "pull_request" | "manual";

export interface SubmissionField {
  id: string;
  label: string;
  value: string;
  multiline: boolean;
  required: boolean;
}

export interface SubmissionPlan {
  kind: SubmissionKind;
  repo: string;
  url: string;
  title: string;
  fields: SubmissionField[];
  body: string;
  guidance: string;
}

export interface CacheUsage {
  feeds: number;
  archives: number;
  logs: number;
  total: number;
}
