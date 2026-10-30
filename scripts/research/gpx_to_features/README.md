# GPX To Features (conversion.GPXtoFeatures), ArcGIS Pro 3.7 — probe notes

Probe input: `sample.gpx` (2 waypoints, 1 track with 2 segments, 1 route); output_type
POINTS (default) or TRACKS_AS_LINES (`TRACKS` is refused, ERROR 000800).

POINTS, shapefile output (`gpx_pts.shp`):
- Order: track points (track by track, segment by segment), then route points, then waypoints.
- Fields: Id (Integer 6, always 0 — the shapefile's default field), Name, Descript, Type, Comment,
  Symbol, DateTimeS (Text 254 each), Elevation (Double 19). No DateTime field in a shapefile
  (a memory/FileGDB output also has DateTime, a Date, and orders rows differently).
- Name/Descript of track and route points are the track's/route's (`Track A`, `first`;
  `Route R`), not the point's own; Type is TRKPT/RTEPT/WPT; Comment/Symbol from cmt/sym of
  waypoints; DateTimeS is the <time> text; missing text is ' ' (blank), missing ele 0.
- Geometry: PointZM (Z = ele or 0, M = NaN), WGS 1984.
TRACKS_AS_LINES (`gpx_trk.shp`): one single-part PolylineZM per <trk>, joining all segments;
fields Id, Name, Descript, Type (TRKPT); routes and waypoints are not written. The .prj combines
WGS 1984 with a WGS 1984 vertical coordinate system. `sticky.gpx` confirms that point desc/cmt/sym
carry forward within tracks and routes.
