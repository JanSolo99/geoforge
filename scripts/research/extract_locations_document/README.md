# Extract Locations From Document — implemented (2026-10-30)

`arcpy.conversion.ExtractLocationsDocument(in_file, out_feature_class, in_template, ...)`: 40
parameters (11 coordinate notations on/off — DD lat/lon, DD x/y degrees, DD plain, DM lat/lon,
DM x/y minutes, DMS lat/lon, DMS x/y seconds, DMS separated, UTM, UPS N/S off, MGRS (+polar off);
comma decimal, prefer lon/lat, input coordinate system, custom locations, fuzzy match, max/ignore
first features; 5 date notations, max/ignore dates, date range, custom attributes, file link,
modified time, pre/post text length 254, std_coord_fmt DD, req_word_breaks).

First probe (a 5-line .txt, defaults; ArcGIS Pro 3.7), output fields:
Name, Pre_Text, Ext_Text, Ext_Type, Post_Text, Precision, Std_Coord, First_Date, Early_Date,
Late_Date, All_Dates, ExDateText, Filename, File_Type, Modified, Scanned.

| Ext_Text | Ext_Type | Precision | Std_Coord | point |
|---|---|---|---|---|
| `34.0522 N, 118.2437 W` | DD | 9 | `34.052200N 118.243700W` | (-118.2437, 34.0522) |
| `40 26 46 N 79 58 56 W` | DMS | 31 | `40.446111N 079.982222W` | (-79.982222, 40.446111) |
| `51.5074, -0.1278` | DD | 7 | `51.507400N 000.127800W` | (-0.1278, 51.5074) |
| `18SUJ2348306479` | MGRS | 1 | `38.889463N 077.035242W` | |
| `17N 630084 4833438` (after "UTM") | UTM | 1 | `43.642562N 079.387143W` | |

- `version 1.2.3` and `555-1234` were not taken. Name = file name; Filename = full path;
  File_Type `TXT`; Modified/Scanned are text `YYYY-MM-DD HH:MM:SS` (local).
- Pre_Text/Post_Text: the document text before/after the match (up to 254 characters), line
  breaks turned into two spaces; Pre_Text trimmed of the separating space.
- Dates are document-wide (same on every row): First_Date (first found), Early_Date, Late_Date,
  All_Dates (`2021-03-03, 2024-03-15`, sorted ISO), ExDateText (as written, in document order).
- Precision looks like metres of the written precision (4-decimal DD 9 m at 34° / 7 m at 51°?,
  whole-second DMS 31 m, 1 m MGRS/UTM) — formula not yet identified.

Next: probe each notation's grammar and edge cases, the precision formula, .docx/.pdf inputs.

## Rules established in the second round (implemented, `eld_*` references)

- Precision: DD min(lat step, lon step x cos(lat)) x 111319.49 m; DM/DMS min(lat step, lon step)
  x 111319.49 m (no cosine); rounded, at least 1. MGRS 10^(5 - digits/2); UTM 1.
- A match may not start right after a letter, digit, sign or decimal point (`-34.05 N, ...` and
  `a34.05N` are not taken); a hemisphere letter may precede (`N34.05`, `N 79`) or follow, and a
  period right after a final letter is part of the match (`79 58 56 W.`).
- Signed decimal pairs need decimals on both values; integers alone (`34, -118`) and `--`
  separators are not taken; `,` `;` `/` or spaces separate; a first value beyond 90 makes the
  pair longitude first. Trailing degree signs are not part of Ext_Text.
- With DM/DMS switched off the text is re-read with fewer parts (`40 26.767 N 79 58.933 W` gives the
  DD pair `N 79` / `58.933 W`).
- UTM needs a band letter (`17 630084 4833438` is not taken); `56S` is the northern band S.
- Dates: all of yyyy-mm-dd, yyyy/mm/dd, yyyymmdd, yymmdd, yyjjj, m/d/yyyy, mm/dd/yy, m-d-yyyy,
  `March 21, 2024`, `22 March 2024`, `Mar 23 2024`, `24 Mar 24`; not `26.03.2024`.
- Context: line breaks become spaces; 254 characters each side, right-trimmed.
