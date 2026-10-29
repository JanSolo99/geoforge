# Extend Line (edit.ExtendLine), ArcGIS Pro 3.7

Probed on synthetic UTM 33N lines (recipe `make_inputs.py`, fixture
`testdata/geoproc/extend_lines.shp`, reference cases `ext_{15,50,100}_feature`,
`ext_{15,25,50}_extension`).

- A dangling end (no other segment within the 0.001 m XY tolerance; its own end segment
  excluded) is extended along its end segment by appending/prepending a vertex. Every part of
  a multipart line has its ends extended.
- Targets: the first non-parallel crossing or touch of any line, the line itself included
  (a hook reaches its own first segment). Collinear lines are not targets (an end pointing
  straight at another line's end stays).
- Length: when editing a shapefile the extension must be shorter than the length (10 m gap
  with length 10 stays, 9.9999 extends). The same run on a memory copy extends exactly-length
  gaps; that inclusive variant is not modelled.
- Order: features in OID order against the lines as already extended (in a memory run a line
  reached a point on an earlier line's new extension; in reverse order it did not).
- EXTENSION: an end may also stop where its ray crosses another dangling end's potential
  extension: that end's ray up to the length (inclusive) or its own first line, whichever
  is nearer, and only if that end has not been extended yet. The nearest candidate wins
  (P stops at R's extension 5 m away, not at Q 10 m away). Only the processed end moves; the
  other end may later reach the new extension itself (U/V meet; C stays while F reaches C's
  potential extension at exactly C's 50 m).
