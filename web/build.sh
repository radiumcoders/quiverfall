#!/usr/bin/env bash
# Builds the browser version into dist/ (static files: deploy anywhere, e.g. `vercel deploy dist`).
# Needs: rustup target add wasm32-unknown-unknown; cargo install wasm-bindgen-cli --version <lockfile version>
# Optional: wasm-opt (binaryen) on PATH to shrink the wasm.
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build --profile web --target wasm32-unknown-unknown
# keep the Vercel project link (dist/.vercel) across rebuilds
keep=$(mktemp -d)
[ -d dist/.vercel ] && mv dist/.vercel "$keep/"
rm -rf dist
mkdir -p dist
[ -d "$keep/.vercel" ] && mv "$keep/.vercel" dist/
rmdir "$keep" 2>/dev/null || true
wasm-bindgen --no-typescript --target web --out-dir dist --out-name quiverfall \
  target/wasm32-unknown-unknown/web/quiverfall.wasm
if command -v wasm-opt >/dev/null; then
  wasm-opt -Os --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
    --enable-mutable-globals --enable-reference-types --enable-multivalue \
    dist/quiverfall_bg.wasm -o dist/quiverfall_bg.wasm
fi
cp web/index.html web/vercel.json dist/
cp -r assets dist/assets
du -sh dist dist/quiverfall_bg.wasm
