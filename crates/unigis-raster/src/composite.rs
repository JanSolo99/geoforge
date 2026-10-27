//! Composite Bands, checked against saved ArcGIS Pro outputs.

use unigis_core::{Error, Result};

use crate::{DataType, MAX_CELLS, Raster};

fn unsupported(message: &str) -> Error {
    Error::Unsupported(format!("Composite Bands: {message}"))
}

pub fn composite_bands(inputs: &[Raster]) -> Result<Raster> {
    let first = inputs
        .first()
        .ok_or_else(|| unsupported("at least one input raster is required"))?;
    let mut band_count = 0usize;
    for raster in inputs {
        if raster.epsg != first.epsg {
            return Err(unsupported(
                "input coordinate systems must match; reprojection is not yet supported",
            ));
        }
        if raster.transform.iter().any(|v| !v.is_finite())
            || raster.transform[1] <= 0.0
            || raster.transform[5] >= 0.0
            || raster.transform[2] != 0.0
            || raster.transform[4] != 0.0
        {
            return Err(unsupported("requires finite north-up input grids"));
        }
        let cells = raster.width.checked_mul(raster.height);
        if cells.is_none_or(|n| n == 0 || raster.bands.iter().any(|b| b.len() != n))
            || raster.bands.is_empty()
        {
            return Err(unsupported("input raster has invalid dimensions or bands"));
        }
        band_count = band_count
            .checked_add(raster.bands.len())
            .ok_or_else(|| unsupported("too many bands"))?;
    }
    if (first.cell_width() - first.cell_height()).abs() > first.cell_width() * 1e-10 {
        return Err(unsupported("the first input must have square cells"));
    }
    if (first.width as u64)
        .checked_mul(first.height as u64)
        .and_then(|n| n.checked_mul(band_count as u64))
        .is_none_or(|n| n > MAX_CELLS)
    {
        return Err(unsupported("output has too many cells across its bands"));
    }
    let has = |t| inputs.iter().any(|r| r.data_type == t);
    let signed_bits = if has(DataType::I32) {
        32
    } else if has(DataType::I16) {
        16
    } else if has(DataType::I8) {
        8
    } else {
        0
    };
    let unsigned_bits = if has(DataType::U32) {
        32
    } else if has(DataType::U16) {
        16
    } else if has(DataType::U8) {
        8
    } else {
        0
    };
    let data_type = if has(DataType::F64)
        || (has(DataType::F32) && (signed_bits == 32 || unsigned_bits == 32))
        || (signed_bits > 0 && unsigned_bits == 32)
    {
        DataType::F64
    } else if has(DataType::F32) {
        DataType::F32
    } else if signed_bits == 32 || (signed_bits > 0 && unsigned_bits == 16) {
        DataType::I32
    } else if signed_bits == 16 || (signed_bits > 0 && unsigned_bits == 8) {
        DataType::I16
    } else if signed_bits == 8 {
        DataType::I8
    } else if unsigned_bits == 32 {
        DataType::U32
    } else if unsigned_bits == 16 {
        DataType::U16
    } else {
        DataType::U8
    };
    // ArcGIS chooses the highest *tagged input's* pixel-type rank for its
    // default sentinel, independently of output promotion. Signed follows
    // unsigned at each width; e.g. tagged I16 + U16 chooses I16's default.
    let fallback_type = inputs
        .iter()
        .filter(|r| r.nodata.is_some())
        .max_by_key(|r| match r.data_type {
            DataType::U8 => 0,
            DataType::I8 => 1,
            DataType::U16 => 2,
            DataType::I16 => 3,
            DataType::U32 => 4,
            DataType::I32 => 5,
            DataType::F32 => 6,
            DataType::F64 => 7,
        })
        .map(|r| r.data_type);
    let fallback = fallback_type.map(|t| match t {
        DataType::F32 => f64::from(3.4e38_f32),
        DataType::F64 => 1.79e308,
        // ArcGIS can write an integer NoData tag outside the pixel range.
        // Preserve those tags rather than clamping them to a valid cell value.
        DataType::U8 | DataType::I8 => 256.0,
        DataType::U16 => 65536.0,
        DataType::I16 => i16::MAX as f64,
        DataType::U32 | DataType::I32 => i32::MAX as f64,
    });
    // ArcGIS retains stored cell values and takes the global TIFF NoData from the
    // final input. If that input has none but earlier inputs do, use the default.
    // Consequently reversing inputs can expose an earlier band's old sentinel.
    let last = inputs.last().expect("nonempty inputs were validated");
    let nodata = last
        .nodata
        .map(|v| {
            if last.data_type == DataType::U32
                && data_type == DataType::F64
                && (0.0..=f64::from(u32::MAX)).contains(&v)
                && v.fract() == 0.0
            {
                // ArcGIS exports the U32 sentinel through a signed 32-bit value
                // when promoting its cells to F64 (2^31 becomes -2^31).
                f64::from(v as u32 as i32)
            } else if data_type == DataType::F32 {
                f64::from(v as f32)
            } else {
                v
            }
        })
        .or(fallback);
    let mut bands = Vec::with_capacity(band_count);
    for raster in inputs {
        for source in &raster.bands {
            let mut band = Vec::with_capacity(first.width * first.height);
            for row in 0..first.height {
                let y = first.transform[3] + (row as f64 + 0.5) * first.transform[5];
                let source_row = ((y - raster.transform[3]) / raster.transform[5]).floor();
                for col in 0..first.width {
                    let x = first.transform[0] + (col as f64 + 0.5) * first.transform[1];
                    let source_col = ((x - raster.transform[0]) / raster.transform[1]).floor();
                    if source_col >= 0.0
                        && source_col < raster.width as f64
                        && source_row >= 0.0
                        && source_row < raster.height as f64
                    {
                        band.push(data_type.store(
                            source[source_row as usize * raster.width + source_col as usize],
                        ));
                    } else {
                        // Padding follows this source's sentinel (or zero), even
                        // when the final TIFF uses a different global NoData tag.
                        band.push(raster.nodata.unwrap_or(0.0));
                    }
                }
            }
            bands.push(band);
        }
    }
    Ok(Raster {
        width: first.width,
        height: first.height,
        data_type,
        transform: first.transform,
        nodata,
        epsg: first.epsg,
        statistics: None,
        bands,
    })
}
