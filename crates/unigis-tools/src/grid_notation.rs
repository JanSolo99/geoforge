//! Grid notations of Convert Coordinate Notation: UTM (bands or zones), MGRS, USNG, GARS and
//! GEOREF from WGS84 longitude/latitude, as ArcGIS Pro 3.7 writes them (`ccn_*` cases).
//!
//! UTM uses the Krüger series of the transverse Mercator (Karney 2011, to n^6), with the Norway
//! and Svalbard zone exceptions; beyond 84°N and 80°S it is UPS. UTM coordinates are truncated to
//! the metre, MGRS and USNG ones rounded. For UPS the UTM notations subtract the false origin of
//! the MGRS polar grid (1 300 000 m in the north, 800 000 m in the south).

use std::f64::consts::PI;

const A: f64 = 6_378_137.0;
const F: f64 = 1.0 / 298.257_223_563;
const LETTERS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ";

/// A position on the UTM or UPS grid.
struct Grid {
    /// `None` for UPS.
    zone: Option<u32>,
    /// The latitude band (`C`..`X`), or `A`/`B`/`Y`/`Z` for UPS.
    band: char,
    easting: f64,
    northing: f64,
}

/// Transverse Mercator easting/northing offsets (metres, before false easting/northing) of
/// `lat`/`dlon` (radians, longitude relative to the central meridian), with scale 0.9996.
fn transverse_mercator(lat: f64, dlon: f64) -> (f64, f64) {
    let n = F / (2.0 - F);
    let (n2, n3) = (n * n, n * n * n);
    let (n4, n5, n6) = (n3 * n, n3 * n2, n3 * n3);
    let big_a = A / (1.0 + n) * (1.0 + n2 / 4.0 + n4 / 64.0 + n6 / 256.0);
    let alpha = [
        n / 2.0 - 2.0 * n2 / 3.0 + 5.0 * n3 / 16.0 + 41.0 * n4 / 180.0 - 127.0 * n5 / 288.0
            + 7891.0 * n6 / 37800.0,
        13.0 * n2 / 48.0 - 3.0 * n3 / 5.0 + 557.0 * n4 / 1440.0 + 281.0 * n5 / 630.0
            - 1_983_433.0 * n6 / 1_935_360.0,
        61.0 * n3 / 240.0 - 103.0 * n4 / 140.0
            + 15061.0 * n5 / 26880.0
            + 167_603.0 * n6 / 181_440.0,
        49561.0 * n4 / 161_280.0 - 179.0 * n5 / 168.0 + 6_601_661.0 * n6 / 7_257_600.0,
        34729.0 * n5 / 80640.0 - 3_418_889.0 * n6 / 1_995_840.0,
        212_378_941.0 * n6 / 319_334_400.0,
    ];
    let e = (F * (2.0 - F)).sqrt();
    let tau = lat.tan();
    let sigma = (e * (e * tau / (1.0 + tau * tau).sqrt()).atanh()).sinh();
    let tau_p = tau * (1.0 + sigma * sigma).sqrt() - sigma * (1.0 + tau * tau).sqrt();
    let xi_p = tau_p.atan2(dlon.cos());
    let eta_p = (dlon.sin() / (tau_p * tau_p + dlon.cos().powi(2)).sqrt()).asinh();
    let (mut xi, mut eta) = (xi_p, eta_p);
    for (j, a) in alpha.iter().enumerate() {
        let k = 2.0 * (j + 1) as f64;
        xi += a * (k * xi_p).sin() * (k * eta_p).cosh();
        eta += a * (k * xi_p).cos() * (k * eta_p).sinh();
    }
    (0.9996 * big_a * eta, 0.9996 * big_a * xi)
}

/// Universal Polar Stereographic easting/northing (scale 0.994, false origin 2 000 000 m).
fn polar_stereographic(lat: f64, lon: f64) -> (f64, f64) {
    let e = (F * (2.0 - F)).sqrt();
    let north = lat >= 0.0;
    let phi = lat.abs();
    let s = e * phi.sin();
    let t = (PI / 4.0 - phi / 2.0).tan() / ((1.0 - s) / (1.0 + s)).powf(e / 2.0);
    let rho = 2.0 * 0.994 * A * t / ((1.0 + e).powf(1.0 + e) * (1.0 - e).powf(1.0 - e)).sqrt();
    let e_ = 2_000_000.0 + rho * lon.sin();
    let n_ = if north {
        2_000_000.0 - rho * lon.cos()
    } else {
        2_000_000.0 + rho * lon.cos()
    };
    (e_, n_)
}

fn grid(lon: f64, lat: f64) -> Option<Grid> {
    if !(-90.0..=90.0).contains(&lat) || !lon.is_finite() {
        return None;
    }
    if !(-80.0..=84.0).contains(&lat) {
        let (easting, northing) = polar_stereographic(lat.to_radians(), lon.to_radians());
        let band = match (lat > 0.0, lon < 0.0) {
            (true, true) => 'Y',
            (true, false) => 'Z',
            (false, true) => 'A',
            (false, false) => 'B',
        };
        return Some(Grid {
            zone: None,
            band,
            easting,
            northing,
        });
    }
    let band_index = (((lat + 80.0) / 8.0).floor() as usize).min(19);
    let band = b"CDEFGHJKLMNPQRSTUVWX"[band_index] as char;
    let mut zone = (((lon + 180.0) / 6.0).floor() as i64).clamp(0, 59) as u32 + 1;
    if band == 'V' && (3.0..12.0).contains(&lon) {
        zone = 32;
    }
    if band == 'X' && (0.0..42.0).contains(&lon) {
        zone = match lon {
            l if l < 9.0 => 31,
            l if l < 21.0 => 33,
            l if l < 33.0 => 35,
            _ => 37,
        };
    }
    let central = f64::from(zone) * 6.0 - 183.0;
    let (x, y) = transverse_mercator(lat.to_radians(), (lon - central).to_radians());
    Some(Grid {
        zone: Some(zone),
        band,
        easting: 500_000.0 + x,
        northing: if lat < 0.0 { 10_000_000.0 + y } else { y },
    })
}

/// `UTM_BANDS` (`33T 509614 5058379`) or `UTM_ZONES` (`33N5096145058379`).
fn utm(lon: f64, lat: f64, bands: bool) -> Option<String> {
    let g = grid(lon, lat)?;
    let (e, n) = match g.zone {
        Some(_) => (g.easting, g.northing),
        None => {
            let origin = if lat > 0.0 { 1_300_000.0 } else { 800_000.0 };
            (g.easting - origin, g.northing - origin)
        }
    };
    let (e, n) = (e.floor() as i64, n.floor() as i64);
    Some(match (g.zone, bands) {
        (Some(z), true) => format!("{z:02}{} {e:06} {n:07}", g.band),
        (Some(z), false) => {
            let hemi = if lat >= 0.0 { 'N' } else { 'S' };
            format!("{z:02}{hemi}{e:06}{n:07}")
        }
        (None, true) => format!("{} {e:06} {n:07}", g.band),
        (None, false) => format!("{}{e:06}{n:07}", g.band),
    })
}

/// MGRS (`33TWL0961558380`) or, with spaces, USNG (`33T WL 09615 58380`); 1 m precision.
fn mgrs(lon: f64, lat: f64, spaced: bool) -> Option<String> {
    let g = grid(lon, lat)?;
    let (e, n) = (g.easting.round() as i64, g.northing.round() as i64);
    let (zone, col, row) = match g.zone {
        Some(z) => {
            let set = (z - 1) % 3;
            let cols: &[u8] = [b"ABCDEFGH", b"JKLMNPQR", b"STUVWXYZ"][set as usize];
            let col = cols[((e / 100_000 - 1).clamp(0, 7)) as usize];
            let offset = if z % 2 == 0 { 5 } else { 0 };
            let row = b"ABCDEFGHJKLMNPQRSTUV"[((n / 100_000 + offset) % 20) as usize];
            // Rounding a point just south of the equator up to 10 000 000 m puts it in band N.
            let band = if lat < 0.0 && n >= 10_000_000 {
                'N'
            } else {
                g.band
            };
            (format!("{z:02}{band}"), col, row)
        }
        None => {
            let (cols, col_origin, rows, row_origin): (&[u8], i64, &[u8], i64) = match g.band {
                'Y' => (b"RSTUXYZ", 1_300_000, b"ABCDEFGHJKLMNP", 1_300_000),
                'Z' => (b"ABCFGHJ", 2_000_000, b"ABCDEFGHJKLMNP", 1_300_000),
                'A' => (b"JKLPQRSTUXYZ", 800_000, LETTERS, 800_000),
                _ => (b"ABCFGHJKLPQR", 2_000_000, LETTERS, 800_000),
            };
            let pick =
                |set: &[u8], v: i64| set[((v / 100_000).clamp(0, set.len() as i64 - 1)) as usize];
            (
                g.band.to_string(),
                pick(cols, e - col_origin),
                pick(rows, n - row_origin),
            )
        }
    };
    let (col, row) = (col as char, row as char);
    let (de, dn) = (e.rem_euclid(100_000), n.rem_euclid(100_000));
    Some(if spaced {
        format!("{zone} {col}{row} {de:05} {dn:05}")
    } else {
        format!("{zone}{col}{row}{de:05}{dn:05}")
    })
}

/// GARS (`391MH32`): 30' longitude band, 30' latitude letters, 15' quadrant, 5' keypad cell.
/// Longitude 180 wraps to band 001; latitude 90 stays in the top cell.
fn gars(lon: f64, lat: f64) -> Option<String> {
    if !(-90.0..=90.0).contains(&lat) || !lon.is_finite() {
        return None;
    }
    let x = (lon + 180.0) * 2.0;
    let y = (lat + 90.0) * 2.0;
    let xi = x.floor();
    let yi = y.floor().min(359.0);
    let (fx, fy) = (x - xi, y - yi);
    let band = (xi as i64).rem_euclid(720) + 1;
    let (l1, l2) = (LETTERS[yi as usize / 24], LETTERS[yi as usize % 24]);
    let east = fx >= 0.5;
    let north = fy >= 0.5;
    let quadrant = match (north, east) {
        (true, false) => 1,
        (true, true) => 2,
        (false, false) => 3,
        (false, true) => 4,
    };
    let qx = if east { fx - 0.5 } else { fx } * 2.0;
    let qy = if north { fy - 0.5 } else { fy } * 2.0;
    let col = ((qx * 3.0).floor() as i64).min(2);
    let row = 2 - ((qy * 3.0).floor() as i64).min(2);
    let key = row * 3 + col + 1;
    Some(format!(
        "{band:03}{}{}{quadrant}{key}",
        l1 as char, l2 as char
    ))
}

/// GARS for decoded grid positions. ArcGIS grid corners use a degree/radian
/// roundtrip followed by direct five-minute tick floors. Keep the operation
/// order: snapping or staged fractional-cell arithmetic changes boundary cells.
pub(crate) fn gars_from_grid(lon: f64, lat: f64) -> Option<String> {
    if !(-180.0..=180.0).contains(&lon) || !(-90.0..=90.0).contains(&lat) {
        return None;
    }
    let x = (((lon.to_radians().to_degrees() + 180.0) * 12.0).floor() as i64).rem_euclid(4320);
    let y = (((lat.to_radians().to_degrees() + 90.0) * 12.0).floor() as i64).clamp(0, 2159);
    let quadrant = match (y % 6 >= 3, x % 6 >= 3) {
        (true, false) => 1,
        (true, true) => 2,
        (false, false) => 3,
        (false, true) => 4,
    };
    let key = (2 - y % 3) * 3 + x % 3 + 1;
    Some(format!(
        "{:03}{}{}{quadrant}{key}",
        x / 6 + 1,
        LETTERS[y as usize / 6 / 24] as char,
        LETTERS[y as usize / 6 % 24] as char
    ))
}

/// GEOREF (`PKAA0740740734`): 15° tile letters, degree letters, then minutes to 0.001 of the
/// longitude and the latitude (rounded, with carry; the east and north edges stay in the last
/// tile at 59.999').
fn georef(lon: f64, lat: f64) -> Option<String> {
    if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
        return None;
    }
    let thousandths = |v: f64, span: f64| {
        let max = (span * 60_000.0) as i64 - 1;
        ((v * 60_000.0).round() as i64).clamp(0, max)
    };
    let (x, y) = (
        thousandths(lon + 180.0, 360.0),
        thousandths(lat + 90.0, 180.0),
    );
    let split = |m: i64| {
        (
            (m / 900_000) as usize,
            ((m / 60_000) % 15) as usize,
            m % 60_000,
        )
    };
    let (xt, xd, xm) = split(x);
    let (yt, yd, ym) = split(y);
    Some(format!(
        "{}{}{}{}{xm:05}{ym:05}",
        LETTERS[xt] as char, LETTERS[yt] as char, LETTERS[xd] as char, LETTERS[yd] as char
    ))
}

/// Decode the observed GARS/GEOREF single-field syntax. Coordinates identify
/// the southwest corner of the selected cell, rather than its centre.
/// GEOREF minute overflow is retained in geometry even outside geographic bounds;
/// the caller separately validates the output notation fields.
pub(crate) fn parse_grid_notation(format: &str, text: &str) -> Option<(f64, f64)> {
    let normalized: Vec<u8> = text
        .bytes()
        .filter(|b| !b.is_ascii_whitespace())
        .map(|b| b.to_ascii_uppercase())
        .collect();
    let s = normalized.as_slice();
    let letter = |c: u8| LETTERS.iter().position(|b| *b == c);
    let number = |digits: &[u8]| -> Option<f64> {
        if !digits.iter().all(u8::is_ascii_digit) {
            return None;
        }
        std::str::from_utf8(digits).ok()?.parse().ok()
    };
    match format {
        "GARS" => {
            if s.len() < 5 {
                return None;
            }
            let band = number(&s[..3])?;
            let row = letter(s[3])? * 24 + letter(s[4])?;
            if !(1.0..=720.0).contains(&band) || row >= 360 {
                return None;
            }
            // Decode in the positive grid frame, then subtract the origin last.
            // This ordering is observable when a corner is re-encoded as GARS.
            let mut x = (band as usize - 1) * 6;
            let mut y = row * 6;
            if let Some(&quadrant) = s.get(5) {
                if !(b'1'..=b'4').contains(&quadrant) {
                    return None;
                }
                if quadrant == b'2' || quadrant == b'4' {
                    x += 3;
                }
                if quadrant == b'1' || quadrant == b'2' {
                    y += 3;
                }
            }
            if let Some(&key) = s.get(6) {
                if !(b'1'..=b'9').contains(&key) {
                    return None;
                }
                let index = key - b'1';
                x += usize::from(index % 3);
                y += usize::from(2 - index / 3);
            }
            // ArcGIS ignores characters following the first seven GARS characters.
            Some((x as f64 / 12.0 - 180.0, y as f64 / 12.0 - 90.0))
        }
        "GEOREF" => {
            if !(8..=22).contains(&s.len()) || !s.len().is_multiple_of(2) {
                return None;
            }
            let (xt, yt, xd, yd) = (letter(s[0])?, letter(s[1])?, letter(s[2])?, letter(s[3])?);
            if yt >= 12 || xd >= 15 || yd >= 15 {
                return None;
            }
            let digits = (s.len() - 4) / 2;
            let scale = 10_f64.powi(digits as i32 - 2) * 60.0;
            Some((
                (xt * 15 + xd) as f64 - 180.0 + number(&s[4..4 + digits])? / scale,
                (yt * 15 + yd) as f64 - 90.0 + number(&s[4 + digits..])? / scale,
            ))
        }
        "UTM_BANDS" | "UTM_ZONES" => parse_utm(s, format == "UTM_BANDS"),
        "MGRS" | "USNG" => parse_mgrs(s),
        _ => None,
    }
}

/// Inverse transverse Mercator (Krüger series to n^6, scale 0.9996): latitude and longitude
/// offset from the central meridian (radians) of easting/northing offsets in metres.
fn inverse_transverse_mercator(x: f64, y: f64) -> (f64, f64) {
    let n = F / (2.0 - F);
    let (n2, n3) = (n * n, n * n * n);
    let (n4, n5, n6) = (n3 * n, n3 * n2, n3 * n3);
    let big_a = A / (1.0 + n) * (1.0 + n2 / 4.0 + n4 / 64.0 + n6 / 256.0);
    let beta = [
        n / 2.0 - 2.0 * n2 / 3.0 + 37.0 * n3 / 96.0 - n4 / 360.0 - 81.0 * n5 / 512.0
            + 96199.0 * n6 / 604_800.0,
        n2 / 48.0 + n3 / 15.0 - 437.0 * n4 / 1440.0 + 46.0 * n5 / 105.0
            - 1_118_711.0 * n6 / 3_870_720.0,
        17.0 * n3 / 480.0 - 37.0 * n4 / 840.0 - 209.0 * n5 / 4480.0 + 5569.0 * n6 / 90720.0,
        4397.0 * n4 / 161_280.0 - 11.0 * n5 / 504.0 - 830_251.0 * n6 / 7_257_600.0,
        4583.0 * n5 / 161_280.0 - 108_847.0 * n6 / 3_991_680.0,
        20_648_693.0 * n6 / 638_668_800.0,
    ];
    let xi = y / (0.9996 * big_a);
    let eta = x / (0.9996 * big_a);
    let (mut xi_p, mut eta_p) = (xi, eta);
    for (j, b) in beta.iter().enumerate() {
        let k = 2.0 * (j + 1) as f64;
        xi_p -= b * (k * xi).sin() * (k * eta).cosh();
        eta_p -= b * (k * xi).cos() * (k * eta).sinh();
    }
    let tau_p = xi_p.sin() / (eta_p.sinh().powi(2) + xi_p.cos().powi(2)).sqrt();
    let dlon = eta_p.sinh().atan2(xi_p.cos());
    let e2 = F * (2.0 - F);
    let e = e2.sqrt();
    let mut tau = tau_p;
    for _ in 0..10 {
        let sigma = (e * (e * tau / (1.0 + tau * tau).sqrt()).atanh()).sinh();
        let ti = tau * (1.0 + sigma * sigma).sqrt() - sigma * (1.0 + tau * tau).sqrt();
        let delta = (tau_p - ti) / (1.0 + ti * ti).sqrt() * (1.0 + (1.0 - e2) * tau * tau)
            / ((1.0 - e2) * (1.0 + tau * tau).sqrt());
        tau += delta;
        if delta.abs() < 1e-14 {
            break;
        }
    }
    (tau.atan(), dlon)
}

/// Inverse Universal Polar Stereographic (false origin 2 000 000 m): longitude and latitude in
/// degrees.
fn inverse_polar_stereographic(easting: f64, northing: f64, north: bool) -> (f64, f64) {
    let e = (F * (2.0 - F)).sqrt();
    let (dx, dy) = (easting - 2_000_000.0, northing - 2_000_000.0);
    let rho = dx.hypot(dy);
    let t = rho * ((1.0 + e).powf(1.0 + e) * (1.0 - e).powf(1.0 - e)).sqrt() / (2.0 * 0.994 * A);
    let mut phi = PI / 2.0 - 2.0 * t.atan();
    for _ in 0..20 {
        let s = e * phi.sin();
        let next = PI / 2.0 - 2.0 * (t * ((1.0 - s) / (1.0 + s)).powf(e / 2.0)).atan();
        if (next - phi).abs() < 1e-15 {
            phi = next;
            break;
        }
        phi = next;
    }
    // The pole itself has longitude 0.
    let lon = match (rho == 0.0, north) {
        (true, _) => 0.0,
        (false, true) => dx.atan2(-dy),
        (false, false) => dx.atan2(dy),
    };
    let lat = if north { phi } else { -phi };
    (lon.to_degrees(), lat.to_degrees())
}

/// Longitude and latitude of UTM `zone` easting/northing (northing with the 10 000 000 m false
/// northing in the south).
fn from_utm(zone: u32, north: bool, easting: f64, northing: f64) -> (f64, f64) {
    let y = if north {
        northing
    } else {
        northing - 10_000_000.0
    };
    let (lat, dlon) = inverse_transverse_mercator(easting - 500_000.0, y);
    // Zones 1 and 60 reach across the antimeridian: wrap into -180..180.
    let mut lon = f64::from(zone) * 6.0 - 183.0 + dlon.to_degrees();
    if lon > 180.0 {
        lon -= 360.0;
    } else if lon < -180.0 {
        lon += 360.0;
    }
    (lon, lat.to_degrees())
}

/// The leading 1- or 2-digit zone (1..=60) of `s`, and the rest.
fn split_zone(s: &[u8]) -> Option<(Option<u32>, &[u8])> {
    let digits = s.iter().take(2).take_while(|b| b.is_ascii_digit()).count();
    if digits == 0 {
        return Some((None, s));
    }
    let zone: u32 = std::str::from_utf8(&s[..digits]).ok()?.parse().ok()?;
    (1..=60)
        .contains(&zone)
        .then_some((Some(zone), &s[digits..]))
}

/// UTM_BANDS (`33T 509614 5058379`) or UTM_ZONES (`33N5096145058379`), whitespace already
/// removed: the digits split in half, the easting taking the shorter half (`509614` is E 509,
/// N 614). Polar (UPS) positions carry no zone and are written relative to the MGRS polar grid
/// origin.
fn parse_utm(s: &[u8], bands: bool) -> Option<(f64, f64)> {
    let (zone, rest) = split_zone(s)?;
    let (&letter, digits) = rest.split_first()?;
    if digits.len() < 2 || !digits.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let text = std::str::from_utf8(digits).ok()?;
    let (e, n) = text.split_at(text.len() / 2);
    let (easting, northing): (f64, f64) = (e.parse().ok()?, n.parse().ok()?);
    match zone {
        None => {
            let north = match letter {
                b'Y' | b'Z' => true,
                b'A' | b'B' => false,
                _ => return None,
            };
            let origin = if north { 1_300_000.0 } else { 800_000.0 };
            Some(inverse_polar_stereographic(
                easting + origin,
                northing + origin,
                north,
            ))
        }
        Some(zone) => {
            let north = if bands {
                let band = b"CDEFGHJKLMNPQRSTUVWX".iter().position(|&b| b == letter)?;
                band >= 10
            } else {
                match letter {
                    b'N' => true,
                    b'S' => false,
                    _ => return None,
                }
            };
            Some(from_utm(zone, north, easting, northing))
        }
    }
}

/// Column letters and false easting, row letters and false northing, and hemisphere of an MGRS
/// polar grid.
type PolarGrid = (&'static [u8], f64, &'static [u8], f64, bool);

/// MGRS or USNG (whitespace already removed): zone and band (or a polar band alone), the 100 km
/// square letters, then an even number (0..=10) of digits; a shorter reference is the south-west
/// corner of its square. The band picks the 2 000 km cycle of the row letter.
fn parse_mgrs(s: &[u8]) -> Option<(f64, f64)> {
    let (zone, rest) = split_zone(s)?;
    if rest.len() < 3 {
        return None;
    }
    let (band, col, row, digits) = (rest[0], rest[1], rest[2], &rest[3..]);
    if digits.len() > 10
        || !digits.len().is_multiple_of(2)
        || !digits.iter().all(u8::is_ascii_digit)
    {
        return None;
    }
    let half = digits.len() / 2;
    let scale = 10_f64.powi(5 - half as i32);
    let value = |d: &[u8]| -> Option<f64> {
        if d.is_empty() {
            Some(0.0)
        } else {
            Some(std::str::from_utf8(d).ok()?.parse::<f64>().ok()? * scale)
        }
    };
    let (de, dn) = (value(&digits[..half])?, value(&digits[half..])?);
    let index = |set: &[u8], c: u8| set.iter().position(|&b| b == c);
    match zone {
        None => {
            let (cols, col_origin, rows, row_origin, north): PolarGrid = match band {
                b'Y' => (
                    b"RSTUXYZ",
                    1_300_000.0,
                    b"ABCDEFGHJKLMNP",
                    1_300_000.0,
                    true,
                ),
                b'Z' => (
                    b"ABCFGHJ",
                    2_000_000.0,
                    b"ABCDEFGHJKLMNP",
                    1_300_000.0,
                    true,
                ),
                b'A' => (b"JKLPQRSTUXYZ", 800_000.0, LETTERS, 800_000.0, false),
                b'B' => (b"ABCFGHJKLPQR", 2_000_000.0, LETTERS, 800_000.0, false),
                _ => return None,
            };
            let easting = col_origin + index(cols, col)? as f64 * 100_000.0 + de;
            let northing = row_origin + index(rows, row)? as f64 * 100_000.0 + dn;
            Some(inverse_polar_stereographic(easting, northing, north))
        }
        Some(zone) => {
            let bands = b"CDEFGHJKLMNPQRSTUVWX";
            let band_index = bands.iter().position(|&b| b == band)?;
            let cols: &[u8] = [b"ABCDEFGH", b"JKLMNPQR", b"STUVWXYZ"][((zone - 1) % 3) as usize];
            let easting = (index(cols, col)? + 1) as f64 * 100_000.0 + de;
            let rows = b"ABCDEFGHJKLMNPQRSTUV";
            let offset = if zone % 2 == 0 { 5 } else { 0 };
            let row_value = ((index(rows, row)? + 20 - offset) % 20) as f64 * 100_000.0;
            // The bottom of the band on the central meridian, floored to 100 km.
            let band_lat = -80.0 + 8.0 * band_index as f64;
            let north = band_index >= 10;
            let (_, y) = transverse_mercator(band_lat.to_radians(), 0.0);
            let band_northing = if north { y } else { 10_000_000.0 + y };
            let band_northing = (band_northing / 100_000.0).floor() * 100_000.0;
            let mut northing = row_value + dn;
            while northing < band_northing {
                northing += 2_000_000.0;
            }
            Some(from_utm(zone, north, easting, northing))
        }
    }
}

/// The notation `format` (`UTM_BANDS`, `UTM_ZONES`, `MGRS`, `USNG`, `GARS`, `GEOREF`, upper
/// case) of a WGS84 position; `None` for other formats.
pub(crate) fn grid_notation(format: &str, lon: f64, lat: f64) -> Option<Option<String>> {
    Some(match format {
        "UTM_BANDS" => utm(lon, lat, true),
        "UTM_ZONES" => utm(lon, lat, false),
        "MGRS" => mgrs(lon, lat, false),
        "USNG" => mgrs(lon, lat, true),
        "GARS" => gars(lon, lat),
        "GEOREF" => georef(lon, lat),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gars_grid_boundaries_match_arcgis() {
        for (format, input, expected) in [
            ("GARS", "116LR17", "115LR29"),
            ("GARS", "161AA34", "161AA37"),
            ("GARS", "391MH32", "391MH32"),
            ("GARS", "249NP46", "249NP46"),
            ("GEOREF", "AAAA0000", "001AA37"),
            ("GEOREF", "ZMQQ6060", "001QZ11"),
        ] {
            let (x, y) = parse_grid_notation(format, input).unwrap();
            assert_eq!(gars_from_grid(x, y).as_deref(), Some(expected), "{input}");
        }
        assert!(gars_from_grid(181.0, 0.0).is_none());
        assert!(gars_from_grid(0.0, f64::NAN).is_none());
    }

    #[test]
    fn grid_inputs_match_arcgis() {
        for (fmt, text, lon, lat) in [
            ("GARS", "391MH", 15.0, 45.5),
            ("GARS", "391MH1", 15.0, 45.75),
            ("GARS", "391MH2", 15.25, 45.75),
            ("GARS", "391MH4", 15.25, 45.5),
            ("GARS", "391 MH 32", 15.083333333333, 45.666666666667),
            ("GARS", "391MH31junk", 15.0, 45.666666666667),
            ("GARS", "001AA11", -180.0, -89.583333333333),
            ("GARS", "720QZ49", 179.916666666667, 89.5),
            ("GEOREF", "pkaa0740740734", 15.12345, 45.6789),
            ("GEOREF", "PKAA 07407 40734", 15.12345, 45.6789),
            ("GEOREF", "PKAA0740", 15.116666666667, 45.666666666667),
            (
                "GEOREF",
                "PKAA07407407340000",
                15.123456666667,
                46.223333333333,
            ),
            ("GEOREF", "PKAA6040", 16.0, 45.666666666667),
            ("GEOREF", "ZMQQ6060", 180.0, 90.0),
            ("GEOREF", "ZMQQ9999999999", 180.66665, 90.66665),
        ] {
            let (x, y) = parse_grid_notation(fmt, text).unwrap();
            assert!(
                (x - lon).abs() < 1e-10 && (y - lat).abs() < 1e-10,
                "{fmt}: {text}: {x}, {y}"
            );
        }
        for (fmt, text) in [
            ("GARS", "000AA11"),
            ("GARS", "721AA11"),
            ("GARS", "001RA11"),
            ("GARS", "001AI11"),
            ("GARS", "391MH5"),
            ("GARS", "391MH30"),
            ("GARS", "391MH3junk"),
            ("GARS", ""),
            ("GEOREF", "PKAA"),
            ("GEOREF", "PKAA07"),
            ("GEOREF", "PKAA07407"),
            ("GEOREF", "ZZAA0000"),
            ("GEOREF", "NMRA0000"),
            ("GEOREF", "NKII0000"),
            ("GEOREF", "PKAA07407407340000000000"),
            ("GEOREF", "PKAA-07404073"),
            ("GEOREF", "PKAA0740740734.0"),
            ("GEOREF", ""),
        ] {
            assert!(parse_grid_notation(fmt, text).is_none(), "{fmt}: {text}");
        }
    }

    /// ArcGIS Pro 3.7 outputs for points across zones, the Norway/Svalbard exceptions and UPS.
    #[test]
    fn grid_notations_match_arcgis() {
        let cases: &[(f64, f64, [&str; 6])] = &[
            (
                15.123456,
                45.678901,
                [
                    "33T 509614 5058379",
                    "33N5096145058379",
                    "33TWL0961558380",
                    "33T WL 09615 58380",
                    "391MH32",
                    "PKAA0740740734",
                ],
            ),
            (
                -0.0001,
                0.0001,
                [
                    "30N 833967 0000011",
                    "30N8339670000011",
                    "30NZF3396700011",
                    "30N ZF 33967 00011",
                    "360HN49",
                    "MGQA5999400006",
                ],
            ),
            (
                3.0,
                56.5,
                [
                    "32V 130903 6277869",
                    "32N1309036277869",
                    "32VJH3090377870",
                    "32V JH 30903 77870",
                    "367NF37",
                    "NKDM0000030000",
                ],
            ),
            (
                8.9,
                72.5,
                [
                    "31X 697737 8054424",
                    "31N6977378054424",
                    "31XFA9773754425",
                    "31X FA 97737 54425",
                    "378PP48",
                    "NLJN5400030000",
                ],
            ),
            (
                0.0,
                85.0,
                [
                    "Z 700000 0144542",
                    "Z7000000144542",
                    "ZAB0000044543",
                    "Z AB 00000 44543",
                    "361QQ37",
                    "NMAL0000000000",
                ],
            ),
            (
                -170.0,
                -82.0,
                [
                    "A 1045526 0323936",
                    "A10455260323936",
                    "AYD4552623937",
                    "A YD 45526 23937",
                    "021AS37",
                    "AALJ0000000000",
                ],
            ),
            (
                10.0,
                -85.0,
                [
                    "B 1296454 1747018",
                    "B12964541747018",
                    "BAT9645447019",
                    "B AT 96454 47019",
                    "381AL37",
                    "NALF0000000000",
                ],
            ),
            (
                -10.0,
                86.0,
                [
                    "Y 622853 0262481",
                    "Y6228530262481",
                    "YZC2285462481",
                    "Y ZC 22854 62481",
                    "341QS37",
                    "MMFM0000000000",
                ],
            ),
            (
                10.0,
                -80.0,
                [
                    "32C 519384 1118247",
                    "32S5193841118247",
                    "32CNS1938518248",
                    "32C NS 19385 18248",
                    "381AW37",
                    "NALL0000000000",
                ],
            ),
            (
                10.0,
                -80.01,
                [
                    "B 1393067 2294941",
                    "B13930672294941",
                    "BBY9306894942",
                    "B BY 93068 94942",
                    "381AV11",
                    "NALK0000059400",
                ],
            ),
            (
                180.0,
                10.0,
                [
                    "60P 828928 1106908",
                    "60N8289281106908",
                    "60PZS2892906909",
                    "60P ZS 28929 06909",
                    "001JJ37",
                    "ZGQL5999900000",
                ],
            ),
            (
                10.0,
                90.0,
                [
                    "Z 700000 0700000",
                    "Z7000000700000",
                    "ZAH0000000000",
                    "Z AH 00000 00000",
                    "381QZ11",
                    "NMLQ0000059999",
                ],
            ),
            (
                170.0,
                89.9,
                [
                    "Z 701927 0710933",
                    "Z7019270710933",
                    "ZAH0192810934",
                    "Z AH 01928 10934",
                    "701QZ14",
                    "ZMFQ0000054000",
                ],
            ),
            (
                -0.0000001,
                -0.0000001,
                [
                    "30M 833978 9999999",
                    "30S8339789999999",
                    "30NZF3397900000",
                    "30N ZF 33979 00000",
                    "360HM23",
                    "NGAA0000000000",
                ],
            ),
            (
                -45.999996,
                45.999996,
                [
                    "23T 422568 5094533",
                    "23N4225685094533",
                    "23TML2256894533",
                    "23T ML 22568 94533",
                    "269MH11",
                    "JKQB0000000000",
                ],
            ),
        ];
        let formats = ["UTM_BANDS", "UTM_ZONES", "MGRS", "USNG", "GARS", "GEOREF"];
        for (lon, lat, want) in cases {
            for (f, w) in formats.iter().zip(want) {
                let got = grid_notation(f, *lon, *lat).unwrap().unwrap();
                assert_eq!(&got, w, "{f} of {lon} {lat}");
            }
        }
    }
}

#[cfg(test)]
mod projected_input_tests {
    use super::parse_grid_notation;

    /// ArcGIS Pro 3.7 DDLon/DDLat (rounded to 8 decimals) of projected-grid strings; `None` is
    /// a failed conversion.
    #[test]
    fn projected_grid_inputs_match_arcgis() {
        type Case = (&'static str, &'static str, Option<(f64, f64)>);
        let cases: &[Case] = &[
            ("MGRS", "33TWL0961558380", Some((15.12346077, 45.67890452))),
            ("MGRS", "33twl0961558380", Some((15.12346077, 45.67890452))),
            (
                "MGRS",
                "33T WL 09615 58380",
                Some((15.12346077, 45.67890452)),
            ),
            ("MGRS", "33TWL09615838", Some((15.12339657, 45.67890459))),
            ("MGRS", "33TWL096583", Some((15.12326658, 45.67818466))),
            ("MGRS", "33TWL0958", Some((15.11555688, 45.67549246))),
            ("MGRS", "33TWL", Some((15.0, 45.15347718))),
            ("MGRS", "33TWL096155838", None),
            ("MGRS", "3TWL0961558380", Some((-164.87653923, 45.67890452))),
            ("MGRS", "ZAB0000044543", Some((0.0, 85.00000352))),
            ("MGRS", "junk", None),
            ("MGRS", "", None),
            ("MGRS", "33XWL0961558380", Some((15.58841475, 81.58345108))),
            ("MGRS", "33TWL09615583800", None),
            ("USNG", "Z AB 00000 44543", Some((0.0, 85.00000352))),
            (
                "UTM_BANDS",
                "33T 509614 5058379",
                Some((15.12344791, 45.67889553)),
            ),
            (
                "UTM_BANDS",
                "33t509614 5058379",
                Some((15.12344791, 45.67889553)),
            ),
            (
                "UTM_BANDS",
                "33T5096145058379",
                Some((15.12344791, 45.67889553)),
            ),
            ("UTM_BANDS", "Z 700000 0144542", Some((0.0, 84.99999453))),
            ("UTM_BANDS", "33T 509614.5 5058379.5", None),
            (
                "UTM_BANDS",
                "3T 509614 5058379",
                Some((-164.87655209, 45.67889553)),
            ),
            (
                "UTM_ZONES",
                "33N 509614 5058379",
                Some((15.12344791, 45.67889553)),
            ),
            (
                "UTM_ZONES",
                "31S2217269972340",
                Some((0.49999714, -0.2500089)),
            ),
            ("UTM_ZONES", "Z7000000144542", Some((0.0, 84.99999453))),
            (
                "UTM_ZONES",
                "A10455260323936",
                Some((-169.99998224, -81.9999931)),
            ),
            ("UTM_ZONES", "junk", None),
            ("UTM_BANDS", "33T 509614", Some((10.51581624, 0.00553793))),
            (
                "UTM_BANDS",
                "01P 171071 1106908",
                Some((179.99999767, 9.99999226)),
            ),
            ("UTM_BANDS", "Z 700000 0700000", Some((0.0, 90.0))),
            (
                "MGRS",
                "60PZS2892906909",
                Some((-179.99999758, 10.00000129)),
            ),
        ];
        let round = |v: f64| (v * 1e8).round() / 1e8;
        for (format, text, want) in cases {
            let got = parse_grid_notation(format, text).map(|(x, y)| (round(x), round(y)));
            assert_eq!(got, *want, "{format} {text:?}");
        }
    }
}
