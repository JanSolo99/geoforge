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
    let fallback = data_type.store(match data_type {
        DataType::F32 | DataType::F64 => 3.4e38,
        DataType::U8 => u8::MAX as f64,
        DataType::I8 => i8::MAX as f64,
        DataType::U16 => u16::MAX as f64,
        DataType::I16 => i16::MAX as f64,
        DataType::U32 => u32::MAX as f64,
        DataType::I32 => i32::MAX as f64,
    });
    // ArcGIS retains stored cell values and takes the global TIFF NoData from the
    // final input. If that input has none but earlier inputs do, use the default.
    // Consequently reversing inputs can expose an earlier band's old sentinel.
    let mut nodata = inputs
        .last()
        .and_then(|r| r.nodata)
        .map(|v| data_type.store(v));
    if nodata.is_none() && inputs.iter().any(|r| r.nodata.is_some()) {
        nodata = Some(fallback);
    }
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
                        // With no NoData tag on any input ArcGIS pads with zero,
                        // preserving both the absent tag and valid maximum values.
                        band.push(nodata.unwrap_or(0.0));
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
