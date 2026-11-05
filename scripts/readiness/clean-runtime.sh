#!/usr/bin/env bash
# Verify a locally generated alpha archive on its supported clean target OS.
set -euo pipefail
if [[ $# != 2 ]]; then
    echo "Usage: $0 ARCHIVE.tar.gz REPORT_DIRECTORY" >&2
    exit 2
fi
root=$(cd -- "$(dirname -- "$0")/../.." && pwd)
archive=$(realpath -- "$1")
mkdir -p -- "$2"
reports=$(realpath -- "$2")
# Only use archives from our packaging workflow, not untrusted uploaded tar files.
(cd -- "$(dirname -- "$archive")" && sha256sum --check -- "$(basename -- "$archive").sha256")
temporary=$(mktemp -d)
trap 'rm -rf -- "$temporary"' EXIT
tar -xzf "$archive" -C "$temporary"
packages=("$temporary"/unigis-cli-alpha-*)
[[ ${#packages[@]} == 1 && -d ${packages[0]} ]]
# Non-root container user must traverse the temporary extraction directory.
chmod 755 "$temporary"
docker build -t unigis-clean-runtime:24.04 "$root/scripts/readiness/runtime" 2>&1 | tee "$reports/image-build.log"
docker image inspect --format '{{.Id}}' unigis-clean-runtime:24.04 > "$reports/image-id.txt"
docker run --rm --network none --read-only --cap-drop ALL \
    --security-opt no-new-privileges --tmpfs /tmp:rw,nosuid,nodev \
    --user "$(id -u):$(id -g)" \
    --mount "type=bind,src=${packages[0]},dst=/opt/UniGIS package,readonly" \
    --mount "type=bind,src=$reports,dst=/reports" \
    unigis-clean-runtime:24.04 '/opt/UniGIS package/runtime.py' --package '/opt/UniGIS package' \
    --report /reports/ubuntu24.json --require-clean-runtime 2>&1 | tee "$reports/runtime.log"
