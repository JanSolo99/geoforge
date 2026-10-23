//! Cell statistics and the `.aux.xml` sidecar ArcGIS writes beside a raster it creates.
//! Later tools take their output cell types from the stored minimum and maximum, so a
//! raster written without them would make a chain of tools differ from ArcGIS's.

use std::path::Path;

use unigis_core::{Error, Result};

use crate::Raster;

/// Statistics of band 1's valid (not NoData) cells.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Statistics {
    pub count: u64,
    pub min: f64,
    pub max: f64,
    pub mean: f64,
    /// Sample standard deviation (0 for a single cell).
    pub std_dev: f64,
}

/// Statistics of the first band, or `None` if every cell is NoData.
pub fn statistics(r: &Raster) -> Option<Statistics> {
    let band = r.bands.first()?;
    let (mut n, mut min, mut max) = (0u64, f64::INFINITY, f64::NEG_INFINITY);
    // Welford's update keeps the variance accurate for large rasters.
    let (mut mean, mut m2) = (0.0, 0.0);
    for &v in band {
        if r.nodata.is_some_and(|nd| v == nd) || v.is_nan() {
            continue;
        }
        n += 1;
        min = min.min(v);
        max = max.max(v);
        let d = v - mean;
        mean += d / n as f64;
        m2 += d * (v - mean);
    }
    (n > 0).then(|| Statistics {
        count: n,
        min,
        max,
        mean,
        std_dev: if n > 1 {
            (m2 / (n - 1) as f64).sqrt()
        } else {
            0.0
        },
    })
}

/// C's `%.14g`: 14 significant digits, trailing zeros dropped (ArcGIS's number form).
pub fn format_g14(v: f64) -> String {
    if v == 0.0 {
        return "0".into();
    }
    if !v.is_finite() {
        return if v.is_nan() {
            "nan".into()
        } else if v > 0.0 {
            "inf".into()
        } else {
            "-inf".into()
        };
    }
    let sci = format!("{v:.13e}");
    let (mantissa, exp) = sci.split_once('e').unwrap();
    let exp: i32 = exp.parse().unwrap();
    if !(-5..14).contains(&exp) {
        let m = mantissa.trim_end_matches('0').trim_end_matches('.');
        return format!("{m}e{}{:02}", if exp < 0 { '-' } else { '+' }, exp.abs());
    }
    let decimals = (13 - exp).max(0) as usize;
    let fixed = format!("{v:.decimals$}");
    if fixed.contains('.') {
        fixed.trim_end_matches('0').trim_end_matches('.').to_owned()
    } else {
        fixed
    }
}

/// The `.aux.xml` text for `s`, as GDAL's PAM and ArcGIS write statistics.
/// `count` is the number written as STATISTICS_COUNT (ArcGIS's Basin writes 0 there).
pub fn aux_xml(s: &Statistics, count: u64) -> String {
    let item = |k: &str, v: String| format!("      <MDI key=\"STATISTICS_{k}\">{v}</MDI>\n");
    let mut out = String::from("<PAMDataset>\n  <PAMRasterBand band=\"1\">\n    <Metadata>\n");
    out += &item("COUNT", format!("{:.6}", count as f64));
    out += &item("MAXIMUM", format_g14(s.max));
    out += &item("MEAN", format_g14(s.mean));
    out += &item("MEDIAN", "nan".into());
    out += &item("MINIMUM", format_g14(s.min));
    out += &item("SKIPFACTORX", "1".into());
    out += &item("SKIPFACTORY", "1".into());
    out += &item("STDDEV", format_g14(s.std_dev));
    out += "    </Metadata>\n  </PAMRasterBand>\n</PAMDataset>\n";
    out
}

/// Writes `<raster file>.aux.xml` with the raster's statistics (nothing if it has no
/// valid cell).
pub fn write_aux_statistics(raster_path: &Path, r: &Raster) -> Result<()> {
    write_aux_statistics_count(raster_path, r, None)
}

/// [`write_aux_statistics`] with the written STATISTICS_COUNT replaced.
pub fn write_aux_statistics_count(
    raster_path: &Path,
    r: &Raster,
    count: Option<u64>,
) -> Result<()> {
    let Some(s) = statistics(r) else {
        return Ok(());
    };
    let mut name = raster_path.as_os_str().to_owned();
    name.push(".aux.xml");
    let path = std::path::PathBuf::from(name);
    std::fs::write(&path, aux_xml(&s, count.unwrap_or(s.count))).map_err(|e| Error::io(&path, e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn g14_matches_c() {
        for (v, want) in [
            (0.0, "0"),
            (7000.0, "7000"),
            (25.530275344848633, "25.530275344849"),
            (9.5949604846554, "9.5949604846554"),
            (-2.3810795454545, "-2.3810795454545"),
            (0.000123456789012345, "0.00012345678901234"),
            (1.5e-7, "1.5e-07"),
            (1.0e20, "1e+20"),
            (3.4028234663852886e38, "3.4028234663853e+38"),
        ] {
            assert_eq!(format_g14(v), want, "{v}");
        }
    }
}
