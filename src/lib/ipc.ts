// Typed bindings for every Rust command. The frontend never calls invoke directly.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  CacheUsage,
  Cart,
  Environment,
  ExportCheck,
  GameBananaFile,
  IndexFeed,
  IndexSource,
  InstallInstructions,
  LabelDoc,
  LabelTemplate,
  ModPin,
  OptionDiscovery,
  ProjectState,
  PublishProgress,
  PublishRequest,
  IndexEntry,
  ReadinessReport,
  Release,
  Report,
  Resolution,
  Settings,
} from "./types";

export class IpcError extends Error {
  readonly detail: string;
  constructor(message: string, detail = "") {
    super(message);
    this.name = "IpcError";
    this.detail = detail;
  }
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (problem) {
    if (typeof problem === "string") throw new IpcError(problem);
    if (problem && typeof problem === "object" && "message" in problem) {
      const shaped = problem as { message: string; detail?: string };
      throw new IpcError(shaped.message, shaped.detail ?? "");
    }
    throw new IpcError(String(problem));
  }
}

export const env = {
  environment: () => call<Environment>("app_environment"),
  recheckTools: () => call<Environment>("recheck_tools"),
  instructions: (tool: "git" | "gh") => call<InstallInstructions>("tool_instructions", { tool }),
  setGitIdentity: (name: string, email: string, dir: string | null) =>
    call<void>("set_git_identity", { name, email, dir }),
  revealPath: (path: string) => call<void>("reveal_path", { path }),
  openUrl: (url: string) => call<void>("open_url", { url }),
};

export const settings = {
  get: () => call<Settings>("get_settings"),
  save: (next: Settings) => call<Settings>("save_settings", { next }),
  cacheUsage: () => call<CacheUsage>("cache_usage"),
  clearCache: (kind: "feeds" | "archives" | "logs" | "all") => call<CacheUsage>("clear_cache", { kind }),
  exportData: (outPath: string) => call<string>("export_app_data", { outPath }),
  refreshEngineVersion: () => call<Settings>("refresh_engine_version"),
};

export interface ScaffoldRequest {
  parent: string;
  id: string;
  title: string | null;
  author: string | null;
  summary: string | null;
  base: string;
  shell: string | null;
  seal: string;
  github: string | null;
  force: boolean;
}

export const projects = {
  scaffold: (request: ScaffoldRequest) => call<ProjectState>("scaffold_project", { request }),
  open: (dir: string) => call<ProjectState>("open_project", { dir }),
  reload: (dir: string) => call<ProjectState>("open_project", { dir }),
  save: (dir: string, cart: Cart) => call<ProjectState>("save_project", { dir, cart }),
  forget: (path: string) => call<Settings>("forget_project", { path }),
  validate: (dir: string) => call<Report>("validate_project", { dir }),
  validateOnline: (dir: string) => call<Report>("validate_online", { dir }),
  bundleName: (dir: string) => call<string>("bundle_name", { dir }),
  exportBundle: (dir: string, outPath: string) =>
    call<{ path: string; bytes: number }>("export_bundle", { dir, outPath }),
  readiness: (dir: string) => call<ReadinessReport>("index_readiness", { dir }),
  readIndexEntry: (dir: string) => call<IndexEntry>("read_index_entry", { dir }),
  writeIndexEntry: (dir: string, entry: IndexEntry) => call<IndexEntry>("write_index_entry", { dir, entry }),
  writeLicense: (dir: string, spdx: string, holder: string) =>
    call<string>("write_license", { dir, spdx, holder }),
  writeWorkflow: (dir: string) => call<void>("write_workflow", { dir }),
};

export const pins = {
  resolve: (spec: string, modId: string | null, fileId: number | null) =>
    call<Resolution>("resolve_spec", { spec, modId, fileId }),
  releases: (slug: string) => call<Release[]>("github_releases", { slug }),
  gamebananaFiles: (modId: number) => call<GameBananaFile[]>("gamebanana_files", { modId }),
  add: (dir: string, pin: ModPin) => call<ProjectState>("add_pin", { dir, pin }),
  remove: (dir: string, id: string) => call<ProjectState>("remove_pin", { dir, id }),
  reorder: (dir: string, order: string[]) => call<ProjectState>("reorder_pins", { dir, order }),
  setOptions: (dir: string, id: string, options: Record<string, string | number | boolean>) =>
    call<ProjectState>("set_pin_options", { dir, id, options }),
  setEnabled: (dir: string, id: string, enabled: boolean) =>
    call<ProjectState>("set_pin_enabled", { dir, id, enabled }),
  options: (pin: ModPin) => call<OptionDiscovery>("mod_options_from_archive", { pin }),
  optionsFromInstall: (saveDir: string) =>
    call<Record<string, OptionDiscovery>>("mod_options_from_install", { saveDir }),
};

export const feeds = {
  sources: () => call<IndexSource[]>("index_sources"),
  addSource: (url: string) => call<IndexSource[]>("add_index_source", { url }),
  removeSource: (id: string) => call<IndexSource[]>("remove_index_source", { id }),
  fetch: (sourceId: string, refresh: boolean) => call<IndexFeed>("fetch_index", { sourceId, refresh }),
  thumbnail: (url: string) => call<string>("fetch_thumbnail", { url }),
};

export const label = {
  templates: () => call<LabelTemplate[]>("label_templates"),
  readDoc: (dir: string) => call<LabelDoc | null>("read_label_doc", { dir }),
  writeDoc: (dir: string, doc: LabelDoc) => call<void>("write_label_doc", { dir, doc }),
  checkExport: (dataUrl: string, labelPath: string) =>
    call<ExportCheck>("check_label_export", { dataUrl, labelPath }),
  writePng: (dir: string, labelPath: string, dataUrl: string) =>
    call<ExportCheck>("write_label_png", { dir, labelPath, dataUrl }),
  placeholder: (shell: string) => call<string>("placeholder_label", { shell }),
  readImage: (path: string) => call<string>("read_image_data_url", { path }),
};

export const publish = {
  start: (request: PublishRequest) => call<string>("publish_start", { request }),
  cancel: (runId: string) => call<void>("publish_cancel", { runId }),
  state: (runId: string) => call<PublishProgress>("publish_state", { runId }),
  onProgress: (handler: (progress: PublishProgress) => void): Promise<UnlistenFn> =>
    listen<PublishProgress>("publish://progress", (event) => handler(event.payload)),
  submissionPlan: (dir: string) => call<import("./types").SubmissionPlan>("index_submission_plan", { dir }),
  // The backend re-derives the plan and applies these edits, so nothing the
  // window sends can become the submission on its own.
  submit: (dir: string, plan: import("./types").SubmissionPlan) =>
    call<{ url: string | null }>("index_submit", {
      dir,
      edits: {
        title: plan.title,
        body: plan.body,
        fields: plan.fields.map((field) => ({ id: field.id, value: field.value })),
      },
    }),
};
