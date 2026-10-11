//! The `.sbn`/`.sbx` spatial index ArcGIS writes next to a shapefile. Worked out
//! from ArcGIS Pro 3.7 output (black box: the files it writes) and shapelib's
//! public reader (`sbnsearch.c`); see `docs/formats/shapefile-arcgis.md`.
//!
//! - Header (100 bytes, like `.shp`): file code 9994, then -400, file length in
//!   16-bit words at 24, record count (null shapes included) at 28, the extent as
//!   big-endian doubles at 32.
//! - Record 1, the bin index: one (first bin record, feature count) pair per tree
//!   node, up to the last node holding features; empty nodes are (-1, 0).
//! - Records 2...: bins of at most 100 features, a node's bins in a row (full ones
//!   first), 8 bytes per feature: its box in bytes (xmin, ymin, xmax, ymax) and its
//!   record number (big-endian).
//! - `.sbx`: the same header, then (offset, length) in words of each record.
//!
//! A box is quantized to the extent as `floor((v - min) * (255 / (max - min)))`,
//! the maximum plus one (both capped at 255). The tree is a binary space
//! partition in that byte space: node `k` has children `2k` (upper half) and
//! `2k + 1` (lower half), splitting x at odd levels and y at even ones; a feature
//! goes to the upper child if its minimum is above the split, to the lower one if
//! its maximum is at most the split, and otherwise stays. The depth is the
//! smallest `L >= 2` with `n <= 8 (2^L - 1)` for `n` indexed features.
//!
//! Below the root, a node whose subtree holds at most 8 features keeps them all.
//! With that this matches ArcGIS byte for byte on all reference outputs; on
//! uniformly spread points ArcGIS still keeps about 0.15% of features higher up
//! (both are valid for searching).

use std::path::Path;

use geoforge_core::{Error, Result};

/// One indexed feature: its 1-based record number and box.
#[derive(Debug, Clone, Copy)]
pub struct IndexEntry {
    pub record: u32,
    pub bbox: [f64; 4],
}

const BIN_CAPACITY: usize = 100;
const MAX_LEVELS: u32 = 16;

fn levels(n: usize) -> u32 {
    let mut l = 2;
    while l < MAX_LEVELS && n as u64 > 8 * ((1u64 << l) - 1) {
        l += 1;
    }
    l
}

/// The byte box of `bbox` within `extent`.
fn quantize(bbox: [f64; 4], extent: [f64; 4]) -> [u8; 4] {
    let scale = |lo: f64, hi: f64| if hi > lo { 255.0 / (hi - lo) } else { 0.0 };
    let (sx, sy) = (scale(extent[0], extent[2]), scale(extent[1], extent[3]));
    let q = |v: f64, lo: f64, s: f64| ((v - lo) * s).floor().clamp(0.0, 255.0) as u32;
    let x0 = q(bbox[0], extent[0], sx);
    let y0 = q(bbox[1], extent[1], sy);
    let x1 = (q(bbox[2], extent[0], sx) + 1).min(255);
    let y1 = (q(bbox[3], extent[1], sy) + 1).min(255);
    [x0 as u8, y0 as u8, x1 as u8, y1 as u8]
}

/// The node a byte box belongs to.
fn node_of(q: [u8; 4], levels: u32) -> usize {
    let (mut lo, mut hi) = ([0u32; 2], [256u32; 2]);
    let mut node = 1usize;
    for level in 1..levels {
        let ax = ((level - 1) % 2) as usize;
        let mid = (lo[ax] + hi[ax]) / 2;
        let (a, b) = (u32::from(q[ax]), u32::from(q[ax + 2]));
        if a > mid {
            node *= 2;
            lo[ax] = mid;
        } else if b <= mid {
            node = 2 * node + 1;
            hi[ax] = mid;
        } else {
            break;
        }
    }
    node
}

/// ArcGIS keeps a small subtree in one node: below the root, a node whose subtree
/// holds at most 8 features takes its descendants' features (after its own, in
/// node order). This reproduces every reference index; ArcGIS still differs on a
/// few uniformly spread points.
fn pull_up_small_subtrees(nodes: &mut [Vec<(u32, [u8; 4])>]) {
    let n = nodes.len();
    let mut subtree = vec![0usize; n];
    for k in (1..n).rev() {
        subtree[k] = nodes[k].len()
            + if 2 * k < n { subtree[2 * k] } else { 0 }
            + if 2 * k + 1 < n { subtree[2 * k + 1] } else { 0 };
    }
    let mut k = 2;
    while k < n {
        if subtree[k] <= 8 && subtree[k] > nodes[k].len() {
            // Descendants in node order: each level of the subtree in turn.
            let mut level = vec![2 * k, 2 * k + 1];
            while level.first().is_some_and(|&d| d < n) {
                for &d in &level {
                    let moved = std::mem::take(&mut nodes[d]);
                    nodes[k].extend(moved);
                }
                level = level.iter().flat_map(|&d| [2 * d, 2 * d + 1]).collect();
            }
        }
        k += 1;
    }
}

fn header(out: &mut Vec<u8>, length_words: usize, records: usize, extent: [f64; 4]) {
    out.extend_from_slice(&9994i32.to_be_bytes());
    out.extend_from_slice(&(-400i32).to_be_bytes());
    out.extend_from_slice(&[0; 16]);
    out.extend_from_slice(&(length_words as i32).to_be_bytes());
    out.extend_from_slice(&(records as i32).to_be_bytes());
    for v in extent {
        out.extend_from_slice(&v.to_be_bytes());
    }
    out.extend_from_slice(&[0; 36]);
}

/// The `.sbn` and `.sbx` contents for a shapefile of `records` records (nulls
/// included) with `extent` (its header box), indexing `entries`.
pub fn build(records: usize, extent: [f64; 4], entries: &[IndexEntry]) -> (Vec<u8>, Vec<u8>) {
    // An empty shapefile's header box is (0, 0, -1, -1); ArcGIS writes NaN then.
    let extent = if extent[2] < extent[0] || extent[3] < extent[1] {
        [f64::from_bits(0x7ff8_0000_0000_0001); 4]
    } else {
        extent
    };
    let levels = levels(entries.len());
    // Features per node, in record order.
    let mut nodes: Vec<Vec<(u32, [u8; 4])>> = vec![Vec::new(); 1 << levels];
    let mut sorted = entries.to_vec();
    sorted.sort_by_key(|e| e.record);
    for e in &sorted {
        let q = quantize(e.bbox, extent);
        nodes[node_of(q, levels)].push((e.record, q));
    }
    pull_up_small_subtrees(&mut nodes);
    let last = nodes.iter().rposition(|n| !n.is_empty()).unwrap_or(0);
    // Records: 1 = bin index, then the bins.
    let mut index = Vec::new();
    let mut bins: Vec<Vec<u8>> = Vec::new();
    for feats in &nodes[1..=last] {
        if feats.is_empty() {
            index.extend_from_slice(&(-1i32).to_be_bytes());
            index.extend_from_slice(&0i32.to_be_bytes());
            continue;
        }
        index.extend_from_slice(&((bins.len() + 2) as i32).to_be_bytes());
        index.extend_from_slice(&(feats.len() as i32).to_be_bytes());
        for chunk in feats.chunks(BIN_CAPACITY) {
            let mut bin = Vec::with_capacity(chunk.len() * 8);
            for (record, q) in chunk {
                bin.extend_from_slice(q);
                bin.extend_from_slice(&(*record as i32).to_be_bytes());
            }
            bins.push(bin);
        }
    }
    if last == 0 {
        index.clear();
    }
    let contents: Vec<&[u8]> = std::iter::once(index.as_slice())
        .chain(bins.iter().map(Vec::as_slice))
        .collect();
    let sbn_words = 50 + contents.iter().map(|c| 4 + c.len() / 2).sum::<usize>();
    let mut sbn = Vec::with_capacity(sbn_words * 2);
    header(&mut sbn, sbn_words, records, extent);
    let mut sbx = Vec::new();
    header(&mut sbx, 50 + 4 * contents.len(), records, extent);
    for (i, c) in contents.iter().enumerate() {
        sbx.extend_from_slice(&((sbn.len() / 2) as i32).to_be_bytes());
        sbx.extend_from_slice(&((c.len() / 2) as i32).to_be_bytes());
        sbn.extend_from_slice(&(i as i32 + 1).to_be_bytes());
        sbn.extend_from_slice(&((c.len() / 2) as i32).to_be_bytes());
        sbn.extend_from_slice(c);
    }
    (sbn, sbx)
}

/// The record count, header extent and record boxes of a `.shp` file's bytes
/// (null and empty shapes are not indexed).
pub fn entries_from_shp(shp: &[u8]) -> Result<(usize, [f64; 4], Vec<IndexEntry>)> {
    let bad = || Error::format("shp", "truncated record");
    let f64_at = |o: usize| -> Result<f64> {
        Ok(f64::from_le_bytes(
            shp.get(o..o + 8).ok_or_else(bad)?.try_into().unwrap(),
        ))
    };
    let extent = [f64_at(36)?, f64_at(44)?, f64_at(52)?, f64_at(60)?];
    let mut entries = Vec::new();
    let (mut o, mut records) = (100usize, 0usize);
    while o + 8 <= shp.len() {
        let words = i32::from_be_bytes(shp[o + 4..o + 8].try_into().unwrap()) as usize;
        let content = shp.get(o + 8..o + 8 + 2 * words).ok_or_else(bad)?;
        records += 1;
        let kind = i32::from_le_bytes(content.get(..4).ok_or_else(bad)?.try_into().unwrap());
        let le = |k: usize| f64::from_le_bytes(content[k..k + 8].try_into().unwrap());
        let bbox = match kind {
            0 => None,
            1 | 11 | 21 if content.len() >= 20 => Some([le(4), le(12), le(4), le(12)]),
            _ if content.len() >= 36 => Some([le(4), le(12), le(20), le(28)]),
            _ => None,
        };
        if let Some(b) = bbox.filter(|b| b.iter().all(|v| v.is_finite())) {
            entries.push(IndexEntry {
                record: records as u32,
                bbox: b,
            });
        }
        o += 8 + 2 * words;
    }
    Ok((records, extent, entries))
}

/// Writes `<base>.sbn` and `<base>.sbx`.
pub fn write(base: &Path, records: usize, extent: [f64; 4], entries: &[IndexEntry]) -> Result<()> {
    let (sbn, sbx) = build(records, extent, entries);
    for (ext, bytes) in [("sbn", sbn), ("sbx", sbx)] {
        let p = base.with_extension(ext);
        std::fs::write(&p, bytes).map_err(|e| Error::io(&p, e))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depth_and_quantization() {
        assert_eq!(levels(1), 2);
        assert_eq!(levels(24), 2);
        assert_eq!(levels(25), 3);
        assert_eq!(levels(56), 3);
        assert_eq!(levels(57), 4);
        assert_eq!(levels(5000), 10);
        let ext = [0.0, 0.0, 100.0, 100.0];
        assert_eq!(quantize([0.0, 0.0, 0.0, 0.0], ext), [0, 0, 1, 1]);
        // 100 × (255 / 100) is 254.99999999999997 in doubles: ArcGIS gets 254 too.
        assert_eq!(
            quantize([100.0, 50.0, 100.0, 50.0], ext),
            [254, 127, 255, 128]
        );
        assert_eq!(
            quantize([0.0, 0.0, 200.0, 50.0], [0.0, 0.0, 200.0, 100.0]),
            [0, 0, 255, 128]
        );
        assert_eq!(node_of([254, 127, 255, 128], 2), 2);
        assert_eq!(node_of([10, 10, 11, 11], 2), 3);
        assert_eq!(node_of([127, 0, 129, 1], 2), 1);
    }

    #[test]
    fn empty_index_is_header_and_empty_bin_index() {
        let (sbn, sbx) = build(0, [0.0; 4], &[]);
        assert_eq!(sbn.len(), 108);
        let (nan, _) = build(0, [0.0, 0.0, -1.0, -1.0], &[]);
        assert_eq!(&nan[32..40], &0x7ff8_0000_0000_0001u64.to_be_bytes());
        assert_eq!(&sbn[100..], &[0, 0, 0, 1, 0, 0, 0, 0]);
        assert_eq!(&sbx[100..], &[0, 0, 0, 50, 0, 0, 0, 0]);
    }
}
