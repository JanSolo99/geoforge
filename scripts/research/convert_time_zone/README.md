# Convert Time Zone (management.ConvertTimeZone), ArcGIS Pro 3.7

Implemented as `convert_time_zone` (crates/unigis-tools/src/timezone.rs) with chrono-tz.
ArcGIS lists 141 Windows time zone names (`arcgis_zones.json`); 139 map to IANA zones through
CLDR's windowsZones table (generated from Python tzlocal's `win_tz`); `Mid-Atlantic Standard
Time` fails in ArcGIS (ERROR 999999) and is refused, as is `Kamchatka Standard Time`.

Probed on a memory table (times kept): 8 zone pairs x 9 instants around the 2024 US DST
changes, 1999 and year end all equal IANA rules (unit test `conversions_match_arcgis`):
- Ambiguous local times (the repeated autumn hour) take the first (summer) reading; times in
  the spring gap take the offset after the change (02:30 Pacific -> 09:30 UTC).
- INPUT/OUTPUT_NOT_ADJUSTED_FOR_DST use the zone's standard offset all year.
- The output is a new Date field; nulls stay null. Reference cases `ctz_*` run in place on
  `dates.dbf` (dBASE dates, so only day shifts are visible there).
