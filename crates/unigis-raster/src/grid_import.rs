//! Legacy ArcGIS ASCII To Raster and Float To Raster, without a GDAL runtime.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::iter::Peekable;
use std::path::Path;
use std::str::SplitWhitespace;

use unigis_core::{Error, Result};

use crate::{DataType, MAX_CELLS, Raster};

fn invalid(message: impl Into<String>) -> Error {
    Error::Unsupported(format!("ERROR 010067: {}", message.into()))
}

struct Header {
    width: usize,
    height: usize,
    transform: [f64; 6],
    nodata: f64,
    big_endian: bool,
}

fn header(text: &str) -> Result<(Header, Peekable<SplitWhitespace<'_>>)> {
    let mut tokens = text
        .trim_start_matches('\u{feff}')
        .split_whitespace()
        .peekable();
    let mut fields = HashMap::new();
    while let Some(key) = tokens.peek() {
        let key = key.to_ascii_lowercase();
        if !matches!(
            key.as_str(),
            "ncols"
                | "nrows"
                | "xllcorner"
                | "yllcorner"
                | "xllcenter"
                | "yllcenter"
                | "cellsize"
                | "nodata_value"
                | "byteorder"
        ) {
            break;
        }
        tokens.next();
        let value = tokens
            .next()
            .ok_or_else(|| invalid("incomplete grid header"))?;
        if fields.insert(key, value).is_some() {
            return Err(invalid("duplicate grid header field"));
        }
    }
    let number = |key: &str| -> Result<f64> {
        let value: f64 = fields
            .get(key)
            .ok_or_else(|| invalid(format!("missing {key}")))?
            .parse()
            .map_err(|_| invalid(format!("invalid {key}")))?;
        if !value.is_finite() {
            return Err(invalid(format!("non-finite {key} is unsupported")));
        }
        Ok(value)
    };
    let dimension = |key: &str| -> Result<usize> {
        let n: usize = fields
            .get(key)
            .ok_or_else(|| invalid(format!("missing {key}")))?
            .parse()
            .map_err(|_| invalid(format!("invalid {key}")))?;
        if n == 0 {
            return Err(invalid("grid dimensions must be positive"));
        }
        Ok(n)
    };
    let width = dimension("ncols")?;
    let height = dimension("nrows")?;
    if width
        .checked_mul(height)
        .is_none_or(|n| n as u64 > MAX_CELLS)
    {
        return Err(invalid("grid has too many cells"));
    }
    let cell = number("cellsize")?;
    if cell <= 0.0 {
        return Err(invalid("cell size must be positive"));
    }
    let origin = |corner: &str, center: &str| -> Result<f64> {
        match (fields.contains_key(corner), fields.contains_key(center)) {
            (true, false) => number(corner),
            (false, true) => Ok(number(center)? - cell / 2.0),
            _ => Err(invalid("grid needs one origin per axis (corner or center)")),
        }
    };
    let x = origin("xllcorner", "xllcenter")?;
    let y = origin("yllcorner", "yllcenter")? + height as f64 * cell;
    if !x.is_finite() || !y.is_finite() || !(x + width as f64 * cell).is_finite() {
        return Err(invalid("grid extent is not finite"));
    }
    let nodata = if fields.contains_key("nodata_value") {
        number("nodata_value")?
    } else {
        -9999.0
    };
    let big_endian = match fields
        .get("byteorder")
        .map(|v| v.to_ascii_uppercase())
        .as_deref()
    {
        None | Some("LSBFIRST") => false,
        Some("MSBFIRST") => true,
        _ => return Err(invalid("unsupported byte order")),
    };
    Ok((
        Header {
            width,
            height,
            transform: [x, cell, 0.0, y, 0.0, -cell],
            nodata,
            big_endian,
        },
        tokens,
    ))
}

fn raster(h: Header, data_type: DataType, values: Vec<f64>, nodata: f64) -> Raster {
    Raster {
        width: h.width,
        height: h.height,
        data_type,
        transform: h.transform,
        nodata: Some(nodata),
        // These legacy tools ignore projection sidecars, as ArcGIS does.
        epsg: None,
        statistics: None,
        bands: vec![values],
    }
}

pub fn ascii_to_raster(path: &Path, integer: bool) -> Result<Raster> {
    let text = fs::read_to_string(path).map_err(|e| Error::io(path, e))?;
    let (h, mut tokens) = header(&text)?;
    let data_type = if integer {
        DataType::I32
    } else {
        DataType::F32
    };
    let output_nodata = if integer {
        -2147483647.0
    } else {
        f64::from(f32::MIN)
    };
    // Grow with the actual data; an invalid huge header must not force allocation.
    let mut values = Vec::new();
    for _ in 0..h.width * h.height {
        let value: f64 = tokens
            .next()
            .ok_or_else(|| invalid("not enough grid values"))?
            .parse()
            .map_err(|_| invalid("invalid grid value"))?;
        if !value.is_finite() {
            return Err(invalid("non-finite grid values are unsupported"));
        }
        if value == h.nodata {
            values.push(output_nodata);
        } else if integer {
            let value = value.trunc();
            if !(f64::from(i32::MIN)..=f64::from(i32::MAX)).contains(&value) {
                return Err(invalid("grid value exceeds signed 32-bit range"));
            }
            values.push(value);
        } else {
            let value = data_type.store(value);
            if !value.is_finite() {
                return Err(invalid("grid value exceeds float32 range"));
            }
            values.push(value);
        }
    }
    // Pro 3.7 ignores trailing values; short data errors with 010067.
    Ok(raster(h, data_type, values, output_nodata))
}

pub fn float_to_raster(path: &Path) -> Result<Raster> {
    if !path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("flt"))
    {
        return Err(invalid("Float To Raster requires a .flt file"));
    }
    let lower = path.with_extension("hdr");
    let upper = path.with_extension("HDR");
    let header_path = if lower.is_file() {
        lower
    } else if upper.is_file() {
        upper
    } else {
        lower
    };
    let text = fs::read_to_string(&header_path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            Error::Unsupported(format!(
                "ERROR 010062: header {} cannot be found",
                header_path.display()
            ))
        } else {
            Error::io(&header_path, e)
        }
    })?;
    let (h, remaining) = header(&text)?;
    if remaining.count() != 0 {
        return Err(invalid("unsupported float header field"));
    }
    let input_nodata = h.nodata as f32;
    if !input_nodata.is_finite() {
        return Err(invalid("float header NoData exceeds float32 range"));
    }
    let file = File::open(path).map_err(|e| Error::io(path, e))?;
    let count = h.width * h.height;
    let need = (count as u64)
        .checked_mul(4)
        .ok_or_else(|| invalid("float grid is too large"))?;
    if file.metadata().map_err(|e| Error::io(path, e))?.len() < need {
        return Err(invalid("not enough binary grid values"));
    }
    let mut reader = BufReader::new(file);
    let mut values = Vec::new();
    for _ in 0..count {
        let mut bytes = [0u8; 4];
        reader
            .read_exact(&mut bytes)
            .map_err(|e| Error::io(path, e))?;
        let value = if h.big_endian {
            f32::from_be_bytes(bytes)
        } else {
            f32::from_le_bytes(bytes)
        };
        if value == input_nodata {
            values.push(f64::from(f32::MIN));
        } else if value.is_finite() {
            values.push(f64::from(value));
        } else {
            return Err(invalid("non-finite binary grid values are unsupported"));
        }
    }
    Ok(raster(h, DataType::F32, values, f64::from(f32::MIN)))
}
