# Enable Editor Tracking

Public reference:
https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/enable-editor-tracking.html

Fourteen public ArcGIS Pro3.7 controls in probe.py cover all/partial roles,
ADD_FIELDS/NO_ADD_FIELDS, feature classes/tables, UTC/DATABASE_TIME, existing
custom fields/values, already-active roles, adding roles, attempted active-role/
time-zone switches, duplicate roles, missing fields and wrong types. Source
fixture enable_tracking_base.gdb was saved once before controls mutated disposable
data. Do not recreate it merely for context. Results: probe_results.json.

Existing rows stay unchanged. Missing user fields are nullable text255, missing
date fields nullable low-precision Date; new values are NULL. Existing aliases,
lengths and values remain. Selected fields become read-only. Tracking metadata
names the selected roles, EditorTrackingEnabled=true and the chosen time basis.
Active roles/time basis are retained when switches are requested; omitted roles
stay active. Additional roles can be enabled. Repeat and attempted switches
succeed unchanged. No current user/timestamps are backfilled by this tool.

No roles fails001218; missing fields without ADD_FIELDS fails001278 (also002941
in ArcGIS messages); wrong type001217; shared role field names001214. Native
reports the primary code, without reproducing repeated/secondary diagnostics.

catalog_format_results.json stores before/after saved catalog XML from
`ogrinfo -ro -q -sql 'SELECT Name,Definition FROM GDB_Items' <workspace.gdb>`.
Role names, EditorTrackingEnabled and IsTimeInUTC are set in the root; selected
GPFieldInfoEx entries gain Editable=false. Binary field flags also clear editable
mask0x04 (as established by the Disable Editor Tracking format controls).
Aliases/domains/defaults/required markers remain. Unrelated ArcGIS workspace-copy
extent/spatial-index catalog normalization is not reproduced. GDAL reads saved
file formats only; ArcGIS is the GP oracle. No runtime GDAL or proprietary SDK.

Native writer/editor_tracking.rs enables simple classes/tables in one journal.
All names/types are preflighted before writing. Missing fields are added in one
rewrite, using normal spatial-grid recalculation; unrelated field flags are
restored afterwards. If no fields need adding, only metadata/flag bytes change.
Canonical existing field spelling is retained. Native-created definitions get
missing role/root elements using their actual qualified closing tag. Three
safety tests cover preserved values/OIDs/NULL additions/unrelated files, readonly
flags, repeat/refusal/active-change byte identity and native-generated table XML.

Registry ENABLE and optional Context::enable_editor_tracking capability provide
CLI FileGDB mutation; other hosts explicitly refuse.14 etrack_* portable schema
references cover10 successes/4 refusals. Existing schema comparator checks roles,
time basis, XML AND physical editability, fields/rows/OIDs/geometry/CRS/domains,
index definitions and repeat schema. All previous cases are retained unchanged.
probe_native.py independently checks all ten successful native outputs with public
arcpy, including every field property and original values/geometry.
probe_native_root.py/native_root_results.json additionally verify a table created
by the native Create Table tool: newly inserted qualified-root properties are
recognized by public ArcGIS Describe and all four fields are read-only.

Scope/gaps: simple FileGDB class/table paths. Feature datasets, relationship classes,
attachments, advanced class extensions, enterprise/mobile data and layers/services
are unsupported or unverified. Conventional <class>__ATTACH tables are explicitly
refused; renamed/advanced attachment schemas are unverified. Native edit APIs do
not automatically populate tracking users/timestamps on later edits; that edit
engine behavior is separate from this metadata/configuration operation. Unusual
field casing, domains/defaults, high-precision Date, Z/M/curves and rare invalid
parameter combinations are unverified (high-precision date roles are refused).

Validation commands (outcomes in docs/HANDOFF.md):

    uv run -q --with 'mcp>=2' python scripts/make-arcgis-reference.py 'etrack_*'
    cargo test -j3 -p unigis-cli --test enable_tracking_safety --test arcgis_reference
    scripts/check
