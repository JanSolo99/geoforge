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
    // Entries per leaf. Keys start after the id array of a full leaf, in leaf and
    // internal pages alike; an internal page therefore holds `per_leaf + 1` child ids
    // and `per_leaf` keys.
    let per_leaf = (PAGE - LEAF_HEADER) / (4 + key_size);
    let values_at = LEAF_HEADER + per_leaf * 4;
    let per_node = per_leaf + 1;

    // Level 0: leaves. `levels[l]` holds, per page of level l, the key it has in its
    // parent: a leaf's last key; an internal page's last stored key.
    let leaves: Vec<&[(Vec<u8>, u32)]> = if entries.is_empty() {
        vec![&[]]
    } else {
        entries.chunks(per_leaf).collect()
    };
    let mut levels: Vec<Vec<Vec<u8>>> = Vec::new();
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
    // Internal pages of a level take `per_node` children each, and the last child of
    // one page is also the first child of the next.
    let groups_of = |n: usize| -> Vec<std::ops::Range<usize>> {
        let mut out = Vec::new();
        let mut start = 0;
        loop {
            let end = (start + per_node).min(n);
            out.push(start..end);
            if end == n {
                return out;
            }
            start = end - 1;
        }
    };
    let mut level_groups: Vec<Vec<std::ops::Range<usize>>> = Vec::new();
    while levels.last().unwrap().len() > 1 {
        let below = levels.last().unwrap();
        let groups = groups_of(below.len());
        let keys: Vec<Vec<u8>> = groups
            .iter()
            .map(|g| below[g.end - 1 - usize::from(g.len() >= 2)].clone())
            .collect();
        level_groups.push(groups);
        levels.push(keys);
    }
    let depth = levels.len();

    // Page numbering as ArcGIS writes it: the root is page 1, then the leaves in
    // order, then each internal level bottom-up.
    let mut first_page = vec![0usize; depth];
    first_page[depth - 1] = 1;
    let mut next = 2;
    for lvl in 0..depth - 1 {
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
        let groups = &level_groups[lvl - 1];
        for (i, group) in groups.iter().enumerate() {
            let p = &mut pages[first_page[lvl] + i - 1];
            // Pages of one level are chained (the root is alone).
            let next_node = if i + 1 < groups.len() {
                (first_page[lvl] + i + 1) as u32
            } else {
                0
            };
            p[0..4].copy_from_slice(&next_node.to_le_bytes());
            let n = group.len() - 1; // a key for every child but the last
            p[4..8].copy_from_slice(&(n as u32).to_le_bytes());
            for (j, c) in group.clone().enumerate() {
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
