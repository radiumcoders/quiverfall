#!/usr/bin/env python3
"""Generate raw art with Cursor's agent CLI (GPT-5.6 Sol image generation).

    tools/gen_assets.py                 # generate everything missing from art/raw/
    tools/gen_assets.py player moth     # (re)generate just these
    tools/gen_assets.py --all           # regenerate everything

Prompts live in tools/assets.json. Raw images land in art/raw/<name>.png; run
`cargo run -p assetproc` afterwards to key/slice them into assets/.
"""

import json
import os
import shutil
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
RAW = ROOT / "art" / "raw"
MODEL = os.environ.get("QF_MODEL", "gpt-5.6-sol-high")
JOBS = int(os.environ.get("QF_JOBS", "5"))


def build_prompt(manifest, a):
    if a["kind"] == "fx":
        # glowing effects go on black and get luminance-keyed by assetproc
        parts = [a["prompt"], manifest["fx_style"], manifest["fx_bg"]]
    else:
        parts = [a["prompt"], manifest["style"]]
    if a["kind"] in ("sprite", "sheet", "layer"):
        parts.append(manifest["sprite_bg"])
    if "size" in a:
        parts.append(f"Image format: {a['size']}.")
    return " ".join(parts)


def generate(manifest, a):
    out = RAW / f"{a['name']}.png"
    prompt = build_prompt(manifest, a)
    instruction = (
        "Use your built-in image generation tool to generate exactly ONE image and save it as "
        "out.png in the current directory. Do not write code, do not edit or post-process the "
        "image, do not create any other files. When finished reply with just the pixel "
        "dimensions. Image prompt:\n\n" + prompt
    )
    for attempt in range(2):
        with tempfile.TemporaryDirectory(prefix=f"qf-{a['name']}-") as tmp:
            try:
                r = subprocess.run(
                    ["cursor-agent", "-p", "--model", MODEL, "--trust", "--force",
                     "--output-format", "text", instruction],
                    cwd=tmp, capture_output=True, text=True, timeout=900,
                )
                msg = (r.stdout or r.stderr).strip().splitlines()[-1:] or [""]
            except subprocess.TimeoutExpired:
                msg = ["timeout"]
            pngs = sorted((p for p in Path(tmp).rglob("*") if p.suffix.lower() in (".png", ".jpg", ".jpeg", ".webp")), key=lambda p: p.stat().st_mtime)
            if pngs:
                shutil.move(str(pngs[-1]), out)
                print(f"[ok]   {a['name']:16} {msg[0][:80]}", flush=True)
                return True
        print(f"[retry] {a['name']} ({msg[0][:80]})", flush=True)
    print(f"[FAIL] {a['name']}", flush=True)
    return False


def main():
    manifest = json.loads((ROOT / "tools" / "assets.json").read_text())
    RAW.mkdir(parents=True, exist_ok=True)
    args = [x for x in sys.argv[1:] if not x.startswith("--")]
    every = "--all" in sys.argv
    todo = [
        a for a in manifest["assets"]
        if ((a["name"] in args) if args else (every or not (RAW / f"{a['name']}.png").exists()))
    ]
    print(f"generating {len(todo)} assets with {MODEL}, {JOBS} at a time", flush=True)
    with ThreadPoolExecutor(JOBS) as ex:
        results = list(ex.map(lambda a: generate(manifest, a), todo))
    failed = [a["name"] for a, ok in zip(todo, results) if not ok]
    if failed:
        print("failed:", " ".join(failed))
        sys.exit(1)


if __name__ == "__main__":
    main()
