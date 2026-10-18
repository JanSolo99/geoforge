#!/usr/bin/env bash
# Builds the GeoForge web app into app/dist/web (needs: rustup target add wasm32-unknown-unknown,
# and `trunk` 0.21 on PATH, or TRUNK=/path/to/trunk).
set -euo pipefail
cd "$(dirname "$0")/../app"
TRUNK="${TRUNK:-trunk}"
(cd apps/rustgis-web && "$TRUNK" build --release --public-url /)
cargo xtask web-pwa dist/web
test -f dist/web/index.html && ls dist/web/*_bg.wasm >/dev/null
echo "Built app/dist/web: $(du -sh dist/web | cut -f1)"
