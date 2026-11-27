#!/usr/bin/env bash
# kitchen-web in a real browser: builds the check module (wasm-bindgen --target web), serves it with
# kitchen-server's web_check example, and drives the page in headless Chromium (Playwright).
# Needs: the wasm32-unknown-unknown target, wasm-bindgen-cli matching the wasm-bindgen crate,
# Node with Playwright and its Chromium.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../../../.." && pwd)"
out="$(mktemp -d)"
trap 'kill ${server:-0} 2>/dev/null || true; rm -rf "$out"' EXIT

cargo build -q --manifest-path "$root/Cargo.toml" -p kitchen-web --features check --target wasm32-unknown-unknown --release
wasm-bindgen --target web --no-typescript --out-dir "$out" "$root/target/wasm32-unknown-unknown/release/kitchen_web.wasm"
cp "$here/index.html" "$root/runtime/kitchen-web/web/"*.js "$out/"

cargo build -q --manifest-path "$root/Cargo.toml" -p kitchen-server --example web_check
"$root/target/debug/examples/web_check" "$out" > "$out/server.json" &
server=$!
for _ in $(seq 100); do [ -s "$out/server.json" ] && break; sleep 0.1; done
read -r page code1 code2 signal < <(python3 -c 'import json,sys; s=json.load(open(sys.argv[1])); print(s["page"], s["codes"][0], s["codes"][1], s["signal"])' "$out/server.json")
query="code1=$code1&code2=$code2&signal=$(python3 -c 'import urllib.parse,sys; print(urllib.parse.quote(sys.argv[1]))' "$signal")"
NODE_PATH="${NODE_PATH:-$(npm root -g)}" node "$here/run.mjs" "$page" "$query"
