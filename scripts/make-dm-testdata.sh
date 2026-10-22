#!/bin/sh
# Small inputs for the Data Management breadth tools (XY To Line, Bearing Distance To Line, ...).
set -e
cd "$(dirname "$0")/.."
d=testdata/dm
mkdir -p $d
cat > $d/segments.csv <<'EOF'
SEG_ID,LABEL,X1,Y1,X2,Y2,BEARING,DIST,D_KM,D_MI,D_NM,D_FT,D_USFT,D_YD,B_MIL,B_RAD,B_GRAD
1,alpha,500010,5000010,500110,5000110,45,141.42,0.14142,0.0878,0.0763,463.97,463.97,154.66,800,0.785398,50
2,beta,500200,5000200,500200,5000300,0,100,0.1,0.0621,0.054,328.084,328.083,109.361,0,0,0
3,gamma,500300,5000300,500250,5000300,270,50,0.05,0.0311,0.027,164.042,164.042,54.68,4800,4.712389,300
4,delta,500400.5,5000400.5,500400.5,5000400.5,90,0,0,0,0,0,0,0,1600,1.570796,100
5,eps,500500,5000100,500600,5000050,120.5,111.8,0.1118,0.0695,0.0604,366.8,366.8,122.27,856,2.103,133.9
EOF
rm -f $d/segments.dbf $d/segments.shp $d/segments.shx $d/segments.prj $d/segments.cpg
ogr2ogr -f GeoJSON $d/segments.geojson $d/segments.csv -oo AUTODETECT_TYPE=YES
ogr2ogr -f "ESRI Shapefile" $d/segments.dbf $d/segments.geojson
rm $d/segments.geojson $d/segments.prj

# A long table for Pivot Table and Transpose Fields.
cat > $d/sales.csv <<'EOF'
REGION,YEAR,PRODUCT,AMOUNT,UNITS
north,2020,tea,10.5,3
north,2020,coffee,20,4
north,2021,tea,11.25,5
south,2020,tea,7,2
south,2020,tea,3,1
south,2021,coffee,8.5,6
south,2021,cocoa,2,9
east,2022,tea,1.5,7
EOF
rm -f $d/sales.dbf $d/sales.shp $d/sales.shx $d/sales.prj $d/sales.cpg
ogr2ogr -f GeoJSON $d/sales.geojson $d/sales.csv -oo AUTODETECT_TYPE=YES
ogr2ogr -f "ESRI Shapefile" $d/sales.dbf $d/sales.geojson
rm $d/sales.geojson $d/sales.prj
