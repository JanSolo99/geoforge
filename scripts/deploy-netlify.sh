#!/usr/bin/env bash
# Deploys app/dist/web (scripts/build-web.sh) to Netlify. Needs NETLIFY_AUTH_TOKEN and
# NETLIFY_SITE_ID in the environment (or run `npx netlify-cli login` and `link` once).
# Without --prod this makes a draft deploy with its own URL.
set -euo pipefail
cd "$(dirname "$0")/.."
test -f app/dist/web/index.html || { echo "build first: scripts/build-web.sh" >&2; exit 1; }
npx --yes netlify-cli@17 deploy --dir app/dist/web "$@"
