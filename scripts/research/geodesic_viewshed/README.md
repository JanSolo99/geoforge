# Geodesic Viewshed (Viewshed2) research

ArcGIS's "How Geodesic Viewshed works": one sight line per target cell centre, cut into
cell-size steps, ground by "a linear IDW using the nearest cell centers", geocentric 3D.

Probed (ArcGIS Pro 3.7, synthetic 41×41 rasters, 10 m cells, `out_agl_raster` for exact values):

- Non-visible cells are NoData: 255 (U8) for FREQUENCY, 0 for OBSERVERS (bit mask, input order).
- Observer elevation = mean of the 2×2 cells right/below the observer cell (+ offset): AGL
  on-axis behind a bump gave z0 = 1.875 with cells 3, 0, 0, 0.5 there. I.e. bilinear at
  (col + 0.5, row + 0.5) in cell-centre coordinates. The sight-line geometry itself is centred.
- A bump one cell from the observer never blocks; two cells away it does → steps within
  1.5 cells are skipped.
- On-axis steps sample cell centres exactly (AGL behind a bump is linear through it).
- Off-axis the kernel reaches ~1.3 cells but is not monotone in distance for the step
  placements tried (row-18 AGL in `p_agl3`); 7-nearest IDW (power 1) with steps L/round(L)
  fits best: 81 / 153 differing cells on `dem.tif` (hill / two observers). Tried: bilinear,
  TIN, 4/9/16-cell IDW (p 1–3), k-nearest 3–16, radial tents, fixed/backward/major-axis steps.

Scripts: `p7.py` (probe loader, IDW variants), `agl2.py` / `k.py` (AGL-based kernel search),
`real2.py` (scores on dem.tif). They expect probe arrays under /tmp/vs2 (regenerate with
Viewshed2 on `NumPyArrayToRaster` probes; see the docstrings/code).
