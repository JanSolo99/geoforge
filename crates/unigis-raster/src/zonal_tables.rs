//! Spatial Analyst Zonal Histogram and Tabulate Area (tables of cell counts and areas per zone).
//!
//! Matched to ArcGIS Pro 3.7 (`testdata/arcgis-reference/zh_*`, `ta_*`):
//! - Both tools count cells of a processing grid that pairs a zone with a value (class) cell. The
//!   cell size is the larger of the two rasters' (Tabulate Area also takes one). When the value
//!   raster's cells are as large as the zones' or larger, the grid is the value raster's (its
//!   corner and extent) and both rasters are read at the cell centers. When the value raster is
//!   finer, the grid starts at the zone raster's corner, the zones are read at the cell centers
//!   and the values at the cell's left edge, three quarters of the way down (the one rule that
//!   fits the single pair observed).
//! - Zonal Histogram has a row per value (the label is the value) and a column per zone, named
//!   after the zone field and the zone value. Float value rasters are binned by ArcGIS into 256
//!   classes, which is not done.
//! - Tabulate Area has a row per zone and a column `VALUE_<class>` per class holding the area, or
//!   with `classes_as_rows` a row per zone and class with `Count` and `Area`.

use std::collections::{BTreeMap, BTreeSet};

use unigis_core::{Field, FieldType, Value};

use crate::Raster;

/// Cell pairs (zone, value) of the processing grid and the grid's cell size.
pub struct Paired {
    pub cell: f64,
    pub pairs: Vec<(Option<i64>, Option<f64>)>,
}

fn sample(r: &Raster, x: f64, y: f64) -> Option<f64> {
    let t = &r.transform;
    let col = ((x - t[0]) / t[1]).floor();
    let row = ((y - t[3]) / t[5]).floor();
    if col < 0.0 || row < 0.0 || col >= r.width as f64 || row >= r.height as f64 {
        return None;
    }
    r.get(col as usize, row as usize)
}

/// Pairs the cells of `zones` and `values` on the processing grid.
pub fn pair_cells(zones: &Raster, values: &Raster, cell: Option<f64>) -> Option<Paired> {
    let cell = cell
        .filter(|c| *c > 0.0)
        .unwrap_or_else(|| zones.cell_width().max(values.cell_width()));
    let on_values = values.cell_width() >= zones.cell_width();
    let base = if on_values { values } else { zones };
    let (x0, y0) = (base.transform[0], base.transform[3]);
    let cols = (base.width as f64 * base.cell_width() / cell - 1e-9).ceil() as usize;
    let rows = (base.height as f64 * base.cell_height() / cell - 1e-9).ceil() as usize;
    let mut pairs = Vec::with_capacity(cols * rows);
    for r in 0..rows {
        for c in 0..cols {
            let (left, top) = (x0 + c as f64 * cell, y0 - r as f64 * cell);
            let zone = sample(zones, left + cell / 2.0, top - cell / 2.0).map(|v| v.round() as i64);
            let value = if on_values {
                sample(values, left + cell / 2.0, top - cell / 2.0)
            } else {
                sample(values, left, top - 0.75 * cell)
            };
            pairs.push((zone, value));
        }
    }
    Some(Paired { cell, pairs })
}

fn pair_counts(p: &Paired) -> BTreeMap<(i64, i64), usize> {
    let mut counts = BTreeMap::new();
    for pair in &p.pairs {
        if let (Some(z), Some(v)) = *pair {
            *counts.entry((z, v.round() as i64)).or_insert(0) += 1;
        }
    }
    counts
}

/// A dbf-safe column name: other characters than letters, digits and `_` become `_`, at most 10
/// characters.
pub fn column_name(prefix: &str, id: i64) -> String {
    format!("{prefix}_{id}")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(10)
        .collect()
}

/// Zonal Histogram: a row per value and a column per zone; `prefix` names the zone columns.
pub fn zonal_histogram(
    zones: &Raster,
    values: &Raster,
    prefix: &str,
) -> Result<(Vec<Field>, Vec<Vec<Value>>), String> {
    let paired = pair_cells(zones, values, None).ok_or("the rasters do not overlap")?;
    let zone_ids: BTreeSet<i64> = paired.pairs.iter().filter_map(|p| p.0).collect();
    // Float values are put into 256 equal intervals from the smallest to the largest, labelled
    // 1 to 256; integer values are their own classes.
    let float = values.data_type.is_float();
    let (lo, hi) = paired
        .pairs
        .iter()
        .filter_map(|p| p.0.and(p.1))
        .fold((f64::MAX, f64::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)));
    let class_of = |v: f64| -> i64 {
        if float {
            let width = hi - lo;
            let bin = if width > 0.0 {
                ((v - lo) / width * 256.0).floor().clamp(0.0, 255.0) as i64
            } else {
                0
            };
            bin + 1
        } else {
            v.round() as i64
        }
    };
    let mut counts: BTreeMap<(i64, i64), usize> = BTreeMap::new();
    for pair in &paired.pairs {
        if let (Some(z), Some(v)) = *pair {
            *counts.entry((z, class_of(v))).or_insert(0) += 1;
        }
    }
    let value_ids: BTreeSet<i64> = if float {
        (1..=256).collect()
    } else {
        paired
            .pairs
            .iter()
            .filter_map(|p| p.1.map(|v| v.round() as i64))
            .collect()
    };
    let mut label = Field::new("LABEL", FieldType::String);
    label.length = 8;
    let mut fields = vec![label];
    fields.extend(
        zone_ids
            .iter()
            .map(|z| Field::new(column_name(prefix, *z), FieldType::Integer)),
    );
    let rows = value_ids
        .iter()
        .map(|v| {
            let mut row = vec![Value::String(v.to_string())];
            row.extend(
                zone_ids
                    .iter()
                    .map(|z| Value::Integer(counts.get(&(*z, *v)).copied().unwrap_or(0) as i32)),
            );
            row
        })
        .collect();
    Ok((fields, rows))
}

/// Tabulate Area. `zone_field` and `class_field` name the output columns of the rows layout.
pub fn tabulate_area(
    zones: &Raster,
    classes: &Raster,
    cell: Option<f64>,
    classes_as_rows: bool,
    zone_field: &str,
    class_field: &str,
) -> Result<(Vec<Field>, Vec<Vec<Value>>), String> {
    if classes.data_type.is_float() {
        return Err("ERROR 000969: The input raster is not integer type.".into());
    }
    let paired = pair_cells(zones, classes, cell).ok_or("the rasters do not overlap")?;
    let zone_ids: BTreeSet<i64> = paired.pairs.iter().filter_map(|p| p.0).collect();
    let class_ids: BTreeSet<i64> = paired
        .pairs
        .iter()
        .filter_map(|p| p.1.map(|v| v.round() as i64))
        .collect();
    let counts = pair_counts(&paired);
    let count = |z: i64, c: i64| counts.get(&(z, c)).copied().unwrap_or(0);
    let area = paired.cell * paired.cell;
    let real = |name: String| Field::new(name, FieldType::Double);
    if classes_as_rows {
        let second = if class_field.eq_ignore_ascii_case(zone_field) {
            format!("{zone_field}_1")
        } else {
            class_field.to_owned()
        };
        let fields = vec![
            Field::new(zone_field, FieldType::Integer),
            Field::new(second, FieldType::Integer),
            real("Count".into()),
            real("Area".into()),
        ];
        let mut rows = Vec::new();
        for z in &zone_ids {
            for c in &class_ids {
                let n = count(*z, *c) as f64;
                rows.push(vec![
                    Value::Integer(*z as i32),
                    Value::Integer(*c as i32),
                    Value::Double(n),
                    Value::Double(n * area),
                ]);
            }
        }
        Ok((fields, rows))
    } else {
        let mut fields = vec![Field::new("VALUE", FieldType::Integer)];
        fields.extend(class_ids.iter().map(|c| real(column_name("VALUE", *c))));
        let rows = zone_ids
            .iter()
            .map(|z| {
                let mut row = vec![Value::Integer(*z as i32)];
                row.extend(
                    class_ids
                        .iter()
                        .map(|c| Value::Double(count(*z, *c) as f64 * area)),
                );
                row
            })
            .collect();
        Ok((fields, rows))
    }
}

/// Storage Capacity's analysis options (without zones).
pub struct StorageCapacity {
    pub area: bool,
    pub volume: bool,
    pub min: Option<f64>,
    pub max: Option<f64>,
    /// `Ok(n)`: n increments; `Err(step)`: levels `step` apart.
    pub increments: std::result::Result<usize, f64>,
    /// Metres per unit of the surface's z values (volumes are in z units cubed).
    pub z_metres: f64,
}

/// Storage Capacity: for each elevation level, the area of the cells at or below it and the
/// volume between the level and those cells (ZONE_CODE 1 for the whole surface). Depths are
/// single precision (level − z), summed in double, as ArcGIS's volumes agree that way.
pub fn storage_capacity_table(
    surface: &Raster,
    options: &StorageCapacity,
) -> unigis_core::Result<(Vec<unigis_core::Field>, Vec<Vec<unigis_core::Value>>)> {
    use unigis_core::{Error, Field, FieldType, Value};
    let values: Vec<f32> = surface.bands[0]
        .iter()
        .filter(|v| !surface.is_nodata(**v))
        .map(|&v| v as f32)
        .collect();
    if values.is_empty() {
        return Err(Error::Unsupported("the surface has no data".into()));
    }
    let lo = options
        .min
        .unwrap_or_else(|| values.iter().fold(f64::INFINITY, |m, &v| m.min(v as f64)));
    let hi = options.max.unwrap_or_else(|| {
        values
            .iter()
            .fold(f64::NEG_INFINITY, |m, &v| m.max(v as f64))
    });
    let levels: Vec<f64> = match options.increments {
        Ok(n) => {
            let n = n.max(1);
            (0..=n)
                .map(|i| lo + (hi - lo) * i as f64 / n as f64)
                .collect()
        }
        Err(step) if step > 0.0 => {
            let mut v: Vec<f64> = (0..)
                .map(|i| lo + i as f64 * step)
                .take_while(|&h| h < hi)
                .collect();
            v.push(hi);
            v
        }
        Err(_) => return Err(Error::Unsupported("the increment must be positive".into())),
    };
    let cell_area = (surface.transform[1] * surface.transform[5]).abs();
    let volume_area = cell_area / (options.z_metres * options.z_metres);
    let rows = levels
        .iter()
        .map(|&h| {
            let level = h as f32;
            let (mut count, mut depth) = (0usize, 0.0f64);
            for &z in &values {
                if z <= level {
                    count += 1;
                    depth += f64::from(level - z);
                }
            }
            let mut row = vec![Value::Integer(1), Value::Double(h)];
            if options.area {
                row.push(Value::Double(count as f64 * cell_area));
            }
            if options.volume {
                row.push(Value::Double(depth * volume_area));
            }
            row
        })
        .collect();
    let mut zone = Field::new("ZONE_CODE", FieldType::Integer);
    zone.length = 10;
    zone.precision = 10;
    let mut fields = vec![zone, Field::new("ELEVATION", FieldType::Double)];
    if options.area {
        fields.push(Field::new("AREA", FieldType::Double));
    }
    if options.volume {
        fields.push(Field::new("VOLUME", FieldType::Double));
    }
    Ok((fields, rows))
}
