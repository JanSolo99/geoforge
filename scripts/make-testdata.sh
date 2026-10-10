#!/usr/bin/env bash
# Regenerates the small committed shapefiles in testdata/shp (one per shape type) with GDAL.
set -euo pipefail
cd "$(dirname "$0")/.."
out=testdata/shp
rm -rf "$out" && mkdir -p "$out"
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT

csv() { # name, then WKT rows
    local f="$tmp/$1.csv"; shift
    echo 'WKT,name,count,ratio,born,flag' > "$f"
    local i=0
    for wkt in "$@"; do
        case $i in
            0) echo "\"$wkt\",Zürich ✓,42,3.14159,2024-02-29,T" ;;
            1) echo "\"$wkt\",,,,," ;;
            *) echo "\"$wkt\",row $i,-$i,$i.5,1999-12-31,F" ;;
        esac >> "$f"
        i=$((i + 1))
    done
    cat > "${f%.csv}.csvt" <<<'WKT,String(40),Integer(9),Real(19.11),Date,Integer(Boolean)'
}

make() { # shp-name, -nlt type, WKT rows...
    local name=$1 nlt=$2; shift 2
    csv "$name" "$@"
    ogr2ogr -q -f "ESRI Shapefile" "$out/$name.shp" "$tmp/$name.csv" \
        -a_srs EPSG:4326 -nlt "$nlt" -lco ENCODING=UTF-8 -oo GEOM_POSSIBLE_NAMES=WKT -oo KEEP_GEOM_COLUMNS=NO
}

make point POINT 'POINT (1 2)' 'POINT (3 4)' 'POINT (-5 6.5)'
make multipoint MULTIPOINT 'MULTIPOINT ((1 2),(3 4))' 'MULTIPOINT ((0 0))' 'MULTIPOINT ((5 5),(6 6),(7 7))'
make polyline MULTILINESTRING 'LINESTRING (0 0,1 1,2 0)' 'MULTILINESTRING ((0 0,1 0),(2 2,3 3))' 'LINESTRING (10 10,20 20)'
make polygon MULTIPOLYGON \
    'POLYGON ((0 0,0 10,10 10,10 0,0 0),(2 2,4 2,4 4,2 4,2 2))' \
    'MULTIPOLYGON (((0 0,0 1,1 1,1 0,0 0)),((5 5,5 6,6 6,6 5,5 5)))' \
    'POLYGON ((20 20,20 30,30 30,20 20))'
make pointz POINTZM 'POINT ZM (1 2 3 4)' 'POINT ZM (5 6 7 8)' 'POINT ZM (0 0 0 0)'
make multipointz MULTIPOINTZM 'MULTIPOINT ZM ((1 2 3 4),(5 6 7 8))' 'MULTIPOINT ZM ((0 0 0 0))' 'MULTIPOINT ZM ((9 9 9 9))'
make polylinez MULTILINESTRINGZM 'LINESTRING ZM (0 0 1 10,1 1 2 20)' 'MULTILINESTRING ZM ((0 0 0 0,1 0 1 1),(2 2 2 2,3 3 3 3))' 'LINESTRING ZM (5 5 5 5,6 6 6 6)'
make polygonz MULTIPOLYGONZM 'POLYGON ZM ((0 0 1 1,0 10 2 2,10 10 3 3,10 0 4 4,0 0 1 1))' 'POLYGON ZM ((0 0 0 0,0 1 0 0,1 1 0 0,0 0 0 0))' 'POLYGON ZM ((5 5 5 5,5 6 5 5,6 6 5 5,5 5 5 5))'
make pointm POINTM 'POINT M (1 2 3)' 'POINT M (4 5 6)' 'POINT M (7 8 9)'
make multipointm MULTIPOINTM 'MULTIPOINT M ((1 2 3),(4 5 6))' 'MULTIPOINT M ((0 0 0))' 'MULTIPOINT M ((9 9 9))'
make polylinem MULTILINESTRINGM 'LINESTRING M (0 0 10,1 1 20)' 'MULTILINESTRING M ((0 0 0,1 0 1),(2 2 2,3 3 3))' 'LINESTRING M (5 5 5,6 6 6)'
make polygonm MULTIPOLYGONM 'POLYGON M ((0 0 1,0 10 2,10 10 3,10 0 4,0 0 1))' 'POLYGON M ((0 0 0,0 1 0,1 1 0,0 0 0))' 'POLYGON M ((5 5 5,5 6 5,6 6 5,5 5 5))'
make nullshapes POINT 'POINT (1 1)' '' 'POINT (2 2)'
csv multipatch 'TIN Z (((0 0 0,0 1 0,1 1 1,0 0 0)),((0 0 0,1 1 1,1 0 0,0 0 0)))' \
    'POLYHEDRALSURFACE Z (((0 0 0,0 1 0,1 1 0,1 0 0,0 0 0)),((0 0 0,0 0 1,0 1 1,0 1 0,0 0 0)))' \
    'TIN Z (((5 5 5,5 6 5,6 6 6,5 5 5)))'
ogr2ogr -q -f "ESRI Shapefile" "$out/multipatch.shp" "$tmp/multipatch.csv" -a_srs EPSG:4326 \
    -lco SHPT=MULTIPATCH -lco ENCODING=UTF-8 -oo GEOM_POSSIBLE_NAMES=WKT -oo KEEP_GEOM_COLUMNS=NO
du -sh "$out"
