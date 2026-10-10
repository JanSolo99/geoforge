#!/usr/bin/env bash
# Downloads large / third-party test corpora into testdata/large (git-ignored).
set -euo pipefail
cd "$(dirname "$0")/.."
dest=testdata/large
mkdir -p "$dest"

# GDAL autotest data (MIT):
#  - shapefiles: all shape types, Z/M, MultiPatch, odd DBFs, code pages
#  - File Geodatabases: all geometry/field types, 64-bit OIDs, curves, sparse tables
if [ ! -d "$dest/gdal-autotest" ] || [ ! -d "$dest/filegdb" ]; then
    tmp=$(mktemp -d)
    git clone --quiet --depth 1 --filter=blob:none --sparse https://github.com/OSGeo/gdal.git "$tmp"
    git -C "$tmp" sparse-checkout set autotest/ogr/data/shp autotest/ogr/data/filegdb autotest/ogr/data/openfilegdb
    rm -rf "$dest/gdal-autotest" "$dest/filegdb"
    mv "$tmp/autotest/ogr/data/shp" "$dest/gdal-autotest"
    mkdir -p "$dest/filegdb"
    cp -r "$tmp"/autotest/ogr/data/filegdb/* "$tmp"/autotest/ogr/data/openfilegdb/* "$dest/filegdb/"
    for z in "$dest"/filegdb/*.zip; do
        d="$dest/filegdb/_unz_$(basename "${z%.zip}")"
        mkdir -p "$d" && unzip -q -o "$z" -d "$d" && rm "$z"
    done
    rm -rf "$tmp"
fi

# Natural Earth admin-0 countries: real-world polygons with a .cpg.
if [ ! -d "$dest/natural-earth" ]; then
    mkdir -p "$dest/natural-earth"
    curl -fsSL -o "$dest/ne.zip" https://naciscdn.org/naturalearth/110m/cultural/ne_110m_admin_0_countries.zip
    (cd "$dest/natural-earth" && unzip -q ../ne.zip) && rm "$dest/ne.zip"
fi
echo "testdata ready in $dest"
