#!/usr/bin/env bash
# Builds the web app from origin/main on this machine and force-pushes it to the `web-dist` branch,
# which Netlify deploys (docs/WEB.md). Replaces the GitHub workflow while Actions minutes are off.
#
#   scripts/publish-web-dist.sh            # only if web-dist does not have origin/main's build yet
#   scripts/publish-web-dist.sh --force
#
# The build (fat LTO WebAssembly) needs about 8 GB, so it holds new coordinator turns (PAUSE),
# waits for the turn in progress and any other Cargo build to finish, requires 9 GiB
# available RAM (8 GiB build estimate plus headroom), and builds with one Cargo job.
# It uses its own worktree and target directory, so agents' uncommitted edits are never published.
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT=$PWD
CACHE="${XDG_CACHE_HOME:-$HOME/.cache}/unigis-web"
SRC="$CACHE/src"
export CARGO_TARGET_DIR="$CACHE/target" CARGO_BUILD_JOBS=1
MIN_AVAILABLE_GIB="${WEB_BUILD_MIN_AVAILABLE_GIB:-9}"
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
command -v "${TRUNK:-trunk}" >/dev/null || { echo "trunk 0.21 is not on PATH (docs/WEB.md)" >&2; exit 1; }

# A read-only check: no fetch, worktree, build, coordinator pause, or publish.
if [ "${1:-}" = "--check-resources" ]; then
    exec python3 "$ROOT/scripts/web-build-memory.py" --minimum-gib "$MIN_AVAILABLE_GIB"
fi
mkdir -p "$CACHE"
exec 9>"$CACHE/publish.lock"
flock -n 9 || { echo "another web publisher is already running"; exit 0; }

git fetch -q origin main web-dist
sha=$(git rev-parse --short=7 origin/main)
last=$(git log -1 --format=%s origin/web-dist 2>/dev/null | sed -n 's/^Web build of //p')
if [ "$sha" = "$last" ] && [ "${1:-}" != "--force" ]; then
    echo "web-dist already has the build of $sha"
    exit 0
fi

REMOTE="$(cd "$(git rev-parse --git-common-dir)" && pwd)/agent-coordinator/remote"
if [ -d "$REMOTE" ] && [ ! -e "$REMOTE/PAUSE" ]; then
    : > "$REMOTE/PAUSE"
    trap 'rm -f "$REMOTE/PAUSE"' EXIT
fi
busy() {
    python3 - "$REMOTE" <<'PY' && return 0
import json, sys
from pathlib import Path
try:
    s = json.loads((Path(sys.argv[1]) / "status.json").read_text())
    alive = b"--daemon" in Path(f"/proc/{s['pid']}/cmdline").read_bytes()
except Exception:
    sys.exit(1)
sys.exit(0 if alive and s.get("state") == "running" else 1)
PY
    if pgrep -u "$USER" -x 'cargo|rustc|trunk' >/dev/null; then return 0; fi
    # busy returns success while either build activity or insufficient RAM blocks us.
    if python3 "$ROOT/scripts/web-build-memory.py" --minimum-gib "$MIN_AVAILABLE_GIB" --quiet; then
        return 1
    fi
    return 0
}
echo "waiting for an idle builder and at least $MIN_AVAILABLE_GIB GiB available RAM"
for _ in $(seq 360); do busy || break; sleep 20; done
busy && { echo "machine busy or RAM below $MIN_AVAILABLE_GIB GiB after 2 hours; try again later" >&2; exit 1; }

echo "building web app for $sha"
if [ ! -d "$SRC" ]; then
    mkdir -p "$CACHE"
    git worktree add -q --detach "$SRC" origin/main
else
    git -C "$SRC" checkout -q --detach --force origin/main
    git -C "$SRC" clean -qfd -e app/dist
fi
python3 "$ROOT/scripts/web-build-memory.py" --minimum-gib "$MIN_AVAILABLE_GIB"
nice -n 10 "$SRC/scripts/build-web.sh"

OUT=$(mktemp -d)
cp -a "$SRC/app/dist/web/." "$OUT/"
sed 's|publish = "app/dist/web"|publish = "."|' "$SRC/netlify.toml" > "$OUT/netlify.toml"
url=$(git remote get-url origin)
(
    cd "$OUT"
    git init -q -b web-dist
    git -c core.hooksPath=/dev/null add -A
    git -c core.hooksPath=/dev/null commit -q -m "Web build of $sha"
    git -c core.hooksPath=/dev/null push -q --force "$url" web-dist
)
rm -rf "$OUT"
echo "pushed web-dist: Web build of $sha"
