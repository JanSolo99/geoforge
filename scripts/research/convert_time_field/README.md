# Convert Temporal Field (management.ConvertTimeField), ArcGIS Pro 3.7

Implemented as `convert_time_field` (crates/unigis-tools/src/temporal.rs); in-place reference
cases `ctf_*` on `testdata/geoproc/dates.dbf` and `ctf_text.dbf` (recipe `make_inputs.py`).

- Formats use .NET-style patterns: yyyy, yy, M/MM (numbers), MMM/MMMM (English names), d/dd,
  ddd/dddd (day names), H/HH, h/hh, m/mm, s/ss, f.., tt; other characters are literal.
- Date -> TEXT: a 254-character field (255 in a geodatabase); default format `yyyyMMddHHmmss`.
  Date -> LONG/SHORT/DOUBLE: the formatted text as a number (`yyyyMMdd` -> 20240229).
- Text/number -> DATE: lenient parsing (`2001011` as yyyyMMdd -> 2001-01-01, `1/1/2001` as
  MM/dd/yyyy); impossible dates, wrong text and short month names for MMMM give null.
- Any value that does not convert (nulls included) -> null with WARNING 002125; none converts ->
  ERROR 002126 (text input without a format fails that way).
- Gaps: `tt` gives lowercase am/pm here (locale?); numeric outputs of unconverted rows are blank
  (null) in ArcGIS's dBASE file while our writer stores numeric nulls as 0; time zones,
  DATEONLY/TIMEONLY/TIMESTAMP_OFFSET outputs and ArcGIS's other format keywords are not done.
