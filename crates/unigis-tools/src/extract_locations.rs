//! Extract Locations From Document: the coordinates written in a text document, as points, with
//! the text around each and the dates the document mentions. Follows ArcGIS Pro 3.7 as probed
//! (`scripts/research/extract_locations_document/README.md`):
//!
//! - Latitude/longitude pairs in decimal degrees (DD), degrees and decimal minutes (DM) and
//!   degrees, minutes and seconds (DMS): hemisphere letters before or after each value (`34.05 N`,
//!   `N34.05`, `40 26 46 N`, `40°26'46"N`, packed `402646N` / `4026.767N`, `40:26:46N`), or a signed
//!   decimal pair (`34.0522, -118.2437`; read as longitude first when the first value is beyond 90).
//! - MGRS (`18SUJ2348306479`, spaced) — the south-west corner of the square — and UTM with a
//!   latitude band (`17N 630084 4833438`, `630084mE`).
//! - Precision (metres): for DD the smaller of the latitude step and the longitude step at that
//!   latitude (a degree is 111 319.49 m); for DM and DMS the smaller step, without the cosine; MGRS
//!   the square size; UTM 1.
//! - Dates (`2024-03-15`, `2024/03/15`, `20240316`, `240317`, `24077`, `3/18/2024`, `03/19/24`,
//!   `3-20-2024`, `March 21, 2024`, `22 March 2024`, `Mar 23 2024`, `24 Mar 24`), reported on every
//!   point: the first, earliest and latest, all of them (ISO, sorted, distinct) and as written.

use std::path::Path;

use chrono::{Datelike, Local, NaiveDate};
use unigis_core::{Error, Feature, Field, FieldType, Geometry, GeometryKind, Result, Value};

use crate::ToolOutput;
use crate::registry::{Context, Kind, Param, Runner, Tool};

/// Metres in a degree of the WGS84 equator.
const DEGREE: f64 = 111_319.490_793_273_57;
const CONTEXT: usize = 254;

fn err(msg: impl Into<String>) -> Error {
    Error::Unsupported(msg.into())
}

/// One coordinate found in the text.
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    /// Character range in the document.
    pub start: usize,
    pub end: usize,
    pub kind: &'static str,
    pub lon: f64,
    pub lat: f64,
    pub precision: i64,
}

/// One date found in the text, with its range.
#[derive(Debug, Clone, PartialEq)]
pub struct FoundDate {
    pub start: usize,
    pub end: usize,
    pub date: NaiveDate,
}

/// Which coordinate notations to look for.
#[derive(Debug, Clone, Copy)]
pub struct Notations {
    pub dd: bool,
    pub dm: bool,
    pub dms: bool,
    pub utm: bool,
    pub mgrs: bool,
}

impl Default for Notations {
    fn default() -> Self {
        Notations {
            dd: true,
            dm: true,
            dms: true,
            utm: true,
            mgrs: true,
        }
    }
}

/// Whether a match may start at `i`: not inside a word or a number (a sign or decimal point
/// before it belongs to that number).
fn word_start(t: &[char], i: usize) -> bool {
    i == 0 || !(t[i - 1].is_alphanumeric() || matches!(t[i - 1], '-' | '+' | '.'))
}

fn word_end(t: &[char], i: usize) -> bool {
    i >= t.len() || !t[i].is_alphanumeric()
}

fn digits(t: &[char], i: usize) -> usize {
    t[i..].iter().take_while(|c| c.is_ascii_digit()).count()
}

fn skip_spaces(t: &[char], mut i: usize) -> usize {
    while i < t.len() && t[i] == ' ' {
        i += 1;
    }
    i
}

fn number(t: &[char], from: usize, to: usize) -> f64 {
    t[from..to]
        .iter()
        .collect::<String>()
        .parse()
        .unwrap_or(f64::NAN)
}

/// A latitude or longitude as written.
#[derive(Debug, Clone, Copy)]
struct Component {
    end: usize,
    /// Where the last digit ends (trailing symbols are not part of the match).
    digits_end: usize,
    value: f64,
    /// 1 DD, 2 DM, 3 DMS.
    parts: u8,
    /// The value of one unit of the last digit, in degrees.
    step: f64,
    hemisphere: Option<char>,
    signed: bool,
    has_fraction: bool,
}

fn is_hemisphere(c: char) -> bool {
    matches!(c, 'N' | 'S' | 'E' | 'W')
}

fn is_degree(c: char) -> bool {
    matches!(c, '°' | 'º' | '\u{fffd}')
}

/// A number `d+(.d+)?` at `i`: (end, integer digits, fraction digits).
fn decimal(t: &[char], i: usize) -> Option<(usize, usize, usize)> {
    let n = digits(t, i);
    if n == 0 {
        return None;
    }
    let mut end = i + n;
    let mut frac = 0;
    if end + 1 < t.len() && t[end] == '.' && t[end + 1].is_ascii_digit() {
        frac = digits(t, end + 1);
        end += 1 + frac;
    }
    Some((end, n, frac))
}

/// A latitude or longitude at `start`, of at most `max_parts` parts (degrees, minutes, seconds:
/// with the DM or DMS notations off, `40 26.767 N` is not read as one value).
fn component(t: &[char], start: usize, max_parts: usize) -> Option<Component> {
    let mut i = start;
    let mut hemisphere = None;
    if i < t.len() && is_hemisphere(t[i]) {
        hemisphere = Some(t[i]);
        // `N34.05` or `N 34.05`.
        i = skip_spaces(t, i + 1);
    }
    let mut signed = false;
    let mut negative = false;
    if hemisphere.is_none() && i < t.len() && (t[i] == '-' || t[i] == '+') {
        signed = true;
        negative = t[i] == '-';
        i += 1;
    }
    let first = i;
    let (mut end, int_digits, frac) = decimal(t, i)?;
    let mut values = vec![(first, end, int_digits, frac)];
    // Further parts: degrees, minutes, seconds separated by spaces, symbols or colons.
    let mut digits_end = end;
    while values.len() < max_parts && values.last().is_some_and(|v| v.3 == 0) {
        let mut j = end;
        let mut sym = false;
        if j < t.len() && (is_degree(t[j]) || matches!(t[j], '\'' | '′' | ':')) {
            j += 1;
            sym = true;
        }
        let k = skip_spaces(t, j);
        if (k == j && !sym) || k >= t.len() || !t[k].is_ascii_digit() {
            break;
        }
        let Some((e, n, f)) = decimal(t, k) else {
            break;
        };
        if n > 2 {
            break;
        }
        values.push((k, e, n, f));
        end = e;
        digits_end = e;
    }
    // Trailing symbol of the last part (°, ', ").
    let mut after = end;
    if after < t.len() && (is_degree(t[after]) || matches!(t[after], '\'' | '′' | '"' | '″')) {
        after += 1;
    }
    let (value, parts, step) = match values.as_slice() {
        [(a, b, n, f)] => {
            let whole = number(t, *a, *b);
            // Packed: DDMM(.m), DDDMM(.m), DDMMSS(.s), DDDMMSS(.s).
            match n {
                4 | 5 if *f > 0 && max_parts >= 2 => {
                    let d = (whole / 100.0).floor();
                    let m = whole - d * 100.0;
                    if m >= 60.0 {
                        return None;
                    }
                    (d + m / 60.0, 2, 10f64.powi(-(*f as i32)) / 60.0)
                }
                6 | 7 if max_parts >= 3 => {
                    let d = (whole / 10_000.0).floor();
                    let m = ((whole - d * 10_000.0) / 100.0).floor();
                    let s = whole - d * 10_000.0 - m * 100.0;
                    if m >= 60.0 || s >= 60.0 {
                        return None;
                    }
                    (
                        d + m / 60.0 + s / 3600.0,
                        3,
                        10f64.powi(-(*f as i32)) / 3600.0,
                    )
                }
                1..=3 => (whole, 1, 10f64.powi(-(*f as i32))),
                _ => return None,
            }
        }
        [(a, b, n, _), (c, d, _, f)] => {
            if *n > 3 {
                return None;
            }
            let m = number(t, *c, *d);
            if m >= 60.0 {
                return None;
            }
            (
                number(t, *a, *b) + m / 60.0,
                2,
                10f64.powi(-(*f as i32)) / 60.0,
            )
        }
        [(a, b, n, _), (c, d, _, _), (e, g, _, f)] => {
            if *n > 3 {
                return None;
            }
            let (m, s) = (number(t, *c, *d), number(t, *e, *g));
            if m >= 60.0 || s >= 60.0 {
                return None;
            }
            (
                number(t, *a, *b) + m / 60.0 + s / 3600.0,
                3,
                10f64.powi(-(*f as i32)) / 3600.0,
            )
        }
        _ => return None,
    };
    let has_fraction = values.last().is_some_and(|v| v.3 > 0);
    let mut end = after;
    if hemisphere.is_none() {
        let k = skip_spaces(t, after);
        if k < t.len() && is_hemisphere(t[k]) && (k + 1 >= t.len() || !t[k + 1].is_alphabetic()) {
            hemisphere = Some(t[k]);
            end = k + 1;
            // A period after the letter is taken with it (`79 58 56 W.`).
            if end < t.len() && t[end] == '.' {
                end += 1;
            }
            digits_end = end;
        }
    }
    Some(Component {
        end,
        digits_end,
        value: if negative { -value } else { value },
        parts,
        step,
        hemisphere,
        signed,
        has_fraction,
    })
}

fn rounded_precision(metres: f64) -> i64 {
    (metres.round() as i64).max(1)
}

/// A latitude/longitude pair starting at `i`.
fn lat_lon(t: &[char], i: usize, n: &Notations) -> Option<Found> {
    let max_parts = if n.dms {
        3
    } else if n.dm {
        2
    } else {
        1
    };
    let a = component(t, i, max_parts)?;
    let mut j = skip_spaces(t, a.end);
    if j < t.len() && matches!(t[j], ',' | ';' | '/') {
        j = skip_spaces(t, j + 1);
    } else if j == a.end {
        // Without a separator the two must still be apart, unless a hemisphere letter ends the
        // first (`34.05N118.24W`).
        a.hemisphere?;
    }
    let b = component(t, j, max_parts)?;
    let (lat, lon, end);
    match (a.hemisphere, b.hemisphere) {
        (Some(ha), Some(hb)) => {
            if a.signed || b.signed || a.parts != b.parts {
                return None;
            }
            let (la, lo, hl, ho) = match (ha, hb) {
                ('N' | 'S', 'E' | 'W') => (a, b, ha, hb),
                ('E' | 'W', 'N' | 'S') => (b, a, hb, ha),
                _ => return None,
            };
            lat = if hl == 'S' { -la.value } else { la.value };
            lon = if ho == 'W' { -lo.value } else { lo.value };
            end = b.end;
            if !word_end(t, end) {
                return None;
            }
            let kind = ["", "DD", "DM", "DMS"][a.parts as usize];
            let enabled = match a.parts {
                1 => n.dd,
                2 => n.dm,
                _ => n.dms,
            };
            if !enabled || lat.abs() > 90.0 || lon.abs() > 180.0 {
                return None;
            }
            let metres = if a.parts == 1 {
                (la.step * DEGREE).min(lo.step * DEGREE * lat.to_radians().cos())
            } else {
                la.step.min(lo.step) * DEGREE
            };
            Some(Found {
                start: i,
                end,
                kind,
                lon,
                lat,
                precision: rounded_precision(metres),
            })
        }
        (None, None) => {
            if !n.dd || a.parts != 1 || b.parts != 1 || !a.has_fraction || !b.has_fraction {
                return None;
            }
            end = b.digits_end;
            if !word_end(t, end) {
                return None;
            }
            let (la, lo) = if a.value.abs() > 90.0 { (b, a) } else { (a, b) };
            lat = la.value;
            lon = lo.value;
            if lat.abs() > 90.0 || lon.abs() > 180.0 {
                return None;
            }
            let metres = (la.step * DEGREE).min(lo.step * DEGREE * lat.to_radians().cos());
            Some(Found {
                start: i,
                end,
                kind: "DD",
                lon,
                lat,
                precision: rounded_precision(metres),
            })
        }
        _ => None,
    }
}

const BANDS: &str = "CDEFGHJKLMNPQRSTUVWX";

/// Zone (1–60) and latitude band at `i`: (end, zone text).
fn zone_band(t: &[char], i: usize) -> Option<(usize, String)> {
    let n = digits(t, i);
    if !(1..=2).contains(&n) || i + n >= t.len() {
        return None;
    }
    let zone: u32 = t[i..i + n].iter().collect::<String>().parse().ok()?;
    let band = t[i + n].to_ascii_uppercase();
    if !(1..=60).contains(&zone) || !BANDS.contains(band) {
        return None;
    }
    Some((i + n + 1, format!("{zone}{band}")))
}

fn mgrs(t: &[char], i: usize) -> Option<Found> {
    let (mut j, zb) = zone_band(t, i)?;
    j = skip_spaces(t, j);
    if j + 2 > t.len() || !t[j].is_ascii_alphabetic() || !t[j + 1].is_ascii_alphabetic() {
        return None;
    }
    let square: String = t[j..j + 2].iter().map(|c| c.to_ascii_uppercase()).collect();
    j += 2;
    let k = skip_spaces(t, j);
    let mut numbers = String::new();
    let mut end = j;
    let first = digits(t, k);
    if first > 0 {
        numbers.extend(&t[k..k + first]);
        end = k + first;
        // Spaced easting and northing of equal length.
        let m = skip_spaces(t, end);
        let second = if m > end { digits(t, m) } else { 0 };
        if second == first && first <= 5 {
            numbers.extend(&t[m..m + second]);
            end = m + second;
        }
    }
    if !numbers.len().is_multiple_of(2) || numbers.len() > 10 || !word_end(t, end) {
        return None;
    }
    let (lon, lat) =
        crate::grid_notation::parse_grid_notation("MGRS", &format!("{zb}{square}{numbers}"))?;
    let half = numbers.len() as i32 / 2;
    Some(Found {
        start: i,
        end,
        kind: "MGRS",
        lon,
        lat,
        precision: 10_i64.pow((5 - half) as u32),
    })
}

/// A UTM easting or northing: `n` digits, optional `m`, optional `E`/`N` letter.
fn utm_value(t: &[char], i: usize, n: usize, letter: char) -> Option<(usize, String)> {
    if digits(t, i) != n {
        return None;
    }
    let value: String = t[i..i + n].iter().collect();
    let mut end = i + n;
    if end < t.len() && t[end] == 'm' {
        end += 1;
    }
    let k = skip_spaces(t, end);
    if k < t.len() && t[k] == letter && word_end(t, k + 1) {
        end = k + 1;
    }
    Some((end, value))
}

fn utm(t: &[char], i: usize) -> Option<Found> {
    let (j, zb) = zone_band(t, i)?;
    let k = skip_spaces(t, j);
    if k == j {
        return None;
    }
    let (e_end, easting) = utm_value(t, k, 6, 'E')?;
    let m = skip_spaces(t, e_end);
    if m == e_end {
        return None;
    }
    let (end, northing) = utm_value(t, m, 7, 'N')?;
    if !word_end(t, end) {
        return None;
    }
    let (lon, lat) = crate::grid_notation::parse_grid_notation(
        "UTM_BANDS",
        &format!("{zb}{easting}{northing}"),
    )?;
    Some(Found {
        start: i,
        end,
        kind: "UTM",
        lon,
        lat,
        precision: 1,
    })
}

/// The coordinates in `t`, in document order.
pub fn find_coordinates(t: &[char], n: &Notations) -> Vec<Found> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < t.len() {
        let c = t[i];
        let candidate = c.is_ascii_digit() || is_hemisphere(c) || c == '-' || c == '+';
        if !candidate || !word_start(t, i) {
            i += 1;
            continue;
        }
        let found = (n.mgrs.then(|| mgrs(t, i)).flatten())
            .or_else(|| n.utm.then(|| utm(t, i)).flatten())
            .or_else(|| lat_lon(t, i, n));
        match found {
            Some(f) => {
                i = f.end;
                out.push(f);
            }
            None => i += 1,
        }
    }
    out
}

const MONTHS: [&str; 12] = [
    "january",
    "february",
    "march",
    "april",
    "may",
    "june",
    "july",
    "august",
    "september",
    "october",
    "november",
    "december",
];

/// A month name or its three-letter abbreviation at `i`: (end, month 1–12).
fn month_name(t: &[char], i: usize) -> Option<(usize, u32)> {
    let n = t[i..].iter().take_while(|c| c.is_alphabetic()).count();
    if n < 3 {
        return None;
    }
    let word: String = t[i..i + n].iter().collect::<String>().to_lowercase();
    let m = MONTHS
        .iter()
        .position(|name| *name == word || (n == 3 && name.starts_with(&word)))?;
    Some((i + n, m as u32 + 1))
}

fn year(text: &str) -> Option<i32> {
    let y: i32 = text.parse().ok()?;
    Some(if text.len() == 2 { 2000 + y } else { y })
}

fn digits_text(t: &[char], i: usize, n: usize) -> String {
    t[i..i + n].iter().collect()
}

/// A date at `i`: (end, date).
fn date_at(t: &[char], i: usize) -> Option<(usize, NaiveDate)> {
    // Month name first: `March 21, 2024`, `Mar 23 2024`.
    if let Some((j, month)) = month_name(t, i) {
        let mut k = skip_spaces(t, j);
        if k == j {
            return None;
        }
        let dn = digits(t, k);
        if !(1..=2).contains(&dn) {
            return None;
        }
        let day: u32 = digits_text(t, k, dn).parse().ok()?;
        k += dn;
        if k < t.len() && t[k] == ',' {
            k += 1;
        }
        let y0 = skip_spaces(t, k);
        if y0 == k {
            return None;
        }
        let yn = digits(t, y0);
        if yn != 4 && yn != 2 {
            return None;
        }
        let end = y0 + yn;
        return word_end(t, end)
            .then(|| NaiveDate::from_ymd_opt(year(&digits_text(t, y0, yn))?, month, day))
            .flatten()
            .map(|d| (end, d));
    }
    let n = digits(t, i);
    if n == 0 {
        return None;
    }
    let after = i + n;
    // Day before a month name: `22 March 2024`, `24 Mar 24`.
    if n <= 2 {
        let j = skip_spaces(t, after);
        if j > after
            && let Some((k, month)) = month_name(t, j)
        {
            let y0 = skip_spaces(t, k);
            let yn = digits(t, y0);
            if y0 > k && (yn == 4 || yn == 2) && word_end(t, y0 + yn) {
                let day: u32 = digits_text(t, i, n).parse().ok()?;
                let d = NaiveDate::from_ymd_opt(year(&digits_text(t, y0, yn))?, month, day)?;
                return Some((y0 + yn, d));
            }
        }
    }
    // Separated numbers: `2024-03-15`, `2024/03/25`, `3/18/2024`, `03/19/24`, `3-20-2024`.
    if after < t.len() && matches!(t[after], '-' | '/') {
        let sep = t[after];
        let b0 = after + 1;
        let bn = digits(t, b0);
        if (1..=2).contains(&bn) && b0 + bn < t.len() && t[b0 + bn] == sep {
            let c0 = b0 + bn + 1;
            let cn = digits(t, c0);
            let end = c0 + cn;
            if !word_end(t, end) {
                return None;
            }
            let (a, b, c) = (
                digits_text(t, i, n),
                digits_text(t, b0, bn),
                digits_text(t, c0, cn),
            );
            return if n == 4 && (1..=2).contains(&cn) {
                NaiveDate::from_ymd_opt(a.parse().ok()?, b.parse().ok()?, c.parse().ok()?)
            } else if n <= 2 && (cn == 4 || cn == 2) {
                NaiveDate::from_ymd_opt(year(&c)?, a.parse().ok()?, b.parse().ok()?)
            } else {
                None
            }
            .map(|d| (end, d));
        }
        return None;
    }
    if !word_end(t, after) {
        return None;
    }
    let s = digits_text(t, i, n);
    let num = |a: usize, b: usize| s[a..b].parse::<u32>().ok();
    let d = match n {
        8 => NaiveDate::from_ymd_opt(s[..4].parse().ok()?, num(4, 6)?, num(6, 8)?),
        6 => NaiveDate::from_ymd_opt(year(&s[..2])?, num(2, 4)?, num(4, 6)?),
        5 => NaiveDate::from_yo_opt(year(&s[..2])?, num(2, 5)?),
        _ => None,
    }?;
    Some((after, d))
}

/// The dates in `t`, in document order.
pub fn find_dates(t: &[char]) -> Vec<FoundDate> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < t.len() {
        if !t[i].is_alphanumeric() || !word_start(t, i) {
            i += 1;
            continue;
        }
        match date_at(t, i) {
            Some((end, date)) => {
                out.push(FoundDate {
                    start: i,
                    end,
                    date,
                });
                i = end;
            }
            None => {
                // On to the next word.
                while i < t.len() && t[i].is_alphanumeric() {
                    i += 1;
                }
            }
        }
    }
    out
}

fn clip(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// `0.052200N`-style text: two (latitude) or three (longitude) integer digits, six decimals.
fn standard(lat: f64, lon: f64) -> String {
    format!(
        "{:09.6}{} {:010.6}{}",
        lat.abs(),
        if lat < 0.0 { 'S' } else { 'N' },
        lon.abs(),
        if lon < 0.0 { 'W' } else { 'E' }
    )
}

fn fields() -> Vec<Field> {
    let text = |name: &str, length: u32| {
        let mut f = Field::new(name, FieldType::String);
        f.length = length;
        f
    };
    let mut precision = Field::new("Precision", FieldType::Integer);
    precision.precision = 10;
    vec![
        text("Name", 50),
        text("Pre_Text", 254),
        text("Ext_Text", 120),
        text("Ext_Type", 50),
        text("Post_Text", 254),
        precision,
        text("Std_Coord", 30),
        Field::new("First_Date", FieldType::Date),
        Field::new("Early_Date", FieldType::Date),
        Field::new("Late_Date", FieldType::Date),
        text("All_Dates", 254),
        text("ExDateText", 254),
        text("Filename", 254),
        text("File_Type", 10),
        text("Modified", 20),
        text("Scanned", 20),
    ]
}

/// Where the text came from, for the Name, Filename, File_Type and Modified fields.
pub struct Source {
    pub name: String,
    pub filename: String,
    pub file_type: String,
    pub modified: String,
}

impl Source {
    /// A document on disk.
    pub fn file(path: &Path, modified: &str) -> Source {
        Source {
            name: path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            filename: path.display().to_string(),
            file_type: path
                .extension()
                .map(|e| e.to_string_lossy().to_uppercase())
                .unwrap_or_default(),
            modified: modified.to_owned(),
        }
    }

    /// Text given to Extract Locations From Text: named `Text`, no file (ArcGIS writes a space),
    /// modified when scanned.
    pub fn text(scanned: &str) -> Source {
        Source {
            name: "Text".into(),
            filename: " ".into(),
            file_type: " ".into(),
            modified: scanned.to_owned(),
        }
    }
}

/// The points of a document's text.
pub fn extract(text: &str, source: &Source, scanned: &str, notations: &Notations) -> ToolOutput {
    let Source {
        name,
        filename,
        file_type,
        modified,
    } = source;
    // Line breaks read as spaces.
    let t: Vec<char> = text
        .chars()
        .map(|c| if c == '\r' || c == '\n' { ' ' } else { c })
        .collect();
    let dates = find_dates(&t);
    let date = |d: Option<NaiveDate>| {
        d.and_then(|d| d.and_hms_opt(0, 0, 0))
            .map_or(Value::Null, Value::Date)
    };
    let mut sorted: Vec<NaiveDate> = dates.iter().map(|d| d.date).collect();
    sorted.sort();
    sorted.dedup();
    let all = sorted
        .iter()
        .map(|d| format!("{:04}-{:02}-{:02}", d.year(), d.month(), d.day()))
        .collect::<Vec<_>>()
        .join(", ");
    let written = dates
        .iter()
        .map(|d| t[d.start..d.end].iter().collect::<String>())
        .collect::<Vec<_>>()
        .join(", ");
    let features = find_coordinates(&t, notations)
        .into_iter()
        .enumerate()
        .map(|(k, f)| {
            let pre: String = t[f.start.saturating_sub(CONTEXT)..f.start].iter().collect();
            let post: String = t[f.end..(f.end + CONTEXT).min(t.len())].iter().collect();
            let ext: String = t[f.start..f.end].iter().collect();
            Feature {
                oid: k as i64 + 1,
                geometry: Some(Geometry::point(f.lon, f.lat)),
                attributes: vec![
                    Value::String(clip(name, 50)),
                    Value::String(pre.trim_end().to_owned()),
                    Value::String(clip(&ext, 120)),
                    Value::String(f.kind.to_owned()),
                    Value::String(post.trim_end().to_owned()),
                    Value::Integer(f.precision as i32),
                    Value::String(standard(f.lat, f.lon)),
                    date(dates.first().map(|d| d.date)),
                    date(sorted.first().copied()),
                    date(sorted.last().copied()),
                    Value::String(clip(&all, 254)),
                    Value::String(clip(&written, 254)),
                    Value::String(clip(filename, 254)),
                    Value::String(clip(file_type, 10)),
                    Value::String(modified.to_owned()),
                    Value::String(scanned.to_owned()),
                ],
            }
        })
        .collect();
    ToolOutput {
        kind: GeometryKind::Point,
        has_z: false,
        has_m: false,
        fields: fields(),
        spatial_reference_wkt: Some(WGS84.to_owned()),
        features,
        messages: Vec::new(),
    }
}

const WGS84: &str = "GEOGCS[\"GCS_WGS_1984\",DATUM[\"D_WGS_1984\",SPHEROID[\"WGS_1984\",6378137.0,298.257223563]],PRIMEM[\"Greenwich\",0.0],UNIT[\"Degree\",0.0174532925199433]]";

fn flag(ctx: &dyn Context, name: &str, yes: &str) -> bool {
    ctx.get(name)
        .is_none_or(|v| v.eq_ignore_ascii_case(yes) || v.eq_ignore_ascii_case("true"))
}

fn run(ctx: &dyn Context) -> Result<ToolOutput> {
    let file = ctx
        .get("in_file")
        .ok_or_else(|| err("ERROR 000735: Input File: Value is required"))?;
    let path = Path::new(&file);
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if !matches!(ext.as_str(), "txt" | "csv" | "text") {
        return Err(err(format!(
            "{file}: only plain-text documents (.txt, .csv) are read so far"
        )));
    }
    let bytes = std::fs::read(path).map_err(|e| Error::io(path, e))?;
    let text = String::from_utf8_lossy(&bytes);
    let stamp = |t: chrono::DateTime<Local>| t.format("%Y-%m-%d %H:%M:%S").to_string();
    let modified = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .map(|m| stamp(m.into()))
        .unwrap_or_default();
    Ok(extract(
        &text,
        &Source::file(path, &modified),
        &stamp(Local::now()),
        &notations(ctx),
    ))
}

fn notations(ctx: &dyn Context) -> Notations {
    Notations {
        dd: flag(ctx, "coord_dd_latlon", "FIND_DD_LATLON")
            || flag(ctx, "coord_dd_xyplain", "FIND_DD_XYPLAIN"),
        dm: flag(ctx, "coord_dm_latlon", "FIND_DM_LATLON"),
        dms: flag(ctx, "coord_dms_latlon", "FIND_DMS_LATLON"),
        utm: flag(ctx, "coord_utm", "FIND_UTM_MAINWORLD"),
        mgrs: flag(ctx, "coord_mgrs", "FIND_MGRS_MAINWORLD"),
    }
}

fn run_text(ctx: &dyn Context) -> Result<ToolOutput> {
    let text = ctx
        .get("in_text")
        .ok_or_else(|| err("ERROR 000735: Input Text: Value is required"))?;
    let scanned = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    Ok(extract(
        &text,
        &Source::text(&scanned),
        &scanned,
        &notations(ctx),
    ))
}

const fn choice(
    name: &'static str,
    label: &'static str,
    options: &'static [&'static str],
) -> Param {
    Param {
        name,
        label,
        kind: Kind::Choice(options),
        required: false,
        default: options[0],
        help: "",
    }
}

pub const TOOL: Tool = Tool {
    id: "extract_locations_document",
    name: "Extract Locations From Document",
    toolbox: "Conversion › From Document",
    summary: "Points for the coordinates written in a text document (DD, DM, DMS, UTM, MGRS), with the text around them and the dates the document mentions (ArcGIS Pro Extract Locations From Document).",
    params: &[
        Param {
            name: "in_file",
            label: "Input File",
            kind: Kind::Path,
            required: true,
            default: "",
            help: "A plain-text document (.txt).",
        },
        choice(
            "coord_dd_latlon",
            "Decimal degrees (lat/lon)",
            &["FIND_DD_LATLON", "DONT_FIND_DD_LATLON"],
        ),
        choice(
            "coord_dd_xyplain",
            "Decimal degrees (signed pair)",
            &["FIND_DD_XYPLAIN", "DONT_FIND_DD_XYPLAIN"],
        ),
        choice(
            "coord_dm_latlon",
            "Degrees decimal minutes",
            &["FIND_DM_LATLON", "DONT_FIND_DM_LATLON"],
        ),
        choice(
            "coord_dms_latlon",
            "Degrees minutes seconds",
            &["FIND_DMS_LATLON", "DONT_FIND_DMS_LATLON"],
        ),
        choice(
            "coord_utm",
            "UTM",
            &["FIND_UTM_MAINWORLD", "DONT_FIND_UTM_MAINWORLD"],
        ),
        choice(
            "coord_mgrs",
            "MGRS",
            &["FIND_MGRS_MAINWORLD", "DONT_FIND_MGRS_MAINWORLD"],
        ),
    ],
    output_suffix: "Locations",
    measures: false,
    run: Runner::Features(run),
};

/// `arcpy.conversion.ExtractLocationsText(in_text, out_feature_class, ...)`.
pub const TEXT_TOOL: Tool = Tool {
    id: "extract_locations_text",
    name: "Extract Locations From Text",
    toolbox: "Conversion › From Document",
    summary: "Points for the coordinates written in a text (DD, DM, DMS, UTM, MGRS), with the text around them and the dates it mentions (ArcGIS Pro Extract Locations From Text).",
    params: &[
        Param {
            name: "in_text",
            label: "Input Text",
            kind: Kind::Text,
            required: true,
            default: "",
            help: "The text to search.",
        },
        choice(
            "coord_dd_latlon",
            "Decimal degrees (lat/lon)",
            &["FIND_DD_LATLON", "DONT_FIND_DD_LATLON"],
        ),
        choice(
            "coord_dd_xyplain",
            "Decimal degrees (signed pair)",
            &["FIND_DD_XYPLAIN", "DONT_FIND_DD_XYPLAIN"],
        ),
        choice(
            "coord_dm_latlon",
            "Degrees decimal minutes",
            &["FIND_DM_LATLON", "DONT_FIND_DM_LATLON"],
        ),
        choice(
            "coord_dms_latlon",
            "Degrees minutes seconds",
            &["FIND_DMS_LATLON", "DONT_FIND_DMS_LATLON"],
        ),
        choice(
            "coord_utm",
            "UTM",
            &["FIND_UTM_MAINWORLD", "DONT_FIND_UTM_MAINWORLD"],
        ),
        choice(
            "coord_mgrs",
            "MGRS",
            &["FIND_MGRS_MAINWORLD", "DONT_FIND_MGRS_MAINWORLD"],
        ),
    ],
    output_suffix: "Locations",
    measures: false,
    run: Runner::Features(run_text),
};

#[cfg(test)]
mod tests {
    use super::*;

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    fn found(s: &str) -> Vec<(String, &'static str, i64, String)> {
        let t = chars(s);
        find_coordinates(&t, &Notations::default())
            .into_iter()
            .map(|f| {
                (
                    t[f.start..f.end].iter().collect(),
                    f.kind,
                    f.precision,
                    standard(f.lat, f.lon),
                )
            })
            .collect()
    }

    #[test]
    fn coordinates_as_arcgis_reads_them() {
        let one = |s: &str| found(s).into_iter().next();
        assert_eq!(
            one("a 34.0522 N, 118.2437 W b").unwrap(),
            (
                "34.0522 N, 118.2437 W".into(),
                "DD",
                9,
                "34.052200N 118.243700W".into()
            )
        );
        assert_eq!(one("a 34.05 N, 118.24 W b").unwrap().2, 922);
        assert_eq!(one("a 34 N, 118 W b").unwrap().2, 92288);
        assert_eq!(one("a 34.052200 N, 118.243700 W b").unwrap().2, 1);
        assert_eq!(one("a 80.0522 N, 118.2437 W b").unwrap().2, 2);
        assert_eq!(one("a 34.0522 N, 118.24 W b").unwrap().2, 11);
        assert_eq!(
            one("a N34.0522 W118.2437 b").unwrap().0,
            "N34.0522 W118.2437"
        );
        assert_eq!(one("a 34.0522N118.2437W b").unwrap().0, "34.0522N118.2437W");
        assert_eq!(
            one("a -118.2437, 34.0522 b").unwrap().3,
            "34.052200N 118.243700W"
        );
        assert_eq!(one("a 95.1, 20.2 b").unwrap().2, 10447);
        assert_eq!(one("a 12.5, 7.25 b").unwrap().2, 1087);
        assert!(one("a -34.0522 N, 118.2437 W b").is_none());
        assert!(one("a34.0522N 118.2437W b").is_none());
        assert!(one("a 34, -118 b").is_none());
        assert!(one("a 34.0522 -- -118.2437 b").is_none());
        assert_eq!(
            one("a 40 26 46 N 79 58 56 W b").unwrap(),
            (
                "40 26 46 N 79 58 56 W".into(),
                "DMS",
                31,
                "40.446111N 079.982222W".into()
            )
        );
        assert_eq!(one("a 40 26 46.5 N 79 58 56.5 W b").unwrap().2, 3);
        assert_eq!(one("a 402646N 0795856W b").unwrap().1, "DMS");
        assert_eq!(one("a 40:26:46N 79:58:56W b").unwrap().1, "DMS");
        assert_eq!(one("a 40°26'46\"N 79°58'56\"W b").unwrap().1, "DMS");
        assert_eq!(one("a 40 26.767 N 79 58.933 W b").unwrap().1, "DM");
        assert_eq!(one("a 40 26 N 79 58 W b").unwrap().2, 1855);
        assert_eq!(one("a 4026.767N 07958.933W b").unwrap().2, 2);
        assert_eq!(
            one("a 18SUJ2348306479 b").unwrap().3,
            "38.889463N 077.035242W"
        );
        assert_eq!(
            one("a 18S UJ 23483 06479 b").unwrap().0,
            "18S UJ 23483 06479"
        );
        assert_eq!(one("a 18SUJ234064 b").unwrap().2, 100);
        assert_eq!(
            one("a 17N 630084 4833438 b").unwrap().3,
            "43.642562N 079.387143W"
        );
        assert_eq!(
            one("a 17N 630084m E 4833438m N b").unwrap().0,
            "17N 630084m E 4833438m N"
        );
        assert!(one("a 17 630084 4833438 b").is_none());
        assert!(one("version 1.2.3 and phone 555-1234").is_none());
    }

    #[test]
    fn dates_as_arcgis_reads_them() {
        let t = chars(
            "on 2024-03-15 20240316 240317 24077 3/18/2024 03/19/24 3-20-2024 March 21, 2024 22 March 2024 Mar 23 2024 24 Mar 24 2024/03/25 26.03.2024",
        );
        let written: Vec<String> = find_dates(&t)
            .iter()
            .map(|d| t[d.start..d.end].iter().collect())
            .collect();
        assert_eq!(
            written,
            [
                "2024-03-15",
                "20240316",
                "240317",
                "24077",
                "3/18/2024",
                "03/19/24",
                "3-20-2024",
                "March 21, 2024",
                "22 March 2024",
                "Mar 23 2024",
                "24 Mar 24",
                "2024/03/25"
            ]
        );
    }
}
