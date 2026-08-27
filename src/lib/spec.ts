// Which paste path the user is on. Mirrors cartkit's GITHUB_SPEC and
// GAMEBANANA_SPEC; every pasted string is untrusted.

export type ParsedSpec =
  | { kind: "github"; slug: string; version: string | null; normalized: string }
  | { kind: "gamebanana"; modId: number; normalized: string }
  | { kind: "unknown"; reason: string };

const SLUG_PART = "[A-Za-z0-9._-]+";
const SLUG = new RegExp(`^(${SLUG_PART})/(${SLUG_PART})$`);
const SLUG_AT_VERSION = new RegExp(`^(${SLUG_PART})/(${SLUG_PART})@([0-9A-Za-z.+-]+)$`);

function stripTrailing(value: string): string {
  return value.replace(/[/\s]+$/, "");
}

function cleanSlug(name: string): string {
  return name.replace(/\.git$/i, "");
}

function github(slug: string, version: string | null): ParsedSpec {
  return { kind: "github", slug, version, normalized: version ? `${slug}@${version}` : slug };
}

function gamebanana(modId: number): ParsedSpec {
  if (!Number.isInteger(modId) || modId <= 0) {
    return { kind: "unknown", reason: "A GameBanana id is a positive whole number." };
  }
  return { kind: "gamebanana", modId, normalized: `gamebanana:${modId}` };
}

export function parseSpec(raw: string): ParsedSpec {
  const input = stripTrailing(raw.trim());
  if (input.length === 0) return { kind: "unknown", reason: "Paste a GitHub repo or a GameBanana link." };

  if (/^gamebanana:\d+$/i.test(input)) {
    return gamebanana(Number(input.split(":")[1] ?? ""));
  }
  if (/^\d+$/.test(input)) return gamebanana(Number(input));

  let url: URL | null = null;
  if (/^https?:\/\//i.test(input)) {
    try {
      url = new URL(input);
    } catch {
      return { kind: "unknown", reason: "That is not a URL this app can read." };
    }
  }

  if (url) {
    const host = url.hostname.replace(/^www\./i, "").toLowerCase();
    const segments = url.pathname.split("/").filter((part) => part.length > 0);
    if (host === "github.com") {
      const owner = segments[0];
      const name = segments[1];
      if (!owner || !name) return { kind: "unknown", reason: "A GitHub link needs an owner and a repository name." };
      const tagIndex = segments.indexOf("tag");
      const tag = tagIndex >= 0 ? segments[tagIndex + 1] : undefined;
      return github(`${owner}/${cleanSlug(name)}`, tag ? tag.replace(/^v/, "") : null);
    }
    if (host === "gamebanana.com") {
      const modsIndex = segments.indexOf("mods");
      const idPart = modsIndex >= 0 ? segments[modsIndex + 1] : segments[segments.length - 1];
      const id = Number(idPart);
      if (!Number.isInteger(id) || id <= 0) {
        return { kind: "unknown", reason: "That GameBanana link has no mod id in it." };
      }
      return gamebanana(id);
    }
    return {
      kind: "unknown",
      reason: `${host} is not a source this app can resolve. Use github.com or gamebanana.com.`,
    };
  }

  const versioned = SLUG_AT_VERSION.exec(input);
  if (versioned) {
    const owner = versioned[1] ?? "";
    const name = versioned[2] ?? "";
    const version = versioned[3] ?? "";
    return github(`${owner}/${cleanSlug(name)}`, version);
  }
  const plain = SLUG.exec(input);
  if (plain) {
    const owner = plain[1] ?? "";
    const name = plain[2] ?? "";
    return github(`${owner}/${cleanSlug(name)}`, null);
  }
  if (input.includes("@") && !input.includes("/")) {
    return { kind: "unknown", reason: "A GitHub spec is owner/repo@1.2.3. The owner is missing." };
  }
  return {
    kind: "unknown",
    reason: "Not recognised. Use owner/repo, owner/repo@1.2.3, a github.com link, or a gamebanana.com link.",
  };
}

export function describeSpec(spec: ParsedSpec): string {
  if (spec.kind === "github") {
    return spec.version
      ? `GitHub release ${spec.slug} at version ${spec.version}`
      : `GitHub repo ${spec.slug}, version to be chosen`;
  }
  if (spec.kind === "gamebanana") return `GameBanana mod ${spec.modId}`;
  return spec.reason;
}
