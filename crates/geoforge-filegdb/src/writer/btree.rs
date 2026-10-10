//! Builds version-1 index files (`.spx`, `.atx`): 4096-byte pages, root at page 1.

use std::path::Path;

use geoforge_core::{Envelope, Error, Result};

const PAGE: usize = 4096;
const LEAF_HEADER: usize = 12;
const INTERNAL_HEADER: usize = 8;

/// Writes `entries` (already sorted by key) as a B-tree index file.
pub(crate) fn write_index(
    path: &Path,
    key_size: usize,
    kind_byte: u8,
    entries: &[(Vec<u8>, u32)],
) -> Result<()> {
    let per_page = (PAGE - LEAF_HEADER) / (4 + key_size);
    let values_at = LEAF_HEADER + per_page * 4;

    // Level 0: leaves. Each level is a list of (pages' entries, max key of page).
    let leaves: Vec<&[(Vec<u8>, u32)]> = if entries.is_empty() {
        vec![&[]]
    } else {
        entries.chunks(per_page).collect()
    };
    let mut levels: Vec<Vec<Vec<u8>>> = Vec::new(); // max key per node, per level
    levels.push(
        leaves
            .iter()
            .map(|c| {
                c.last()
                    .map(|e| e.0.clone())
                    .unwrap_or_else(|| vec![0; key_size])
            })
            .collect(),
    );
    // Internal nodes hold up to `per_page` children.
    let mut fanouts: Vec<usize> = Vec::new();
    while levels.last().unwrap().len() > 1 {
        let below = levels.last().unwrap();
        let nodes: Vec<Vec<u8>> = below
            .chunks(per_page)
            .map(|c| c.last().unwrap().clone())
            .collect();
        fanouts.push(per_page);
        levels.push(nodes);
    }
    let depth = levels.len();

    // Page numbering: root = 1, then each level top-down, leaves last.
    let mut first_page = vec![0usize; depth];
    let mut next = 1;
    for lvl in (0..depth).rev() {
        first_page[lvl] = next;
        next += levels[lvl].len();
    }
    let mut pages = vec![vec![0u8; PAGE]; next - 1];

    for (i, chunk) in leaves.iter().enumerate() {
        let p = &mut pages[first_page[0] + i - 1];
        let next_leaf = if i + 1 < leaves.len() {
            (first_page[0] + i + 1) as u32
        } else {
            0
        };
        p[0..4].copy_from_slice(&next_leaf.to_le_bytes());
        p[4..8].copy_from_slice(&(chunk.len() as u32).to_le_bytes());
        for (j, (key, oid)) in chunk.iter().enumerate() {
            p[LEAF_HEADER + 4 * j..][..4].copy_from_slice(&oid.to_le_bytes());
            p[values_at + key_size * j..][..key_size].copy_from_slice(key);
        }
    }
    for lvl in 1..depth {
        let children = &levels[lvl - 1];
        for (i, group) in (0..children.len())
            .collect::<Vec<_>>()
            .chunks(per_page)
            .enumerate()
        {
            let p = &mut pages[first_page[lvl] + i - 1];
            let n = group.len() - 1; // values for all but the last child
            p[4..8].copy_from_slice(&(n as u32).to_le_bytes());
            for (j, &c) in group.iter().enumerate() {
                let page_no = (first_page[lvl - 1] + c) as u32;
                p[INTERNAL_HEADER + 4 * j..][..4].copy_from_slice(&page_no.to_le_bytes());
                if j < n {
                    p[values_at + key_size * j..][..key_size].copy_from_slice(&children[c]);
                }
            }
        }
    }

    let mut out = Vec::with_capacity(pages.len() * PAGE + 22);
    pages.iter().for_each(|p| out.extend_from_slice(p));
    out.push(key_size as u8);
    out.push(kind_byte);
    out.extend_from_slice(&1i32.to_le_bytes());
    out.extend_from_slice(&(depth as u32).to_le_bytes());
    out.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    out.extend_from_slice(&0i32.to_le_bytes());
    out.extend_from_slice(&1i32.to_le_bytes());
    let _ = fanouts;
    std::fs::write(path, out).map_err(|e| Error::io(path, e))
}

/// Picks a single spatial grid size: about one cell per typical feature, and at
/// most ~32 cells across the largest one.
pub(crate) fn grid_size(envelopes: &[(u64, Envelope)], extent: &Envelope, resolution: f64) -> f64 {
    let mut sizes: Vec<f64> = envelopes
        .iter()
        .map(|(_, e)| (e.xmax - e.xmin).max(e.ymax - e.ymin))
        .collect();
    sizes.sort_by(f64::total_cmp);
    let span = (extent.xmax - extent.xmin).max(extent.ymax - extent.ymin);
    let median = sizes.get(sizes.len() / 2).copied().unwrap_or(0.0);
    let largest = sizes.last().copied().unwrap_or(0.0);
    let mut g = if median > 0.0 {
        median
    } else {
        span / (envelopes.len() as f64).sqrt().max(1.0)
    };
    g = g.max(largest / 32.0).max(resolution * 1024.0);
    // Cell numbers must fit in 29 bits around the origin.
    let reach = [extent.xmin, extent.xmax, extent.ymin, extent.ymax]
        .iter()
        .fold(0.0f64, |a, v| a.max(v.abs()));
    g = g.max(reach / f64::from(1u32 << 28));
    if !(g.is_finite() && g > 0.0) { 1.0 } else { g }
}

/// Spatial index entries for one grid level, sorted as the format requires.
pub(crate) fn spatial_entries(envelopes: &[(u64, Envelope)], grid: f64) -> Vec<(Vec<u8>, u32)> {
    let off = 1i64 << 29;
    let cell = |v: f64| (v / grid).floor() as i64;
    let mut keys: Vec<(i64, u32)> = Vec::new();
    for (oid, e) in envelopes {
        for cx in cell(e.xmin)..=cell(e.xmax) {
            for cy in cell(e.ymin)..=cell(e.ymax) {
                let k = ((((cx + off) as u64) << 31) | ((cy + off) as u64)) as i64;
                keys.push((k, *oid as u32));
            }
        }
    }
    keys.sort_unstable();
    keys.into_iter()
        .map(|(k, oid)| (k.to_le_bytes().to_vec(), oid))
        .collect()
}
