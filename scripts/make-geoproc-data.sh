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

# squares A and B sharing the edge x=500050, and lines running along their edges
# (Identity KEEP_RELATIONSHIPS: which polygon lies left and right of each piece).
cat > /tmp/gf-squares.csv <<'CSV'
WKT,SQ_ID,NAME
"POLYGON ((500000 5000000,500000 5000050,500050 5000050,500050 5000000,500000 5000000))",1,A
"POLYGON ((500050 5000000,500050 5000050,500100 5000050,500100 5000000,500050 5000000))",2,B
CSV
rm -f squares.*
ogr2ogr -f "ESRI Shapefile" squares.shp /tmp/gf-squares.csv -oo GEOM_POSSIBLE_NAMES=WKT \
  -oo KEEP_GEOM_COLUMNS=NO -oo AUTODETECT_TYPE=YES -nlt POLYGON -a_srs roads.prj -lco ENCODING=UTF-8
rm /tmp/gf-squares.csv
cat > /tmp/gf-edges.csv <<'CSV'
WKT,EDGE_ID,NAME
"LINESTRING (500050 5000002,500050 5000012)",1,shared edge north
"LINESTRING (500050 5000022,500050 5000014)",2,shared edge south
"LINESTRING (500000 5000010,500000 5000040)",3,outer edge
"LINESTRING (500025 5000025,500050 5000025,500050 5000035,500075 5000035)",4,in A then along edge
"LINESTRING (499980 5000010,499980 5000040)",5,outside
"LINESTRING (500040 5000045,500060 5000045)",6,across edge
CSV
rm -f edges.*
ogr2ogr -f "ESRI Shapefile" edges.shp /tmp/gf-edges.csv -oo GEOM_POSSIBLE_NAMES=WKT \
  -oo KEEP_GEOM_COLUMNS=NO -oo AUTODETECT_TYPE=YES -nlt LINESTRING -a_srs roads.prj -lco ENCODING=UTF-8
rm /tmp/gf-edges.csv
