# Unsplit Line and Dice probes

`probe.py` runs Unsplit Line on `testdata/dm/chain_split.shp` (Split Line At Vertices of `chain.shp`) with
several dissolve fields. Findings (implemented in `unsplit_line`):

- Only the dissolve fields are kept (none: just the `Id` field, 0).
- Within a group of equal dissolve values, lines join where exactly two ends meet; a point with three or
  more ends stays a split; a closed ring stays whole.
- Output order: lines that were not joined (input order), then the joined lines in the order of their
  first piece.

Dice (`management.Dice`, vertex limit at least 10), probed with `testdata/dm/dice_*.shp` (made with arcpy
in the bridge session; random polygons use a radial jitter):

- A feature within the limit is copied. A multipart line over the limit gives one feature per part.
- Lines: `n = ceil(segments / (limit - 1))` pieces of `ceil(segments / n)` segments (the last one shorter).
- Polygons: recursive median split of the extent (count of original vertices inside the cell, borders
  inclusive; cut at `values[len / 2]` of the cell's vertex coordinates; upper half first). Multipart
  polygons are cut part by part. The cut axis is the longer cell side; the first cell goes across x unless
  it is higher than wide by more than ~0.125. For cells of 11 to 18 vertices ArcGIS departs from the longer
  side in about a quarter of the cuts (31 of 41 cuts follow it); neither extents, vertex spread, standard
  deviation, alternation nor a global toggle explains the rest.
