//! Spatial Analyst Cost Distance, Cost Back Link and Cost Allocation.
//!
//! Matched to ArcGIS Pro 3.7 (`testdata/arcgis-reference/cdist_*`, `cback_*`, `calloc_*`):
//! - The accumulated cost of a move between neighbouring cells is the cell size (times the square
//!   root of two for a diagonal move) times the mean of the two cells' costs. Source cells cost 0;
//!   a cell with NoData cost cannot be crossed, and a source on one is not used. With a maximum
//!   distance, cells whose accumulated cost is higher are NoData.
//! - The back link is the direction of the neighbour a cell's cheapest path comes from, numbered
//!   clockwise from east: 1 east, 2 south-east, 3 south, 4 south-west, 5 west, 6 north-west,
//!   7 north, 8 north-east; sources are 0. Among equally cheap paths (compared in 32-bit floats) the first to reach the cell wins; cells of equal cost leave the queue last row first.
//! - Cost Allocation gives each cell the value of the source its cheapest path starts from.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use crate::{DataType, Raster};

/// Neighbours as (row, column) steps in back link order: the back link code of a cell is
/// the index (plus one) of the step that leads back to the cell's predecessor.
const STEPS: [(isize, isize); 8] = [
    (0, 1),
    (1, 1),
    (1, 0),
    (1, -1),
    (0, -1),
    (-1, -1),
    (-1, 0),
    (-1, 1),
];

/// What a cost distance run leaves for every cell.
pub struct CostResult {
    /// Accumulated cost, `None` where a cell was not reached.
    pub distance: Vec<Option<f64>>,
    /// Back link codes (0 on sources), meaningful where `distance` is set.
    pub back_link: Vec<u8>,
    /// The value of the source each cell is allocated to.
    pub source_value: Vec<f64>,
    /// Sources that lie on cells without a cost (ArcGIS warns 010780).
    pub masked_sources: usize,
}

#[derive(PartialEq)]
struct Entry(f32, usize, usize);

impl Eq for Entry {}

impl Ord for Entry {
    fn cmp(&self, other: &Self) -> Ordering {
        // BinaryHeap is a max-heap: smaller cost first; among equal costs the cell
        // later in row order comes first (ArcGIS keeps the first path that reaches a cell).
        other
            .0
            .total_cmp(&self.0)
            .then_with(|| self.1.cmp(&other.1))
    }
}

impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Cost Distance from the cells of `source` that have values, over `cost`; the rasters share a grid.
/// Accumulates in 32-bit floats and keeps the first of equally cheap paths.
pub fn cost_distance(
    source: &Raster,
    cost: &Raster,
    max_distance: Option<f64>,
) -> Option<CostResult> {
    if (source.width, source.height) != (cost.width, cost.height) {
        return None;
    }
    let (w, h) = (cost.width, cost.height);
    let (cw, ch) = (cost.cell_width() as f32, cost.cell_height() as f32);
    let diagonal = (cost.cell_width().hypot(cost.cell_height())) as f32;
    let mut distance = vec![f32::INFINITY; w * h];
    let mut back_link = vec![0u8; w * h];
    let mut source_value = vec![0.0; w * h];
    let mut heap = BinaryHeap::new();
    let mut masked = 0;
    for i in 0..w * h {
        if source.is_nodata(source.bands[0][i]) {
            continue;
        }
        if cost.is_nodata(cost.bands[0][i]) {
            masked += 1;
            continue;
        }
        distance[i] = 0.0;
        source_value[i] = source.bands[0][i];
        heap.push(Entry(0.0, i, 0));
    }
    let max = max_distance.map(|m| m as f32);
    let mut seq = 0usize;
    while let Some(Entry(d, i, _)) = heap.pop() {
        if d > distance[i] {
            continue;
        }
        let (row, col) = ((i / w) as isize, (i % w) as isize);
        let here = cost.bands[0][i] as f32;
        for (k, &(dr, dc)) in STEPS.iter().enumerate() {
            let (r, c) = (row + dr, col + dc);
            if r < 0 || c < 0 || r >= h as isize || c >= w as isize {
                continue;
            }
            let j = r as usize * w + c as usize;
            if cost.is_nodata(cost.bands[0][j]) {
                continue;
            }
            let there = cost.bands[0][j] as f32;
            let length = if dr != 0 && dc != 0 {
                diagonal
            } else if dr != 0 {
                ch
            } else {
                cw
            };
            let nd = d + length * ((here + there) / 2.0);
            if max.is_some_and(|m| nd > m) {
                continue;
            }
            if nd < distance[j] {
                distance[j] = nd;
                // The move came from the neighbour on the opposite side.
                back_link[j] = ((k + 4) % 8 + 1) as u8;
                source_value[j] = source_value[i];
                seq += 1;
                heap.push(Entry(nd, j, seq));
            }
        }
    }
    Some(CostResult {
        distance: distance
            .iter()
            .map(|d| d.is_finite().then_some(*d as f64))
            .collect(),
        back_link,
        source_value,
        masked_sources: masked,
    })
}

/// The accumulated cost raster (32-bit float, NoData 3.4e38).
pub fn distance_raster(like: &Raster, r: &CostResult) -> Raster {
    let nodata = 3.4e38_f32 as f64;
    let mut out = like.like(DataType::F32);
    out.nodata = Some(nodata);
    out.bands[0] = r
        .distance
        .iter()
        .map(|d| d.map_or(nodata, |d| DataType::F32.store(d)))
        .collect();
    out
}

/// The back link raster (16-bit integer, NoData 32767).
pub fn back_link_raster(like: &Raster, r: &CostResult) -> Raster {
    let mut out = like.like(DataType::I16);
    out.nodata = Some(32767.0);
    out.bands[0] = r
        .distance
        .iter()
        .zip(&r.back_link)
        .map(|(d, b)| if d.is_some() { *b as f64 } else { 32767.0 })
        .collect();
    out
}

/// The allocation raster: unsigned 8-bit with NoData 0 when the values run from 1 to 255,
/// else ArcGIS's usual choice for the values present.
pub fn allocation_raster(like: &Raster, source: &Raster, r: &CostResult) -> Raster {
    let cells: Vec<Option<f64>> = r
        .distance
        .iter()
        .zip(&r.source_value)
        .map(|(d, v)| d.map(|_| *v))
        .collect();
    let (t, nodata) = if source.data_type.is_float() {
        (DataType::F32, f32::MIN as f64)
    } else {
        let (min, max) = cells
            .iter()
            .flatten()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &v| {
                (a.min(v), b.max(v))
            });
        if min >= 1.0 && max <= 255.0 {
            (DataType::U8, 0.0)
        } else if min.is_finite() {
            crate::zonal::integer_output(
                min,
                max,
                matches!(source.data_type, DataType::U8 | DataType::I8),
            )
        } else {
            (DataType::U8, 0.0)
        }
    };
    let mut out = like.like(t);
    out.nodata = Some(nodata);
    out.bands[0] = cells.iter().map(|v| v.unwrap_or(nodata)).collect();
    out
}

/// Spatial Analyst Corridor: the sum of two accumulated cost rasters on the same grid (32-bit
/// float, NoData where either has none; ArcGIS's NoData is the lowest float).
pub fn corridor(a: &Raster, b: &Raster) -> Option<Raster> {
    if (a.width, a.height) != (b.width, b.height) {
        return None;
    }
    let nodata = f32::MIN as f64;
    let mut out = a.like(DataType::F32);
    out.nodata = Some(nodata);
    out.bands[0] = a.bands[0]
        .iter()
        .zip(&b.bands[0])
        .map(|(x, y)| {
            if a.is_nodata(*x) || b.is_nodata(*y) {
                nodata
            } else {
                DataType::F32.store(x + y)
            }
        })
        .collect();
    Some(out)
}
