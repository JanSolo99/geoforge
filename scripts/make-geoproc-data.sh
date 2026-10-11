#!/usr/bin/env bash
# Builds extra inputs in testdata/geoproc/ for the ArcGIS reference cases.
# (parcels, roads, wells and clip_areas predate this script.) Needs ogr2ogr.
set -euo pipefail
cd "$(dirname "$0")/../testdata/geoproc"

# streams: lines crossing the roads; stream 1 runs along road 3 between
# (500100 5000100) and (500200 5000200) (exactly collinear), stream 3 touches nothing.
cat > /tmp/gf-streams.csv <<'CSV'
WKT,STREAM_ID,NAME
"LINESTRING (500050 5000150,500100 5000100,500200 5000200,500250 5000150,500400 5000300,500450 5000500)",1,Mill Creek
"LINESTRING (499980 5000400,500600 5000420)",2,Cedar Brook
"LINESTRING (500600 5000600,500640 5000640)",3,Fox Run
CSV
rm -f streams.*
ogr2ogr -f "ESRI Shapefile" streams.shp /tmp/gf-streams.csv -oo GEOM_POSSIBLE_NAMES=WKT \
  -oo KEEP_GEOM_COLUMNS=NO -oo AUTODETECT_TYPE=YES -nlt LINESTRING -a_srs roads.prj -lco ENCODING=UTF-8
rm /tmp/gf-streams.csv
