//! Get Cell Value: the value of one cell, as the text ArcGIS returns.

use crate::{DataType, Raster};

/// Parses the "x y" location parameter (ERROR 000628 when it is not two numbers).
pub fn parse_location(text: &str) -> Result<(f64, f64), String> {
    let bad = || {
        "ERROR 000622: Failed to execute (Get Cell Value). Parameters are not valid.\n\
         ERROR 000628: Cannot set input into parameter location_point."
            .to_string()
    };
    let parts: Vec<&str> = text.split_whitespace().collect();
    if parts.len() != 2 {
        return Err(bad());
    }
    let x = parts[0].parse::<f64>().map_err(|_| bad())?;
    let y = parts[1].parse::<f64>().map_err(|_| bad())?;
    Ok((x, y))
}

/// The cell value at (x, y) in the 1-based `band`: `NoData` outside the raster, in a
/// NoData cell or for a band that does not exist. The right and bottom edges belong to the
/// last column and row.
pub fn cell_value_text(raster: &Raster, x: f64, y: f64, band: i64) -> String {
    let t = raster.transform;
    let (w, h) = (raster.width as f64, raster.height as f64);
    let (fc, fr) = ((x - t[0]) / t[1], (y - t[3]) / t[5]);
    if band < 1
        || band as usize > raster.bands.len()
        || !(0.0..=w).contains(&fc)
        || !(0.0..=h).contains(&fr)
    {
        return "NoData".into();
    }
    let col = (fc.floor() as usize).min(raster.width - 1);
    let row = (fr.floor() as usize).min(raster.height - 1);
    let v = raster.bands[band as usize - 1][row * raster.width + col];
    if raster.is_nodata(v) {
        return "NoData".into();
    }
    match raster.data_type {
        DataType::F32 => format!("{}", v as f32),
        DataType::F64 => format!("{v}"),
        _ => format!("{}", v as i64),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid() -> Raster {
        let mut r = Raster::new(2, 2, DataType::F32, [100.0, 10.0, 0.0, 200.0, 0.0, -10.0]);
        r.nodata = Some(-9999.0);
        r.bands = vec![vec![1.5, -9999.0, 3.0, 204.9949]];
        r
    }

    #[test]
    fn values_edges_and_nodata() {
        let r = grid();
        assert_eq!(cell_value_text(&r, 105.0, 195.0, 1), "1.5");
        assert_eq!(cell_value_text(&r, 115.0, 195.0, 1), "NoData");
        assert_eq!(cell_value_text(&r, 120.0, 180.0, 1), "204.9949");
        assert_eq!(cell_value_text(&r, 120.001, 180.0, 1), "NoData");
        assert_eq!(cell_value_text(&r, 105.0, 195.0, 2), "NoData");
        assert_eq!(cell_value_text(&r, 105.0, 195.0, 0), "NoData");
    }

    #[test]
    fn location_must_be_two_numbers() {
        assert_eq!(parse_location("1 2.5"), Ok((1.0, 2.5)));
        assert!(parse_location("500005").unwrap_err().contains("000628"));
        assert!(parse_location("abc").is_err());
    }
}
