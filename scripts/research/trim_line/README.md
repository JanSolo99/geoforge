# Trim Line (edit.TrimLine), ArcGIS Pro 3.7

Probed on in-place shapefile edits of synthetic UTM 33N lines (recipe `make_inputs.py`,
fixture `testdata/geoproc/trim_lines.shp`, cases `trim_{15,25,35,60}_{delete,keep}`).

- Each part is cut wherever a line meets it: crossings, touches of an end point on it
  (T junctions, shared end points), other parts of the same feature, and non-adjacent
  segments of the part itself (a hook trims its own tail).
- An end is free when no other line comes within the 0.001 m XY tolerance. The piece between
  a free end and the nearest cut is a dangle; dangles shorter than the length are removed.
- A part touching another line at one end, free at the other, with no cut in between, is
  removed whole when shorter (both modes; the 30 m chain stub B at 35 m).
- A part free at both ends whose pieces are all dangles (a short crossing such as V 10/30
  at 35 m, an X of 10 m arms, an isolated 20 m line at 25 m) is a short feature: deleted with
  DELETE_SHORT, kept untouched (not even trimmed) with KEEP_SHORT. This is per part: a
  multipart line loses only its short part.
- Trimming looks at the input lines: M is trimmed at S's touch point although S itself is
  removed.
- Exact ties: a dangle of exactly the length was kept when its cut is a computed crossing
  (10 m, length 10) but removed when the cut is an existing vertex; ArcGIS's rounding of the
  crossing point decides. The implementation keeps exact ties; the fixture avoids them.
