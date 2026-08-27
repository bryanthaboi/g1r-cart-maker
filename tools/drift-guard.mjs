// Fails when gen1recomp's cart format moves away from the constants this repo
// carries. The format lives in another repo; this is how we notice.
//
//   node tools/drift-guard.mjs [--ref dev] [--repo bryanthaboi/gen1recomp]

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..");

function arg(name, fallback) {
  const at = process.argv.indexOf(`--${name}`);
  return at >= 0 && process.argv[at + 1] ? process.argv[at + 1] : fallback;
}

const repo = arg("repo", "bryanthaboi/gen1recomp");
const ref = arg("ref", "dev");
const raw = (path) => `https://raw.githubusercontent.com/${repo}/${ref}/${path}`;

async function fetchText(path) {
  const url = raw(path);
  const response = await fetch(url, { headers: { "user-agent": "g1r-cart-maker drift guard" } });
  if (!response.ok) throw new Error(`${url}: HTTP ${response.status}`);
  return response.text();
}

const ours = readFileSync(join(root, "crates/cartcore/src/schema.rs"), "utf8");
const template = readFileSync(join(root, "crates/cartcore/templates/release.yml"), "utf8");

function rustList(name) {
  const match = ours.match(new RegExp(`pub const ${name}: \\[[^\\]]+\\] = \\[([^\\]]*)\\];`, "s"));
  if (!match) throw new Error(`schema.rs has no ${name}`);
  return match[1]
    .split(",")
    .map((item) => item.trim().replace(/^"|"$/g, ""))
    .filter(Boolean);
}

function rustConst(name) {
  const match = ours.match(new RegExp(`pub const ${name}: [^=]+= ([^;]+);`));
  if (!match) throw new Error(`schema.rs has no ${name}`);
  return match[1].trim().replace(/_/g, "").replace(/"/g, "");
}

function pyTuple(source, name) {
  const match = source.match(new RegExp(`^${name} = \\(([^)]*)\\)`, "m"));
  if (!match) throw new Error(`cartkit.py has no ${name}`);
  return match[1]
    .split(",")
    .map((item) => item.trim().replace(/^"|"$/g, ""))
    .filter(Boolean);
}

function pyConst(source, name) {
  const match = source.match(new RegExp(`^${name} = ([^\\n#]+)`, "m"));
  if (!match) throw new Error(`cartkit.py has no ${name}`);
  return match[1].trim();
}

function luaSet(source, name) {
  const match = source.match(new RegExp(`CartManifest\\.${name} = \\{([^}]*)\\}`, "s"));
  if (!match) throw new Error(`CartManifest.lua has no ${name}`);
  return [...match[1].matchAll(/(?:\[?"([^"]+)"\]?|([A-Za-z_][A-Za-z0-9_]*))\s*=\s*true/g)]
    .map((entry) => entry[1] ?? entry[2])
    .filter(Boolean);
}

function luaNumber(source, name) {
  const match = source.match(new RegExp(`CartManifest\\.${name} = ([^\\n]+)`));
  if (!match) throw new Error(`CartManifest.lua has no ${name}`);
  return match[1].trim();
}

const failures = [];
const same = (label, ours_, theirs) => {
  const a = JSON.stringify(ours_);
  const b = JSON.stringify(theirs);
  if (a !== b) failures.push(`${label}: this repo has ${a}, ${repo}@${ref} has ${b}`);
};

const cartkit = await fetchText("tools/cartkit.py");
const manifest = await fetchText("src/carts/CartManifest.lua");
const gameSpeed = await fetchText("src/core/GameSpeed.lua");
const upstreamWorkflow = await fetchText("tools/cart_release_workflow.yml");

same("BASES", rustList("BASES"), pyTuple(cartkit, "BASES"));
same("SEALS", rustList("SEALS").sort(), pyTuple(cartkit, "SEALS").sort());
same("FINISHES", rustList("FINISHES").sort(), pyTuple(cartkit, "FINISHES").sort());
same("SOURCES", rustList("SOURCES").sort(), pyTuple(cartkit, "SOURCES").sort());
same("CART_KEYS", rustList("CART_KEYS"), pyTuple(cartkit, "CART_KEYS"));
same("MOD_KEYS", rustList("MOD_KEYS"), pyTuple(cartkit, "MOD_KEYS"));
same(
  "SPEED_LEVELS",
  rustList("SPEED_LEVELS").map(Number),
  pyTuple(cartkit, "SPEED_LEVELS").map(Number),
);
same("CART_SCHEMA", rustConst("CART_SCHEMA"), pyConst(cartkit, "CART_SCHEMA"));
same("BUNDLE_VERSION", rustConst("BUNDLE_VERSION"), pyConst(cartkit, "BUNDLE_VERSION"));
same("BUNDLE_FORMAT", rustConst("BUNDLE_FORMAT"), pyConst(cartkit, "BUNDLE_FORMAT").replace(/"/g, ""));
same("MAX_MODS", rustConst("MAX_MODS"), pyConst(cartkit, "MAX_MODS"));
same("MAX_OPTIONS", rustConst("MAX_OPTIONS"), pyConst(cartkit, "MAX_OPTIONS"));
same("MAX_LABEL_PATH", rustConst("MAX_LABEL_PATH"), pyConst(cartkit, "MAX_LABEL_PATH"));

same("CartManifest.SEALS", rustList("SEALS").sort(), luaSet(manifest, "SEALS").sort());
same("CartManifest.FINISHES", rustList("FINISHES").sort(), luaSet(manifest, "FINISHES").sort());
same(
  "CartManifest.SOURCES",
  rustList("RUNTIME_SOURCES").sort(),
  luaSet(manifest, "SOURCES").sort(),
);
same("CartManifest.MAX_ID", rustConst("MAX_ID"), luaNumber(manifest, "MAX_ID"));
same("CartManifest.MAX_TITLE", rustConst("MAX_TITLE"), luaNumber(manifest, "MAX_TITLE"));
same("CartManifest.MAX_AUTHOR", rustConst("MAX_AUTHOR"), luaNumber(manifest, "MAX_AUTHOR"));
same("CartManifest.MAX_SUMMARY", rustConst("MAX_SUMMARY"), luaNumber(manifest, "MAX_SUMMARY"));
same("CartManifest.MAX_LABEL", rustConst("MAX_LABEL_PATH"), luaNumber(manifest, "MAX_LABEL"));
same("CartManifest.MAX_OPTION_KEY", rustConst("MAX_OPTION_KEY"), luaNumber(manifest, "MAX_OPTION_KEY"));
same("CartManifest.MAX_OPTION_TEXT", rustConst("MAX_OPTION_TEXT"), luaNumber(manifest, "MAX_OPTION_TEXT"));
same("CartManifest.SCHEMA", rustConst("CART_SCHEMA"), luaNumber(manifest, "SCHEMA"));

const levels = gameSpeed.match(/LEVELS\s*=\s*\{([^}]*)\}/);
if (!levels) failures.push("GameSpeed.lua has no LEVELS");
else {
  same(
    "GameSpeed.LEVELS",
    rustList("SPEED_LEVELS").map(Number),
    levels[1].split(",").map((item) => Number(item.trim())).filter((n) => !Number.isNaN(n)),
  );
}

const normalize = (text) => text.replace(/\r\n/g, "\n").trimEnd();
if (normalize(template) !== normalize(upstreamWorkflow)) {
  failures.push(
    "crates/cartcore/templates/release.yml no longer matches tools/cart_release_workflow.yml upstream",
  );
}

if (failures.length) {
  console.error(`drift guard: ${failures.length} mismatch(es) against ${repo}@${ref}`);
  for (const failure of failures) console.error(`  ${failure}`);
  process.exit(1);
}
console.log(`drift guard: cart format constants still match ${repo}@${ref}`);
