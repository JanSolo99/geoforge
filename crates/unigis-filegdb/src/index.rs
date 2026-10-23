//! Attribute (`.atx`) and spatial (`.spx`) indexes: paged B-trees of
//! (key, OBJECTID) pairs. Page 1 is the root; leaf pages are chained.

use std::cmp::Ordering;
use std::fs::File;
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};

use unigis_core::{Envelope, Error, Result};

use crate::bytes::{Reader, err};

pub(crate) struct BTree {
    file: File,
    path: PathBuf,
    page_size: usize,
    pub key_size: usize,
    oid_size: usize,
    depth: u32,
    /// Where keys start (the same in leaf and internal pages).
    values_offset: usize,
    leaf_header: usize,
    internal_header: usize,
}

impl BTree {
    pub fn open(path: &Path) -> Result<Self> {
        let file = File::open(path).map_err(|e| Error::io(path, e))?;
        let len = file.metadata().map_err(|e| Error::io(path, e))?.len() as usize;
        let mut ver = [0u8; 4];
        file.read_exact_at(
            &mut ver,
            len.checked_sub(4).ok_or_else(|| err("index too short"))? as u64,
        )
        .map_err(|e| Error::io(path, e))?;
        let version = i32::from_le_bytes(ver);
        let (trailer_len, page_size, oid_size, leaf_header, internal_header) = match version {
            1 => (22, 4096, 4, 12, 8),
            2 => (30, 65536, 8, 20, 12),
            v => return Err(err(format!("unsupported index version {v}"))),
        };
        let mut t = vec![0u8; trailer_len];
        file.read_exact_at(&mut t, (len - trailer_len) as u64)
            .map_err(|e| Error::io(path, e))?;
        let mut r = Reader::new(&t);
        let key_size = usize::from(r.u8()?);
        r.u8()?;
        r.i32()?;
        let depth = r.u32()?;
        if key_size == 0 || depth == 0 || depth > 32 {
            return Err(err("implausible index header"));
        }
        let max_per_page = (page_size - leaf_header) / (oid_size + key_size);
        Ok(Self {
            file,
            path: path.to_owned(),
            page_size,
            key_size,
            oid_size,
            depth,
            values_offset: leaf_header + max_per_page * oid_size,
            leaf_header,
            internal_header,
        })
    }

    fn page(&self, n: u64) -> Result<Vec<u8>> {
        if n == 0 {
            return Err(err("index page 0"));
        }
        let mut p = vec![0u8; self.page_size];
        self.file
            .read_exact_at(&mut p, (n - 1) * self.page_size as u64)
            .map_err(|e| Error::io(&self.path, e))?;
        Ok(p)
    }

    fn id(&self, p: &[u8], at: usize) -> u64 {
        let mut b = [0u8; 8];
        b[..self.oid_size].copy_from_slice(&p[at..at + self.oid_size]);
        u64::from_le_bytes(b)
    }

    /// Visits `(oid, key)` for every key with `lower(key) != Less` and stops at the
    /// first key with `upper(key) == Greater`. Keys are in index order.
    pub fn scan(
        &self,
        lower: &dyn Fn(&[u8]) -> Ordering,
        upper: &dyn Fn(&[u8]) -> Ordering,
        visit: &mut dyn FnMut(u64, &[u8]),
    ) -> Result<()> {
        // Descend: internal values hold the maximum key under each child.
        let mut page_no = 1u64;
        for _ in 1..self.depth {
            let p = self.page(page_no)?;
            let n = u32::from_le_bytes(p[self.oid_size..self.oid_size + 4].try_into().unwrap())
                as usize;
            let child = (0..n)
                .find(|&i| {
                    let k = &p[self.values_offset + i * self.key_size..][..self.key_size];
                    lower(k) != Ordering::Less
                })
                .unwrap_or(n);
            page_no = self.id(&p, self.internal_header + child * self.oid_size);
        }
        // Walk the leaf chain.
        let mut guard = 0u64;
        while page_no != 0 {
            let p = self.page(page_no)?;
            let next = self.id(&p, 0);
            let n = u32::from_le_bytes(p[self.oid_size..self.oid_size + 4].try_into().unwrap())
                as usize;
            if self.values_offset + n * self.key_size > self.page_size {
                return Err(err("index page entry count too large"));
            }
            for i in 0..n {
                let k = &p[self.values_offset + i * self.key_size..][..self.key_size];
                if lower(k) == Ordering::Less {
                    continue;
                }
                if upper(k) == Ordering::Greater {
                    return Ok(());
                }
                visit(self.id(&p, self.leaf_header + i * self.oid_size), k);
            }
            page_no = next;
            guard += 1;
            if guard > 1 << 32 {
                return Err(err("index leaf chain loops"));
            }
        }
        Ok(())
    }
}

/// Spatial index key: `grid << 62 | (floor(x / size) + 2^29) << 31 | (floor(y / size) + 2^29)`,
/// sorted as signed 64-bit integers.
fn spatial_key(grid: u64, cx: i64, cy: i64) -> i64 {
    let off = 1i64 << 29;
    ((grid << 62) | (((cx + off) as u64) << 31) | ((cy + off) as u64)) as i64
}

/// OBJECTIDs whose index cells touch `env`, across all grid levels (may include
/// features whose geometry does not actually intersect; never misses one).
pub(crate) fn spatial_candidates(
    tree: &BTree,
    grid_sizes: &[f64],
    env: &Envelope,
) -> Result<Vec<u64>> {
    if tree.key_size != 8 {
        return Err(err("spatial index key size is not 8"));
    }
    let mut oids = Vec::new();
    for (g, &size) in grid_sizes.iter().enumerate() {
        if size.is_nan() || size <= 0.0 {
            continue;
        }
        let cell = |v: f64| (v / size).floor() as i64;
        let (x0, x1, y0, y1) = (
            cell(env.xmin),
            cell(env.xmax),
            cell(env.ymin),
            cell(env.ymax),
        );
        let key = |k: &[u8]| i64::from_le_bytes(k.try_into().unwrap());
        let off = 1i64 << 29;
        // One key range per x column keeps narrow windows cheap; very wide windows
        // use a single range and filter on y.
        if x1 - x0 <= 4096 {
            for cx in x0..=x1 {
                let (lo, hi) = (spatial_key(g as u64, cx, y0), spatial_key(g as u64, cx, y1));
                tree.scan(&|k| key(k).cmp(&lo), &|k| key(k).cmp(&hi), &mut |oid, _| {
                    oids.push(oid)
                })?;
            }
        } else {
            let (lo, hi) = (spatial_key(g as u64, x0, y0), spatial_key(g as u64, x1, y1));
            tree.scan(&|k| key(k).cmp(&lo), &|k| key(k).cmp(&hi), &mut |oid, k| {
                let cy = (key(k) as u64 & 0x7FFF_FFFF) as i64 - off;
                if (y0..=y1).contains(&cy) {
                    oids.push(oid);
                }
            })?;
        }
    }
    oids.sort_unstable();
    oids.dedup();
    Ok(oids)
}

/// One entry of `.gdbindexes`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexDef {
    pub name: String,
    /// Field name, or an expression such as `LOWER(Name)`.
    pub expression: String,
}

pub(crate) fn read_gdbindexes(path: &Path) -> Result<Vec<IndexDef>> {
    let bytes = std::fs::read(path).map_err(|e| Error::io(path, e))?;
    let mut r = Reader::new(&bytes);
    let n = r.u32()?;
    let mut out = Vec::new();
    for _ in 0..n.min(1024) {
        let len = r.u32()? as usize;
        let name = r.utf16(len)?;
        r.bytes(2 + 4 + 2 + 4)?;
        let len = r.u32()? as usize;
        let expression = r.utf16(len)?;
        r.bytes(2)?;
        out.push(IndexDef { name, expression });
    }
    Ok(out)
}
