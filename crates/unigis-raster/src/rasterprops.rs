//! Get Raster Properties: one property of a raster, as the text ArcGIS Pro 3.7 returns.

use std::path::Path;

use crate::worldfile::format_g;
use crate::{DataType, Raster};

pub const PROPERTIES: &[&str] = &[
    "MINIMUM",
    "MAXIMUM",
    "MEAN",
    "STD",
    "TOP",
    "LEFT",
    "RIGHT",
    "BOTTOM",
    "CELLSIZEX",
    "CELLSIZEY",
    "VALUETYPE",
    "COLUMNCOUNT",
    "ROWCOUNT",
    "BANDCOUNT",
    "ANYNODATA",
    "ALLNODATA",
    "SENSORNAME",
    "PRODUCTNAME",
    "ACQUISITIONDATE",
    "SOURCETYPE",
    "CLOUDCOVER",
    "SUNAZIMUTH",
    "SUNELEVATION",
    "OFFNADIR",
];

fn value_type(t: DataType) -> &'static str {
    match t {
        DataType::U8 => "3",
        DataType::I8 => "4",
        DataType::U16 => "5",
        DataType::I16 => "6",
        DataType::U32 => "7",
        DataType::I32 => "8",
        DataType::F32 => "9",
        DataType::F64 => "10",
    }
}

fn domain_error(what: &str, member: &str) -> String {
    format!(
        "ERROR 000864: {what}: The input is not within the defined domain.\n\
         ERROR 000800: The value is not a member of {member}."
    )
}

/// The property as text. `band` is a name like `Band_2` (blank for the default).
pub fn property(
    raster: &Raster,
    path: &Path,
    property: &str,
    band: Option<&str>,
) -> Result<String, String> {
    if !PROPERTIES.contains(&property) {
        return Err(domain_error("Property type", &PROPERTIES.join(" | ")));
    }
    let bands = raster.bands.len();
    let chosen = match band {
        None => None,
        Some(b) => match b
            .strip_prefix("Band_")
            .and_then(|n| n.parse::<usize>().ok())
        {
            Some(n) if (1..=bands).contains(&n) => Some(n - 1),
            _ => {
                let names: Vec<String> = (1..=bands).map(|i| format!("Band_{i}")).collect();
                return Err(domain_error("Band Name", &names.join(" | ")));
            }
        },
    };
    let t = raster.transform;
    let (w, h) = (raster.width as f64, raster.height as f64);
    let g = |v: f64| format_g(v, 15).replace('e', "E");
    let nodata = |i: usize| {
        raster.bands[i]
            .iter()
            .filter(|&&v| raster.is_nodata(v))
            .count()
    };
    let range: Vec<usize> = chosen.map_or_else(|| (0..bands).collect(), |i| vec![i]);
    Ok(match property {
        "TOP" => g(t[3]),
        "BOTTOM" => g(t[3] + t[5] * h),
        "LEFT" => g(t[0]),
        "RIGHT" => g(t[0] + t[1] * w),
        // ArcGIS derives the cell size from the extent, so rounding in the corners shows.
        "CELLSIZEX" => g(((t[0] + t[1] * w) - t[0]).abs() / w),
        "CELLSIZEY" => g((t[3] - (t[3] + t[5] * h)).abs() / h),
        "VALUETYPE" => value_type(raster.data_type).into(),
        "COLUMNCOUNT" => raster.width.to_string(),
        "ROWCOUNT" => raster.height.to_string(),
        "BANDCOUNT" => bands.to_string(),
        "ANYNODATA" => (range.iter().any(|&i| nodata(i) > 0) as u8).to_string(),
        "ALLNODATA" => (range
            .iter()
            .all(|&i| nodata(i) == raster.width * raster.height) as u8)
            .to_string(),
        "SENSORNAME" | "PRODUCTNAME" | "ACQUISITIONDATE" | "CLOUDCOVER" | "SUNAZIMUTH"
        | "SUNELEVATION" | "OFFNADIR" => "UNKNOWN".into(),
        "SOURCETYPE" => crate::setprops::source_type(path).map_err(|e| e.to_string())?,
        stat => {
            // Statistics are the stored ones; ArcGIS does not compute them here.
            let key = match stat {
                "MINIMUM" => "STATISTICS_MINIMUM",
                "MAXIMUM" => "STATISTICS_MAXIMUM",
                "MEAN" => "STATISTICS_MEAN",
                _ => "STATISTICS_STDDEV",
            };
            let stored = crate::stats_metadata::read(path).map_err(|e| e.to_string())?;
            stored
                .get(chosen.unwrap_or(0))
                .and_then(|b| b.get(key))
                .cloned()
                .ok_or("ERROR 001100: Failed because no statistics are available.")?
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry_and_counts() {
        let mut r = Raster::new(4, 3, DataType::F32, [1000.0, 20.0, 0.0, 2030.0, 0.0, -10.0]);
        r.nodata = Some(-1.0);
        let p = Path::new("none.tif");
        let get = |q| property(&r, p, q, None).unwrap();
        assert_eq!(get("RIGHT"), "1080");
        assert_eq!(get("BOTTOM"), "2000");
        assert_eq!(get("CELLSIZEY"), "10");
        assert_eq!(get("VALUETYPE"), "9");
        assert_eq!(get("ANYNODATA"), "0");
        assert_eq!(get("SOURCETYPE"), "Generic");
        assert!(property(&r, p, "FOO", None).unwrap_err().contains("000864"));
        assert!(property(&r, p, "TOP", Some("Band_2")).is_err());
    }
}
