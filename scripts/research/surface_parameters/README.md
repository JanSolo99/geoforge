# Surface Parameters (Spatial Analyst) — black-box findings, ArcGIS Pro 3.7

Probed 2026-11-07 on `testdata/raster/dem.tif` (150x120, 10 m, UTM 33N, F32, no NoData) and
the same heights on 1 m and 30 m cells (arrays saved from the bridge as `.npz` in a scratch
folder, not kept). Scripts here (`*.py`, run with
`uv run --with pyproj --with numpy python -I <script> scratch/ref.npz ...`) compare ArcGIS
arrays with candidate models.

Observed:
- A spatial reference is required (ERROR 000248 without one): the computation is geodesic.
- Each cell's 3x3 neighbours (default neighbourhood distance = cell size) are converted to
  ECEF (geodetic lon/lat of the cell centres from the CRS, ellipsoidal height = z), then to a
  local frame at the centre: z along the ellipsoid normal, x toward the east neighbour
  (projected onto the tangent plane), y = z × x. The grid-aligned x matters for the
  biquadratic fit (not rotation invariant): off the central meridian (x = 300000, UTM 33N)
  a true-east frame misses by 2.7e-4° in slope, the grid frame by 5e-6° (`h.py`, `i.py`).
  Aspect is then measured from true north (grid-relative aspect is off by the convergence).
- BIQUADRATIC: exact 9-term fit z = c1 x²y² + c2 x²y + c3 xy² + a x² + b y² + s xy + p x + q y + f.
- QUADRATIC: weighted least squares of z = a x² + b y² + s xy + p x + q y + f over all 9
  points, weight w = 1 / (1 + x² + y²)² (x, y in metres in the tangent plane; centre weight 1).
  Ratios 0.25028 at 30 m cells, ~0.2526 at 10 m, and the 1 m case pin the "+1 m²".
- With r = 2a, t = 2b, g = p² + q², w = 1 + g (Minár et al. 2020), all signs verified:
  slope = atan(√g) (degrees); aspect = atan2(−p, −q) clockwise from north, 0–360;
  profile = −(p²r + 2pqs + q²t) / (g w^1.5); tangential = −(q²r − 2pqs + p²t) / (g w^0.5);
  contour (plan) = −(q²r − 2pqs + p²t) / g^1.5; contour geodesic torsion =
  (pq(r − t) − (p² − q²)s) / (g w); mean = −((1 + q²)r − 2pqs + (1 + p²)t) / (2 w^1.5);
  Gaussian = (rt − s²) / w²; Casorati = √(2·mean² − Gaussian).
- Agreement: median relative error ~1e-7 (ArcGIS single-precision noise); biquadratic slope
  exceeds 5e-5° on 7 of 17464 cells.
- Output F32, NoData −FLT_MAX for an input without NoData (3.4e38 with); the one-cell border
  is NoData.
- NoData neighbours (`dem_nodata.tif`, `j.py`): QUADRATIC fits the points present (weighted
  LS, 2e-6° agreement); the cell is NoData when the east neighbour is missing (the frame
  needs it) or the fit is singular (a whole side missing). BIQUADRATIC: any missing
  neighbour gives NoData.
- Not probed: flat cells' directional curvatures (we write 0), larger/adaptive
  neighbourhoods, projected azimuths, equatorial aspect, masks.
- Lead: Surface Area Ratio's unidentified ARC_CHORD plane (KNOWN_GAPS) may be this
  weighted fit.
