# Add Global IDs — public ArcGIS Pro 3.7 controls

Reference: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/add-global-ids.html

`probe.py` prepares `testdata/geoproc/globalids_base.gdb` once before any tools;
never regenerate this source. Eight controls cover features, tables, empty classes,
repeat, field-name collision, feature dataset and missing input. Seven succeed;
missing input fails000242. `probe_results.json` retains raw observations. Public
schema export recipe `probe_xml.py` writes to /tmp/unigis-globalids-probe/schema.xml.

ArcGIS adds a nonnullable required noneditable GlobalID field, length38, with
UUID4 values and FDO_GlobalID attribute index. An ordinary GlobalID field remains
and the new system field becomes GlobalID_1. Repeated calls preserve values/schema.
Feature datasets apply the operation to their child classes. Public exported XML
reports HasGlobalID=true and GlobalIDFieldName, alongside required/editable flags.

Eight `gids_*` manifest references add batch coverage (seven successes and one
expected refusal). Membership is deterministic; identifiers are random. The portable
contract compares exact field schema, existing attributes/geometries/OIDs, index
names/fields, UUID4 validity and uniqueness, and repeat stability. Random GUID bytes
are deliberately not expected to match an unrelated ArcGIS invocation. Public
reader checks supplement the native metadata validation.

Native CLI `add_global_ids in_datasets='path;path'` edits in place, with all inputs
in one existing FileGDB, including simple feature datasets. An optional host method
refuses unsupported hosts. A single journal covers the batch: invalid targets and
later write failures preserve originals. Existing GlobalID fields are left unchanged.
OS entropy supplies UUID4 values; source fields, field flags and surviving indexes
are retained. No GDAL runtime or proprietary SDK used.

Scope: cross-workspace batches, enterprise/mobile geodatabases and layers refused;
advanced dataset/controller metadata and exotic schemas unverified. This tool adds
IDs to current rows; automatic assignment by later native insert/copy operations,
replication/attachment/relationship workflows and protection by other edit tools
are not implemented or verified here. Feature-dataset controls use one empty child;
nonempty nested classes receive the same per-class operation. Validation results
and exact commands are recorded in docs/HANDOFF.md.
