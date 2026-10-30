# Cut Fill — deferred (2026-10-31, ArcGIS Pro 3.7)

Established (small probes): regions are 8-connected groups of cells with the same change class
(unchanged, cut = before > after, fill = before < after), numbered 1.. in scan order (row-major from
the top); NoData in either surface is NoData. Attribute table: Value, Count, AREA (count x cell
area), VOLUME ((before - after) x cell area x z_factor; cut positive, fill negative, 0 unchanged);
z_factor only scales VOLUME.

Not identified: the output pixel type. With identical content (one unchanged region, value 1):
1x7 and 1x8 give S8 (NoData -128); 2x2, 2x3, 2x4 give U4 (2x4 with NoData 0); 3x3 and 4x4 give S8;
1x63 and 1x64 U8; 8x16 U8; 1x127, 1x128, 1x200 U16; dem.tif (150x120, 59 regions) U16 (65535). It
depends on the grid's shape, not on the values, by a rule not found. The reference comparison checks
type and NoData exactly, so the tool is deferred. Also no `.vat.dbf` writer exists yet.
