//! Cell-by-cell frequency comparisons, checked against ArcGIS Pro reference rasters.
use crate::{DataType, MAX_CELLS, Raster};
use unigis_core::{Error, Result};

#[derive(Clone, Copy)]
pub enum Comparison {
    Equal,
    Greater,
    Less,
}

fn unsupported(message: &str) -> Error {
    Error::Unsupported(format!("Frequency: {message}"))
}

/// Counts comparison bands (SINGLE_BAND) or corresponding bands (MULTI_BAND).
/// The supported subset uses matching grids/CRS and at most 256 comparisons.
pub fn frequency(
    value: &Raster,
    rasters: &[Raster],
    comparison: Comparison,
    multiband: bool,
) -> Result<Raster> {
    if rasters.is_empty() {
        return Err(unsupported("at least one comparison raster is required"));
    }
    let cells = value
        .width
        .checked_mul(value.height)
        .filter(|n| *n > 0)
        .ok_or_else(|| unsupported("invalid grid dimensions"))?;
    for raster in std::iter::once(value).chain(rasters) {
        if (raster.width, raster.height, raster.transform, raster.epsg)
            != (value.width, value.height, value.transform, value.epsg)
        {
            return Err(unsupported(
                "requires matching grids and coordinate systems; resampling/reprojection is not supported",
            ));
        }
        if raster.transform.iter().any(|v| !v.is_finite())
            || raster.bands.is_empty()
            || raster.bands.iter().any(|b| b.len() != cells)
        {
            return Err(unsupported("invalid grid or band dimensions"));
        }
        if raster
            .bands
            .iter()
            .flatten()
            .any(|&v| !v.is_finite() && !raster.is_nodata(v))
        {
            return Err(unsupported("requires finite values or NoData"));
        }
    }
    let comparisons = if multiband {
        rasters.len()
    } else {
        rasters.iter().map(|r| r.bands.len()).sum()
    };
    let (data_type, nodata) = match comparisons {
        1 | 3 | 15 => (DataType::U8, 16.),
        2 => (DataType::U8, 3.),
        4..=14 => (DataType::U8, 15.),
        16..=127 => (DataType::I8, -128.),
        128..=254 => (DataType::U8, 255.),
        255 => (DataType::U8, 256.),
        256 => (DataType::U16, 65535.),
        _ => return Err(unsupported("supports at most 256 comparisons")),
    };
    let bands = if !multiband {
        1
    } else if value.bands.len() > 1 {
        value.bands.len()
    } else {
        rasters.iter().map(|r| r.bands.len()).max().unwrap_or(1)
    };
    if (cells as u64)
        .checked_mul(bands as u64)
        .is_none_or(|n| n > MAX_CELLS)
    {
        return Err(unsupported("output has too many cells across its bands"));
    }
    let mut output = value.like(data_type);
    output.nodata = Some(nodata);
    output.bands = vec![vec![nodata; cells]; bands];
    for (index, output_band) in output.bands.iter_mut().enumerate() {
        let value_band = &value.bands[if !multiband || value.bands.len() == 1 {
            0
        } else {
            index
        }];
        // Missing comparison bands produce NoData; they are never repeated.
        let comparison_bands: Vec<_> = rasters
            .iter()
            .flat_map(|raster| {
                raster
                    .bands
                    .iter()
                    .enumerate()
                    .filter(move |(i, _)| !multiband || *i == index)
                    .map(move |(_, band)| (raster, band))
            })
            .collect();
        if multiband && comparison_bands.len() != rasters.len() {
            continue;
        }
        for (cell, result) in output_band.iter_mut().enumerate() {
            let target = value_band[cell];
            if value.is_nodata(target) {
                continue;
            }
            let mut count = 0;
            let mut valid = true;
            for (raster, band) in &comparison_bands {
                let v = band[cell];
                if raster.is_nodata(v) {
                    valid = false;
                    break;
                }
                count += usize::from(match comparison {
                    Comparison::Equal => v == target,
                    Comparison::Greater => v > target,
                    Comparison::Less => v < target,
                });
            }
            if valid {
                *result = count as f64;
            }
        }
    }
    Ok(output)
}
