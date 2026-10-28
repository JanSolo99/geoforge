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
