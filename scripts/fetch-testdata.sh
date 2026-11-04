#!/usr/bin/env bash
# Downloads large / third-party test corpora into testdata/large (git-ignored).
set -euo pipefail
cd "$(dirname "$0")/.."
dest=testdata/large
mkdir -p "$dest"
# Review corpus updates explicitly: upstream development fixtures can require a
# newer GDAL than CI. Record the revision beside the downloaded data.
gdal_revision=fa1fc3c77e75787c614e12a2ba5ac366db833e1f

# GDAL autotest data (MIT):
#  - shapefiles: all shape types, Z/M, MultiPatch, odd DBFs, code pages
#  - File Geodatabases: all geometry/field types, 64-bit OIDs, curves, sparse tables
if [ ! -d "$dest/gdal-autotest" ] || [ ! -d "$dest/filegdb" ] ||
   [ "$(cat "$dest/gdal-revision" 2>/dev/null || true)" != "$gdal_revision" ]; then
    tmp=$(mktemp -d)
    trap 'rm -rf "$tmp"' EXIT
    git -C "$tmp" init --quiet
    git -C "$tmp" remote add origin https://github.com/OSGeo/gdal.git
    git -C "$tmp" sparse-checkout init --cone
    git -C "$tmp" fetch --quiet --depth 1 origin "$gdal_revision"
    git -C "$tmp" checkout --quiet --detach FETCH_HEAD
    git -C "$tmp" sparse-checkout set autotest/ogr/data/shp autotest/ogr/data/filegdb autotest/ogr/data/openfilegdb
    mkdir -p "$tmp/corpus"
    cp -r "$tmp"/autotest/ogr/data/filegdb/* "$tmp"/autotest/ogr/data/openfilegdb/* "$tmp/corpus/"
    for z in "$tmp"/corpus/*.zip; do
        d="$tmp/corpus/_unz_$(basename "${z%.zip}")"
        mkdir -p "$d"
        unzip -q -o "$z" -d "$d"
        rm "$z"
    done
    # Do not replace a usable cache until download and extraction both succeeded.
    rm -rf "$dest/gdal-autotest" "$dest/filegdb"
    mv "$tmp/autotest/ogr/data/shp" "$dest/gdal-autotest"
    mv "$tmp/corpus" "$dest/filegdb"
    printf '%s\n' "$gdal_revision" > "$dest/gdal-revision"
    rm -rf "$tmp"
    trap - EXIT
fi

# Natural Earth admin-0 countries: real-world polygons with a .cpg.
if [ ! -d "$dest/natural-earth" ]; then
    mkdir -p "$dest/natural-earth"
    curl -fsSL -o "$dest/ne.zip" https://naciscdn.org/naturalearth/110m/cultural/ne_110m_admin_0_countries.zip
    (cd "$dest/natural-earth" && unzip -q ../ne.zip) && rm "$dest/ne.zip"
fi
echo "testdata ready in $dest"
