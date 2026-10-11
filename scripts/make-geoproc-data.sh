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

# Z/M copies: roads_zm (PolylineZM: Z = 100 + 10 × vertex number + ROAD_ID, M = distance
# along the road) and parcels_z (PolygonZ: Z = PARCEL_ID + 0.5 × vertex number).
python3 - <<'PY'
import json, math, subprocess
def features(name):
    out = subprocess.run(["ogr2ogr", "-f", "GeoJSON", "/vsistdout/", f"{name}.shp"],
                         capture_output=True, text=True, check=True).stdout
    return json.loads(out)["features"]
with open("/tmp/gf-roads-zm.csv", "w") as f:
    f.write("WKT,ROAD_ID,CLASS\n")
    for ft in features("roads"):
        p = ft["properties"]
        g = ft["geometry"]
        paths = [g["coordinates"]] if g["type"] == "LineString" else g["coordinates"]
        out, m = [], 0.0
        for path in paths:
            pts = []
            for i, (x, y) in enumerate(path):
                if i:
                    m += math.dist(path[i - 1], (x, y))
                pts.append(f"{x} {y} {100 + 10 * i + p['ROAD_ID']} {round(m, 3)}")
            out.append("(" + ",".join(pts) + ")")
        f.write(f"\"MULTILINESTRING ZM ({','.join(out)})\",{p['ROAD_ID']},{p['CLASS']}\n")
with open("/tmp/gf-parcels-z.csv", "w") as f:
    f.write("WKT,PARCEL_ID,NAME\n")
    for ft in features("parcels"):
        p = ft["properties"]
        g = ft["geometry"]
        rings = g["coordinates"] if g["type"] == "Polygon" else g["coordinates"][0]
        out = []
        for ring in rings:
            out.append("(" + ",".join(f"{x} {y} {p['PARCEL_ID'] + 0.5 * (i % (len(ring) - 1))}"
                                      for i, (x, y) in enumerate(ring)) + ")")
        f.write(f"\"POLYGON Z ({','.join(out)})\",{p['PARCEL_ID']},{p['NAME']}\n")
PY
rm -f roads_zm.* parcels_z.*
ogr2ogr -f "ESRI Shapefile" roads_zm.shp /tmp/gf-roads-zm.csv -oo GEOM_POSSIBLE_NAMES=WKT \
  -oo KEEP_GEOM_COLUMNS=NO -oo AUTODETECT_TYPE=YES -nlt MULTILINESTRINGZM -a_srs roads.prj -lco ENCODING=UTF-8
ogr2ogr -f "ESRI Shapefile" parcels_z.shp /tmp/gf-parcels-z.csv -oo GEOM_POSSIBLE_NAMES=WKT \
  -oo KEEP_GEOM_COLUMNS=NO -oo AUTODETECT_TYPE=YES -nlt POLYGONZ -a_srs roads.prj -lco ENCODING=UTF-8
rm /tmp/gf-roads-zm.csv /tmp/gf-parcels-z.csv

# Three-way meetings for Intersect POINT/LINE output with three inputs: tri_a/tri_b/tri_c
# are lines that all cross at (500050 5000010) (tri_c also crosses tri_a elsewhere);
# tri_p/tri_q/tri_r are squares whose outlines share x=500050 from y 10 to 30.
mk() { # name type csv-body
  printf 'WKT,TID,NAME\n%s\n' "$3" > /tmp/gf-$1.csv
  rm -f $1.*
  ogr2ogr -f "ESRI Shapefile" $1.shp /tmp/gf-$1.csv -oo GEOM_POSSIBLE_NAMES=WKT \
    -oo KEEP_GEOM_COLUMNS=NO -oo AUTODETECT_TYPE=YES -nlt $2 -a_srs roads.prj -lco ENCODING=UTF-8
  rm /tmp/gf-$1.csv
}
mk tri_a LINESTRING '"LINESTRING (500000 5000010,500100 5000010)",1,a1
"LINESTRING (500000 5000040,500100 5000040)",2,a2'
mk tri_b LINESTRING '"LINESTRING (500050 4999990,500050 5000060)",1,b1'
mk tri_c LINESTRING '"LINESTRING (500040 5000000,500060 5000020)",1,c1
"LINESTRING (500070 5000000,500070 5000050)",2,c2'
mk tri_p POLYGON '"POLYGON ((500000 5000000,500000 5000050,500050 5000050,500050 5000000,500000 5000000))",1,p1'
mk tri_q POLYGON '"POLYGON ((500050 5000000,500050 5000050,500100 5000050,500100 5000000,500050 5000000))",1,q1'
mk tri_r POLYGON '"POLYGON ((500050 5000010,500050 5000030,500090 5000030,500090 5000010,500050 5000010))",1,r1
"POLYGON ((500020 5000020,500020 5000060,500080 5000060,500080 5000020,500020 5000020))",2,r2'

# wells_wgs84: the wells in geographic WGS 84 (input for projecting back to UTM).
rm -f wells_wgs84.shp wells_wgs84.shx wells_wgs84.dbf wells_wgs84.prj wells_wgs84.cpg
ogr2ogr -f "ESRI Shapefile" wells_wgs84.shp wells.shp -t_srs EPSG:4326 -lco ENCODING=UTF-8
