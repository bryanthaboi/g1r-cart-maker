# G1R Custom Cart Maker

A desktop app for building a **custom cart** for gen1recomp: pick a base game, 
pin a set of published mods to exact builds, freeze the options players inherit,
design the cartridge label, and either export a `.g1rcart` bundle or have the 
app create the cart's GitHub repo and publish it.

A cart is a manifest, not code. carts do not contain the mods they are made up
of (nor the base game).

## What it does

- **Assemble a cart.** Identity, base game, seal, finish, shell colour, the
  speed ladder, and one pin per mod, validated live against the engine's own
  rules with errors, warnings and notes kept distinct.
- **Add mods three ways.** Browse the community index, paste a GitHub repo URL
  or `owner/repo@1.2.3`, or paste a GameBanana mod URL or id. Each path resolves
  to the same pinned entry, with the published hash checked
- **Freeze mod options.** Read a mod's option schema from its archive or from a
  real install, render the rows as controls, and write the values the player
  inherits into the cart.
- **Design the label.** A layered canvas at the shipped templates' native
  500x441, with import, text, fit modes, alignment, undo/redo, a live cartridge
  preview, and export
- **Export or publish.** Write `<id>-<version>.g1rcart` to disk, or create the
  GitHub repo, push, tag, watch the release workflow publish the bundle, and
  offer to submit the cart to the main index

## Cartkit parity

The Rust core reimplements `tools/cartkit.py` from
[`bryanthaboi/gen1recomp`](https://github.com/bryanthaboi/gen1recomp). It is
held to byte-identical `.g1rcart` output, identical validation rule ids
(`CK001`-`CK005`, `CK100`, `CK101`, `CK110`, `CK111`), and the same three-way
findings model, by a golden-fixture test suite generated from real cartkit plus
cartkit's own selftest cases. A CI job compares the format constants against
`gen1recomp@dev` daily so the app notices when the format moves.

Nothing at runtime needs Python, a gen1recomp checkout, or a network connection
once feeds and archives are cached.

