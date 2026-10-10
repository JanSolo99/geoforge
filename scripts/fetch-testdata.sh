#!/usr/bin/env bash
# Downloads large / third-party test corpora into testdata/large (git-ignored).
set -euo pipefail
cd "$(dirname "$0")/.."
dest=testdata/large
mkdir -p "$dest"

# GDAL autotest shapefiles (MIT): all shape types, Z/M, MultiPatch, odd DBFs, code pages.
if [ ! -d "$dest/gdal-autotest" ]; then
    tmp=$(mktemp -d)
    git clone --quiet --depth 1 --filter=blob:none --sparse https://github.com/OSGeo/gdal.git "$tmp"
    git -C "$tmp" sparse-checkout set autotest/ogr/data/shp
    mv "$tmp/autotest/ogr/data/shp" "$dest/gdal-autotest"
    rm -rf "$tmp"
fi

# Natural Earth admin-0 countries: real-world polygons with a .cpg.
if [ ! -d "$dest/natural-earth" ]; then
    mkdir -p "$dest/natural-earth"
    curl -fsSL -o "$dest/ne.zip" https://naciscdn.org/naturalearth/110m/cultural/ne_110m_admin_0_countries.zip
    (cd "$dest/natural-earth" && unzip -q ../ne.zip) && rm "$dest/ne.zip"
fi
echo "testdata ready in $dest"
