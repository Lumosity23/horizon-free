#!/usr/bin/env python3
"""Puts Tailcat for Minecraft into a source tree of Horizon.

    python3 tailcat/apply.py SOURCE_DIR

SOURCE_DIR is Horizon's own source (a nightly's `horizon-source.tar.gz`, unpacked). It gets this
fork's game-side changes (`tailcat/game.patch`: horizon-friends' `tailcat` module and the Play with
Friends button) and their two language strings (`tailcat/lang-keys.json`). The game then builds
from exactly the commit the official folder's tools and exporters were made from, which is what keeps
its prepared resources valid (a newer game wants data an older `horizon-prepare` doesn't write).
"""
import json
import subprocess
import sys
from pathlib import Path

here = Path(__file__).resolve().parent
src = Path(sys.argv[1]).resolve()
if not (src / "Cargo.toml").is_file():
    sys.exit(f"{src}: not a Horizon source tree (no Cargo.toml)")

# `git apply` works outside a repository too, and exists on every runner.
subprocess.run(["git", "apply", "--whitespace=nowarn", "--verbose", str(here / "game.patch")], cwd=src, check=True)

keys = json.loads((here / "lang-keys.json").read_text(encoding="utf-8"))
for lang, entries in keys.items():
    path = src / "crates/horizon-client/assets/horizon/lang" / f"{lang}.json"
    text = path.read_text(encoding="utf-8")
    body = text.rstrip()
    assert body.endswith("}"), path
    add = "".join(
        ",\n  " + json.dumps(k) + ": " + json.dumps(v, ensure_ascii=False)
        for k, v in entries.items()
        if json.dumps(k) not in text
    )
    new = body[:-1].rstrip() + add + "\n}\n"
    json.loads(new)  # still valid
    path.write_text(new, encoding="utf-8")
print("Tailcat applied to", src)
