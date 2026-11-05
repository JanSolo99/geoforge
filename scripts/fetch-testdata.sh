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
   [ ! -d "$dest/filegdb-raster" ] ||
   [ "$(cat "$dest/gdal-revision" 2>/dev/null || true)" != "$gdal_revision" ]; then
    tmp=$(mktemp -d)
    trap 'rm -rf "$tmp"' EXIT
    git -C "$tmp" init --quiet
    git -C "$tmp" remote add origin https://github.com/OSGeo/gdal.git
    git -C "$tmp" sparse-checkout init --cone
    git -C "$tmp" fetch --quiet --depth 1 --filter=blob:none origin "$gdal_revision"
    git -C "$tmp" checkout --quiet --detach FETCH_HEAD
    git -C "$tmp" sparse-checkout set autotest/ogr/data/shp autotest/ogr/data/filegdb autotest/ogr/data/openfilegdb \
        autotest/gdrivers/data/filegdb
    mkdir -p "$tmp/corpus"
    cp -r "$tmp"/autotest/ogr/data/filegdb/* "$tmp"/autotest/ogr/data/openfilegdb/* "$tmp/corpus/"
    for z in "$tmp"/corpus/*.zip; do
        d="$tmp/corpus/_unz_$(basename "${z%.zip}")"
        mkdir -p "$d"
        unzip -q -o "$z" -d "$d"
        rm "$z"
    done
    # Do not replace a usable cache until download and extraction both succeeded.
    # Raster datasets in file geodatabases (GDAL's OpenFileGDB raster tests).
    mkdir -p "$tmp/rasters"
    for f in "$tmp"/autotest/gdrivers/data/filegdb/*; do
        case "$f" in
            *.zip) unzip -q -o "$f" -d "$tmp/rasters" ;;
            *) cp -r "$f" "$tmp/rasters/" ;;
        esac
    done
    rm -rf "$dest/gdal-autotest" "$dest/filegdb" "$dest/filegdb-raster"
    mv "$tmp/autotest/ogr/data/shp" "$dest/gdal-autotest"
    mv "$tmp/corpus" "$dest/filegdb"
    mv "$tmp/rasters" "$dest/filegdb-raster"
    printf '%s\n' "$gdal_revision" > "$dest/gdal-revision"
    rm -rf "$tmp"
    trap - EXIT
fi

# Natural Earth admin-0 countries: real-world polygons with a .cpg.
if [ ! -d "$dest/natural-earth" ]; then
    # Build in a temporary folder so a failed download leaves nothing behind and the next run retries.
    ne=$(mktemp -d)
    trap 'rm -rf "$ne"' EXIT
    curl -fsSL -o "$ne/ne.zip" https://naciscdn.org/naturalearth/110m/cultural/ne_110m_admin_0_countries.zip
    mkdir "$ne/data"
    unzip -q "$ne/ne.zip" -d "$ne/data"
    mv "$ne/data" "$dest/natural-earth"
    rm -rf "$ne"
    trap - EXIT
fi
echo "testdata ready in $dest"
