# Rescale by Function (2026-10-30, ArcGIS Pro 3.7)

- `transformation_function` must be an arcpy `Tf...` object or ArcGIS's text form
  (`LINEAR 2 6 # # # #`, logistic functions with a 7th value); `TfLinear 2 6 ...` text is silently
  ignored and the default MSSMALL runs.
- Formulas and defaults: Esri "Formulas for the transformation functions" — all 13 matched at
  defaults on 0..8 (float32 level), with two corrections: MSSmall/MSLarge use the sample standard
  deviation (n - 1), and LogisticDecay is the LogisticGrowth expression with y intercept 99.
- Mapping: `from + (y - min y) * (to - from) / (max y - min y)` over the threshold interval;
  shape defaults follow the thresholds; outside cells get below/above (number or NoData), by
  default the value at the threshold.
- Cells are evaluated with the resolved parameters rounded to float32 (Logarithm's default shift
  188.0848388671875 gives 1.0000447 at the minimum), the range with full precision.
- Output Float32, NoData -3.4028235e38. Whether ArcGIS writes statistics varies between identical
  runs (rbf_linear_thresholds had them once, not in two reruns), so `rbf_` statistics presence is
  not compared (STATISTICS_OPTIONAL in arcgis_reference.rs).
