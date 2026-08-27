#!/usr/bin/env python3
"""Regenerate the golden parity fixtures from a gen1recomp checkout.

    python3 generate.py /path/to/gen1recomp

Every .g1rcart and .png here is the output of that repo's tools/cartkit.py.
The Rust suite asserts byte equality against them; nothing at runtime needs
Python or this script.
"""

import base64
import importlib.util
import json
import os
import shutil
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))


def load_cartkit(repo):
    path = os.path.join(repo, "tools", "cartkit.py")
    spec = importlib.util.spec_from_file_location("cartkit", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def good_cart(ck):
    return ck.good_cart()


def torture_cart():
    return {
        "schema": 1,
        "id": "edge-case",
        "title": "Pokémon \"quoted\" \\ back",
        "version": "1.0.0-rc.1+build.5",
        "author": "auteur namé",
        "repo": "someone/edge-case",
        "summary": "quotes \" and a backslash \\ and ünïcode",
        "shell": "#0A0B0C",
        "finish": "sparkle+holo",
        "label": "art/label.png",
        "base": "crystal",
        "engine": ">=1.0.0 <2.0.0 || ^3.1",
        "seal": "open",
        "speeds": [1, 2, 200],
        "mods": [
            {"id": "zeta", "source": "github", "repo": "o/zeta",
             "version": "0.0.1", "sha256": "f" * 64,
             "options": {"n": 3, "f": 0.5, "big": 1e20, "small": 1.5e-7,
                         "neg": -2, "t": True, "s": "vál\"ue",
                         "1numeric": "keyed", "with space": "x",
                         "end": "reserved word key"}},
            {"id": "alpha", "source": "gamebanana", "mod": 546899,
             "file": 1294214, "md5": "b" * 32, "enabled": False,
             "options": {}},
        ],
        "load_order": ["alpha", "zeta"],
    }


def minimal_cart():
    return {
        "schema": 1,
        "id": "minimal",
        "title": "Minimal",
        "version": "0.1.0",
        "author": "someone",
        "shell": "#8b1a1a",
        "base": "red",
        "mods": [{"id": "only", "source": "github", "repo": "o/only",
                  "version": "1.0.0", "sha256": "c" * 64}],
    }


def write(name, body):
    path = os.path.join(HERE, name)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    mode = "wb" if isinstance(body, bytes) else "w"
    with open(path, mode) as fh:
        fh.write(body)
    print("wrote", name)


def pack_case(ck, repo, name, cart, with_label):
    cart = dict(cart)
    if not with_label:
        cart.pop("label", None)
    root = tempfile.mkdtemp(prefix="cartfix-")
    try:
        cart_dir = os.path.join(root, cart["id"])
        os.makedirs(cart_dir, exist_ok=True)
        ck.write_cart(cart_dir, cart)
        if with_label and cart.get("label"):
            art_path = os.path.join(cart_dir, cart["label"])
            os.makedirs(os.path.dirname(art_path) or cart_dir, exist_ok=True)
            with open(art_path, "wb") as fh:
                fh.write(ck.label_art(cart["shell"]))
        out = os.path.join(root, "out.g1rcart")
        code = subprocess.call(
            [sys.executable, os.path.join(repo, "tools", "cartkit.py"),
             "pack", cart_dir, "-o", out, "--quiet", "--repo", repo])
        if code != 0:
            raise SystemExit(f"{name}: cartkit pack refused ({code})")
        write(f"{name}.cart.json",
              open(os.path.join(cart_dir, "cart.json"), encoding="utf-8").read())
        write(f"{name}.g1rcart", open(out, "rb").read())
    finally:
        shutil.rmtree(root, ignore_errors=True)


def scaffold_case(ck, repo):
    root = tempfile.mkdtemp(prefix="cartscaf-")
    try:
        code = subprocess.call(
            [sys.executable, os.path.join(repo, "tools", "cartkit.py"),
             "scaffold", "demo_cart", "--into", root, "--quiet",
             "--title", "Demo Cart", "--author", "someone",
             "--base", "gold", "--shell", "#2f6f4f", "--seal", "sealed+",
             "--summary", "A demo cart.", "--github", "someone/demo-cart",
             "--repo", repo])
        if code != 0:
            raise SystemExit("scaffold refused")
        cart_dir = os.path.join(root, "demo_cart")
        for name in ("cart.json", "README.md", "CHANGELOG.md"):
            write(os.path.join("scaffold", name),
                  open(os.path.join(cart_dir, name), encoding="utf-8").read())
        write(os.path.join("scaffold", "label.png"),
              open(os.path.join(cart_dir, "label.png"), "rb").read())
        write(os.path.join("scaffold", "release.yml"),
              open(os.path.join(cart_dir, ".github", "workflows", "release.yml"),
                   encoding="utf-8").read())
        write(os.path.join("scaffold", "engine_version.txt"),
              ck.engine_version(repo))
    finally:
        shutil.rmtree(root, ignore_errors=True)


def label_cases(ck):
    shells = {}
    for shell in ("#8b1a1a", "#123456", "#ffffff", "#000000", "#2f6f4f",
                  "#0a0b0c"):
        shells[shell] = base64.b64encode(ck.label_art(shell)).decode("ascii")
    write("label_art.json", json.dumps(shells, indent=2, sort_keys=True) + "\n")


def lua_cases(ck):
    cases = []
    for text in ('a"b', "a\\b", "a\nb", "a\tb", "a\t1", "a\rb", "Pokémon",
                 "\x7f", "\x7f7", "\x00", "\x1b[0m", "", "tab\tand\ndigit9"):
        cases.append({"text": text, "lua": ck.lua_string(text)})
    numbers = []
    for value in (3, -2, 0.5, 0.0, -0.0, 1.0, 1e20, 1.5e-7, 1e-5, 123456789012345.0,
                  1234567890123456.0, 0.1, 2.5e-10, -1.25, 3.14159265358979,
                  1e14, 1e13, 99999999999999.0, 100000000000000.0):
        numbers.append({"json": json.dumps(value),
                        "lua": ck.lua_value(value)})
    write("lua_strings.json", json.dumps(cases, ensure_ascii=False, indent=2) + "\n")
    write("lua_numbers.json", json.dumps(numbers, indent=2) + "\n")


def validation_cases(ck):
    """Every selftest mutation, with the findings cartkit reports for it."""
    cases = []

    def record(name, cart):
        found = ck.schema_findings(cart)
        cases.append({
            "name": name,
            "cart": cart,
            "findings": [f.as_dict() for f in found],
        })

    record("good", ck.good_cart())
    for key, value in (("id", "no spaces here"), ("id", "x" * 65),
                       ("title", ""), ("title", "t" * 49),
                       ("version", "1.0"), ("version", "v1.0.0"),
                       ("author", ""), ("shell", "8b1a1a"),
                       ("shell", "#8b1a1"), ("base", "nonesuch"),
                       ("seal", "welded"), ("schema", 2),
                       ("summary", "s" * 121), ("repo", "someone"),
                       ("engine", ">=1.0.0 <<2.0.0"), ("schema", None),
                       ("schema", True), ("finish", "matte"),
                       ("speeds", []), ("speeds", [5]), ("speeds", "fast"),
                       ("label", "/etc/passwd"), ("label", "../out.png"),
                       ("label", "a/../../b.png"), ("label", "x" * 129),
                       ("label", "C:\\art.png"), ("label", "sub dir/a.png"),
                       ("load_order", ["harder-trainers"]),
                       ("load_order", ["harder-trainers", "ghost"]),
                       ("load_order", ["harder-trainers", "harder-trainers"]),
                       ("load_order", "harder-trainers"),
                       ("load_order", [1, 2]),
                       ("colour", "red"), ("mods", []), ("mods", "nope")):
        cart = ck.good_cart()
        cart[key] = value
        record(f"{key}={json.dumps(value)}", cart)

    for key in ("repo", "summary", "engine", "label", "load_order", "seal"):
        cart = ck.good_cart()
        del cart[key]
        record(f"drop {key}", cart)

    for patch in ({"source": "torrent"}, {"repo": "nope"}, {"version": "1.0"},
                  {"sha256": "A" * 64}, {"sha256": "a" * 63},
                  {"source": "local"}, {"mod": 1}, {"md5": "b" * 32},
                  {"enabled": "yes"}, {"extra": 1},
                  {"repo": "owner/example-mod", "sha256": "0" * 64}):
        cart = ck.good_cart()
        cart["mods"][0].update(patch)
        record(f"github pin {json.dumps(patch, sort_keys=True)}", cart)

    for patch in ({"mod": 0}, {"mod": "546899"}, {"file": -1},
                  {"md5": "B" * 32}, {"md5": "b" * 31}, {"repo": "a/b"},
                  {"version": "1.0.0"}, {"mod": True}):
        cart = ck.good_cart()
        cart["mods"][1].update(patch)
        record(f"gamebanana pin {json.dumps(patch, sort_keys=True)}", cart)

    for value in ({"a": {"nested": 1}}, {"a": None}, {"a": ["list"]},
                  {"k" * 65: 1}, {"a": "x" * 257},
                  dict((str(n), n) for n in range(65)),
                  {"a\t": 1}, {"a": "line\nbreak"}, {}, {"ok": 1.5}):
        cart = ck.good_cart()
        cart["mods"][0]["options"] = value
        record(f"options {json.dumps(value, sort_keys=True)[:60]}", cart)

    cart = ck.good_cart()
    cart["mods"] = [dict(cart["mods"][0]) for _ in range(65)]
    record("65 mods", cart)
    cart = ck.good_cart()
    cart["mods"][1] = dict(cart["mods"][0])
    cart["load_order"] = ["harder-trainers", "harder-trainers"]
    record("duplicate pin", cart)

    write("validation.json", json.dumps(cases, ensure_ascii=False, indent=2) + "\n")


def spec_cases(ck):
    specs = []
    for text in ("owner/repo@1.2.3", "https://github.com/owner/repo@1.2.3",
                 "owner/repo.git@v1.2.3", "http://github.com/o/r@0.0.1",
                 "https://gamebanana.com/mods/546899", "gamebanana:546899",
                 "546899", "www.gamebanana.com/mods/12/", "not a spec",
                 "owner/repo", "  owner/repo@1.2.3  ", "owner/repo@",
                 "gamebanana:0", "https://gamebanana.com/mods/abc"):
        source, target, version = ck.parse_spec(text)
        specs.append({"spec": text, "source": source,
                      "target": target, "version": version})
    options = []
    for text in ("a=true", "a=FALSE", "a=3", "a=1.5", "a=hard", "a=",
                 "a= 7 ", "a=0x10", "a=1e3", "a=-2", "nope", "=v",
                 "a=b=c", "a=inf", "a=nan"):
        try:
            key, value = ck.parse_option(text)
        except ValueError:
            options.append({"text": text, "key": None, "value": None,
                            "nonfinite": False})
            continue
        finite = not (isinstance(value, float)
                      and (value != value or value in (float("inf"),
                                                       float("-inf"))))
        options.append({"text": text, "key": key,
                        "value": value if finite else None,
                        "nonfinite": not finite})
    ids = [{"text": t, "id": ck.derive_id(t)} for t in
           ("Harder Trainers", "  --lead--  ", "MOD", "a" * 70, "!!!",
            "mod_v2.zip", "ünïcode", "a--b")]
    ranges = [{"range": t, "problem": ck.range_problem(t)} for t in
              (">=1.0.0 <2.0.0", "^1.2", "1.2.3", ">1 || <0.9", "<=2", "",
               ">=x", ">>1.0.0", "1.0.0 || ", "v1.0.0", "==1.0.0", "!=1",
               ">=1.0.0-rc.1", "1.0.0+build")]
    write("specs.json", json.dumps(
        {"specs": specs, "options": options, "ids": ids, "ranges": ranges},
        ensure_ascii=False, indent=2) + "\n")


def main():
    if len(sys.argv) != 2:
        print(__doc__)
        return 2
    repo = os.path.abspath(sys.argv[1])
    ck = load_cartkit(repo)
    pack_case(ck, repo, "good", good_cart(ck), True)
    pack_case(ck, repo, "good_nolabel", good_cart(ck), False)
    cart = good_cart(ck)
    del cart["seal"]
    del cart["load_order"]
    pack_case(ck, repo, "defaults", cart, True)
    pack_case(ck, repo, "torture", torture_cart(), True)
    pack_case(ck, repo, "minimal", minimal_cart(), False)
    scaffold_case(ck, repo)
    label_cases(ck)
    lua_cases(ck)
    validation_cases(ck)
    spec_cases(ck)
    return 0


if __name__ == "__main__":
    sys.exit(main())
