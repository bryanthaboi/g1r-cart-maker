# Making a cart

A cart is a manifest. It names a base game and pins a list of mods to exact
published builds, and it ships no code of its own. Players install the
`.g1rcart` bundle and it boots as its own game, with its own save scope.

## 1. New Cart

Pick an id (letters, digits, `_` and `-`, up to 64 characters), a title (up to
48), a base game, a shell colour and a seal:

| Seal | What the player can do |
| --- | --- |
| `sealed` | Nothing. The pinned set runs exactly as pinned. |
| `sealed+` | Toggle any pinned mod on or off. Nothing else. |
| `open` | Add more mods on top of the pinned set. |

The app writes a directory: `cart.json`, `label.png`, `label.layers.json`,
`README.md`, `CHANGELOG.md`, `.gitignore` and
`.github/workflows/release.yml`. That directory is the project; there is no
separate project file, so it is yours to keep in git.

## 2. Add mods

Three ways, all producing the same pinned entry:

- **The index.** The community index ships preconfigured. Search, filter by base
  game, category or tag, and add. Compatibility notes (mod API, engine range,
  profile, link play, permissions) are shown before you add.
- **A GitHub repo.** Paste `owner/repo@1.2.3`, a github.com URL, or just
  `owner/repo` and pick a tag. The app looks for a release tagged `v1.2.3` then
  `1.2.3`, takes the asset named `<mod-id>-<version>.zip` (or the only `.zip` if
  there is exactly one), and records the sha256 from the release's
  `sha256sums.txt`, or by hashing the download when there is none.
- **GameBanana.** Paste a mod URL or id. The app reads the v11 API, lets you
  pick a file when the mod publishes several, and records the published md5.

A pin is always an exact build. If a resolution fails, the message says what the
author needs to change.

### Options

If a mod publishes an option schema, the app renders it as real controls and
writes the values you choose into that pin. The player inherits them the first
time the cart boots. Two sources, in order: the mod's own archive, or a real
install's `mod_option_schemas.json` (Settings, game directory). When neither is
available you can still enter keys and values by hand.

### Load order

Drag to reorder. `load_order` must name every pinned mod exactly once, so the
app keeps it a permutation for you.

## 3. Design the label

The canvas is 500x441, the size of the engine's own cartridge labels. Start from
one of the six shipped templates, replace images, edit text, and align, snap and
nudge until it looks right. The preview shows the cart as the launcher draws it,
with your shell colour and finish.

Export writes the PNG into the cart directory. Anything the manifest would
reject - not a PNG, over 1 MB, a path that leaves the cart - is refused before
it is written, with an offer to recompress. Over 256 KB is allowed but warned
about: a cart label wants a few KB.

## 4. Validate

Offline validation runs on every edit. Online validation additionally resolves
every pin and compares the published hash.

| Severity | Effect |
| --- | --- |
| Error | The cart is invalid. Export is refused. |
| Warning | Export is refused too: packing is always strict. |
| Note | Never fails anything. An unreachable API is a note. |

Every finding carries a rule id, so a finding here reads the same as one from
the release workflow's own run of cartkit.

## 5. Export or publish

**Export** writes `<id>-<version>.g1rcart`. Install it by dropping it on the
launcher, using Import cart on the game's page, or copying it into the
launcher's `carts/` folder.

**Prepare GitHub Repo** does the rest: it checks that `git` and `gh` are present
and authenticated (showing per-platform install instructions when they are not),
creates the repo, commits, pushes, tags `v<version>`, watches the release
workflow validate and pack the cart with the authoritative cartkit, and confirms
the `.g1rcart` landed on the release. Then it offers to submit the repo to the
community index, showing exactly what will be submitted first.

The app never handles a GitHub token. `gh` owns that credential; the app only
reports which one it will use.

## Index readiness

For a cart repo to be listable it needs: a public repo, `cart.json` at the root,
the eight required fields with at least one valid pin, a release tagged exactly
`v<version>`, and the `.g1rcart` attached to it named `<id>-<version>.g1rcart`.
A `sha256sums.txt`, a thumbnail, a description URL, a licence, a summary within
120 characters and some tags are recommended. The Publish screen tracks all of
it live.
