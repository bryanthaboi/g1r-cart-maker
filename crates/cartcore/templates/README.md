# {{title}}

A custom cart for the LOVE2D Pokemon engine: a pinned mod setup that plays as
its own game on top of `{{base}}`. It ships no code -- every mod named here is
published separately, and the cart pins each one to an exact build.

## Files

- `cart.json` - identity, base game, seal, and one pin per mod
- `{{label}}` - the cart label art the launcher draws
- `.github/workflows/release.yml` - packs and publishes on a `v*` tag

## Loop

Every command below is `tools/cartkit.py` from a gen1recomp checkout, pointed
at this directory.

1. pin a mod:

   ```sh
   python3 tools/cartkit.py pin . owner/repo@1.2.3
   python3 tools/cartkit.py pin . https://gamebanana.com/mods/546899
   ```

2. freeze an option the player inherits:

   ```sh
   python3 tools/cartkit.py pin . owner/repo@1.2.3 --option difficulty=hard
   ```

3. check it, hashes and all:

   ```sh
   python3 tools/cartkit.py validate . --online
   ```

4. build the bundle players load:

   ```sh
   python3 tools/cartkit.py pack .
   ```

`seal` is `{{seal}}`. A sealed cart loads exactly the mods listed below and
nothing else; an open cart lets the player add more on top.

## Releasing

Bump `version` in `cart.json`, tag it `v<version>`, and push the tag. The
workflow validates, packs, and attaches `{{id}}-<version>.g1rcart` to the
release.
