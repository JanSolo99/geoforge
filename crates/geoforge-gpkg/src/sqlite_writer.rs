//! Writes new SQLite database files from the public file format description
//! (<https://www.sqlite.org/fileformat2.html>), as a bulk load: rows arrive in
//! rowid order and are packed into leaf pages that are written out as soon as
//! they fill, overflow pages are written once, and the interior levels of each
//! b-tree are built afterwards from the finished pages, so no page is ever split
//! or rewritten. Indexes are sorted once and built bottom-up the same way. The
//! schema table and the header go into page 1 last; the file is written to a
//! temporary name in one sequential pass and renamed into place.

use std::cmp::Ordering;
use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use geoforge_core::{Error, Result};

use crate::sqlite::SqlValue;

/// A value to store, borrowed from the caller.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ValueRef<'a> {
    Null,
    Integer(i64),
    Real(f64),
    Text(&'a str),
    Blob(&'a [u8]),
}

impl SqlValue {
    pub fn as_value_ref(&self) -> ValueRef<'_> {
        match self {
            SqlValue::Null => ValueRef::Null,
            SqlValue::Integer(v) => ValueRef::Integer(*v),
            SqlValue::Real(v) => ValueRef::Real(*v),
            SqlValue::Text(t) => ValueRef::Text(t),
            SqlValue::Blob(b) => ValueRef::Blob(b),
        }
    }
}

/// File-level settings.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// A power of two from 512 to 65536.
    pub page_size: usize,
    /// Header `application_id` (GeoPackage: 0x47504B47, "GPKG").
    pub application_id: u32,
    /// Header `user_version` (GeoPackage 1.4: 10400).
    pub user_version: u32,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            page_size: 4096,
            application_id: 0,
            user_version: 0,
        }
    }
}

/// The version of SQLite whose file format this follows, stored in the header.
const SQLITE_VERSION_NUMBER: u32 = 3_046_000;

/// The byte offset of the lock-byte page, which must never hold b-tree data.
const PENDING_BYTE: u64 = 0x4000_0000;

const TABLE_LEAF: u8 = 0x0d;
const TABLE_INTERIOR: u8 = 0x05;
const INDEX_LEAF: u8 = 0x0a;
const INDEX_INTERIOR: u8 = 0x02;

struct SchemaRow {
    kind: &'static str,
    name: String,
    table: String,
    root: u32,
    sql: Option<String>,
}

/// A database being written.
pub struct Writer {
    /// `None` once finished.
    out: Option<BufWriter<File>>,
    temp: PathBuf,
    path: PathBuf,
    options: Options,
    usable: usize,
    /// The number of pages written so far (page 1 is reserved up front).
    pages: u32,
    schema: Vec<SchemaRow>,
    /// Reusable buffers.
    record: Vec<u8>,
    cell: Vec<u8>,
}

impl Writer {
    /// Starts a new database at `path`, replacing any file there when finished.
    pub fn create(path: &Path, options: Options) -> Result<Writer> {
        let ps = options.page_size;
        if !(512..=65536).contains(&ps) || !ps.is_power_of_two() {
            return Err(Error::Unsupported(format!("SQLite page size {ps}")));
        }
        let temp = PathBuf::from(format!("{}.geoforge-tmp", path.display()));
        let file = File::create(&temp).map_err(|e| Error::io(&temp, e))?;
        let mut out = BufWriter::with_capacity(1 << 20, file);
        // Page 1 (header and schema) is filled in at the end.
        out.write_all(&vec![0u8; ps])
            .map_err(|e| Error::io(&temp, e))?;
        Ok(Writer {
            out: Some(out),
            temp,
            path: path.to_path_buf(),
            options,
            usable: ps,
            pages: 1,
            schema: Vec::new(),
            record: Vec::new(),
            cell: Vec::new(),
        })
    }

    fn write(&mut self, bytes: &[u8]) -> Result<()> {
        self.out
            .as_mut()
            .expect("writer is open")
            .write_all(bytes)
            .map_err(|e| Error::io(&self.temp, e))
    }

    /// Appends a page; returns its number.
    fn append(&mut self, page: &[u8]) -> Result<u32> {
        let ps = self.options.page_size as u64;
        // Skip the lock-byte page.
        if self.pages as u64 * ps == PENDING_BYTE {
            self.write(&vec![0u8; ps as usize])?;
            self.pages += 1;
        }
        self.write(page)?;
        self.pages += 1;
        Ok(self.pages)
    }

    /// Writes `payload` past its first `local` bytes into overflow pages and
    /// returns the first page number.
    fn overflow(&mut self, payload: &[u8], local: usize) -> Result<u32> {
        let chunk = self.usable - 4;
        let rest = &payload[local..];
        let count = rest.len().div_ceil(chunk);
        let ps = self.options.page_size;
        let mut page = vec![0u8; ps];
        let mut first = 0;
        for (k, part) in rest.chunks(chunk).enumerate() {
            // Pages are consecutive unless the lock-byte page intervenes, so
            // work out the next page's number from where it will land.
            let this = self.next_page_number();
            if k == 0 {
                first = this;
            }
            let next = if k + 1 < count {
                let after = this + 1;
                if (after as u64 - 1) * ps as u64 == PENDING_BYTE {
                    after + 1
                } else {
                    after
                }
            } else {
                0
            };
            page[..4].copy_from_slice(&next.to_be_bytes());
            page[4..4 + part.len()].copy_from_slice(part);
            page[4 + part.len()..].fill(0);
            let written = self.append(&page)?;
            debug_assert_eq!(written, this);
        }
        Ok(first)
    }

    fn next_page_number(&self) -> u32 {
        let ps = self.options.page_size as u64;
        let n = self.pages + 1;
        if (n as u64 - 1) * ps == PENDING_BYTE {
            n + 1
        } else {
            n
        }
    }

    /// The number of payload bytes kept on a b-tree page (the rest overflows).
    fn local_size(&self, payload: usize, index: bool) -> usize {
        let u = self.usable;
        let x = if index {
            (u - 12) * 64 / 255 - 23
        } else {
            u - 35
        };
        if payload <= x {
            return payload;
        }
        let m = (u - 12) * 32 / 255 - 23;
        let k = m + (payload - m) % (u - 4);
        if k <= x { k } else { m }
    }

    /// `varint(len) [varint(rowid)] local [overflow page]` of a leaf cell (or
    /// the part of an index interior cell after its child pointer).
    fn payload_cell(&mut self, rowid: Option<i64>, index: bool, out: &mut Vec<u8>) -> Result<()> {
        out.clear();
        let record = std::mem::take(&mut self.record);
        put_varint(out, record.len() as u64);
        if let Some(r) = rowid {
            put_varint(out, r as u64);
        }
        let local = self.local_size(record.len(), index);
        out.extend_from_slice(&record[..local]);
        if local < record.len() {
            let first = self.overflow(&record, local)?;
            out.extend_from_slice(&first.to_be_bytes());
        }
        self.record = record;
        Ok(())
    }

    /// Starts a table; rows go in with [`TableWriter::push`].
    pub fn table(&mut self, name: &str, sql: &str) -> TableWriter<'_> {
        let ps = self.options.page_size;
        TableWriter {
            w: self,
            name: name.to_string(),
            sql: sql.to_string(),
            leaf: PageBuf::new(ps, 0, TABLE_LEAF),
            children: Vec::new(),
            last: None,
        }
    }

    /// Adds an index of `table` (which must have been written): `entries` are
    /// each row's indexed values and rowid. `sql` is the CREATE INDEX statement,
    /// or `None` for an automatic index of a UNIQUE / PRIMARY KEY constraint
    /// (named `sqlite_autoindex_<table>_<n>`). Values compare as SQLite does
    /// with the BINARY collation.
    pub fn index(
        &mut self,
        name: &str,
        table: &str,
        sql: Option<&str>,
        mut entries: Vec<(Vec<SqlValue>, i64)>,
    ) -> Result<()> {
        // Rowids make every key unique, so an unstable sort is exact.
        entries.sort_unstable_by(|a, b| {
            for (x, y) in a.0.iter().zip(&b.0) {
                match compare(x, y) {
                    Ordering::Equal => {}
                    o => return o,
                }
            }
            a.1.cmp(&b.1)
        });
        // Each entry's cell body (shared by leaf and interior cells), packed
        // into one buffer.
        let mut arena = Vec::new();
        let mut spans = Vec::with_capacity(entries.len());
        let mut values: Vec<ValueRef> = Vec::new();
        let mut body = Vec::new();
        for (key, rowid) in &entries {
            values.clear();
            values.extend(key.iter().map(SqlValue::as_value_ref));
            values.push(ValueRef::Integer(*rowid));
            let mut record = std::mem::take(&mut self.record);
            encode_record(&values, &mut record);
            self.record = record;
            self.payload_cell(None, true, &mut body)?;
            spans.push((arena.len(), body.len()));
            arena.extend_from_slice(&body);
        }
        drop(entries);
        let bodies = Bodies { arena, spans };
        let root = self.build_index(bodies)?;
        self.schema.push(SchemaRow {
            kind: "index",
            name: name.to_string(),
            table: table.to_string(),
            root,
            sql: sql.map(str::to_string),
        });
        Ok(())
    }

    /// Adds a schema entry without data (a view, trigger or virtual table).
    pub fn schema_entry(&mut self, kind: &'static str, name: &str, table: &str, sql: &str) {
        self.schema.push(SchemaRow {
            kind,
            name: name.to_string(),
            table: table.to_string(),
            root: 0,
            sql: Some(sql.to_string()),
        });
    }

    fn build_index(&mut self, bodies: Bodies) -> Result<u32> {
        let ps = self.options.page_size;
        // Leaves separated by single divider entries.
        let groups = split_with_dividers(&bodies.spans, ps - 8, |s| 2 + s.1);
        let mut children = Vec::with_capacity(groups.len());
        let mut dividers = Vec::with_capacity(groups.len());
        for (range, divider) in groups {
            let mut page = PageBuf::new(ps, 0, INDEX_LEAF);
            for k in range {
                page.add(bodies.get(k));
            }
            children.push(self.append(page.finish(None))?);
            dividers.extend(divider);
        }
        // Interior levels: c0 d0 c1 d1 ... cn, until one page is left.
        while children.len() > 1 {
            let cells: Vec<usize> = (0..dividers.len()).collect();
            let groups =
                split_with_dividers(&cells, ps - 12, |&j| 2 + 4 + bodies.spans[dividers[j]].1);
            let mut next_children = Vec::new();
            let mut next_dividers = Vec::new();
            let mut start = 0;
            for (range, divider) in groups {
                // Cells for children start..start+len, the next child rightmost.
                let mut page = PageBuf::new(ps, 0, INDEX_INTERIOR);
                let mut cell = Vec::new();
                for j in range.clone() {
                    cell.clear();
                    cell.extend_from_slice(&children[j].to_be_bytes());
                    cell.extend_from_slice(bodies.get(dividers[j]));
                    page.add(&cell);
                }
                let right = children[range.end];
                next_children.push(self.append(page.finish(Some(right)))?);
                if let Some(d) = divider {
                    next_dividers.push(dividers[d]);
                }
                start = range.end + 1;
            }
            debug_assert_eq!(start, children.len());
            children = next_children;
            dividers = next_dividers;
        }
        Ok(children[0])
    }

    /// Interior levels of a table b-tree over `(page, largest rowid)` children;
    /// returns the root page.
    fn build_table_interior(&mut self, mut items: Vec<(u32, i64)>) -> Result<u32> {
        let ps = self.options.page_size;
        while items.len() > 1 {
            let groups = group_children(&items, ps - 12);
            let mut next = Vec::with_capacity(groups.len());
            for g in groups {
                let page = self.interior_table_page(&g, 0);
                next.push((self.append(&page)?, g.last().unwrap().1));
            }
            items = next;
        }
        Ok(items[0].0)
    }

    fn interior_table_page(&self, children: &[(u32, i64)], header_at: usize) -> Vec<u8> {
        let mut page = PageBuf::new(self.options.page_size, header_at, TABLE_INTERIOR);
        let mut cell = Vec::with_capacity(13);
        for &(child, key) in &children[..children.len() - 1] {
            cell.clear();
            cell.extend_from_slice(&child.to_be_bytes());
            put_varint(&mut cell, key as u64);
            page.add(&cell);
        }
        page.finish(Some(children.last().unwrap().0)).to_vec()
    }

    /// Writes the schema into page 1, the header, and moves the file into place.
    pub fn finish(mut self) -> Result<()> {
        let ps = self.options.page_size;
        // Schema rows as leaf cells.
        let rows = std::mem::take(&mut self.schema);
        let mut cells = Vec::with_capacity(rows.len());
        for (k, r) in rows.iter().enumerate() {
            let values = [
                ValueRef::Text(r.kind),
                ValueRef::Text(&r.name),
                ValueRef::Text(&r.table),
                ValueRef::Integer(r.root as i64),
                r.sql.as_deref().map_or(ValueRef::Null, ValueRef::Text),
            ];
            let mut record = std::mem::take(&mut self.record);
            encode_record(&values, &mut record);
            self.record = record;
            let mut cell = Vec::new();
            self.payload_cell(Some(k as i64 + 1), false, &mut cell)?;
            cells.push(cell);
        }
        let fits_page1 = cells.iter().map(|c| c.len() + 2).sum::<usize>() <= ps - 100 - 8;
        let page1 = if fits_page1 {
            let mut page = PageBuf::new(ps, 100, TABLE_LEAF);
            for c in &cells {
                page.add(c);
            }
            page.finish(None).to_vec()
        } else {
            // Leaves elsewhere (at least two), then interior levels until the
            // root fits on page 1.
            let mut items = Vec::new();
            let mut page = PageBuf::new(ps, 0, TABLE_LEAF);
            let half = cells.len().div_ceil(2);
            for (k, c) in cells.iter().enumerate() {
                if !page.fits(c.len()) || (k == half && items.is_empty() && page.ncells > 0) {
                    let p = self.append(page.finish(None))?;
                    items.push((p, k as i64));
                    page = PageBuf::new(ps, 0, TABLE_LEAF);
                }
                page.add(c);
            }
            let p = self.append(page.finish(None))?;
            items.push((p, cells.len() as i64));
            loop {
                let cost: usize = items[..items.len() - 1]
                    .iter()
                    .map(|&(_, key)| 2 + 4 + varint_len(key as u64))
                    .sum();
                if cost <= ps - 100 - 12 {
                    break self.interior_table_page(&items, 100);
                }
                let groups = group_children(&items, ps - 12);
                let mut next = Vec::new();
                for g in groups {
                    let page = self.interior_table_page(&g, 0);
                    next.push((self.append(&page)?, g.last().unwrap().1));
                }
                items = next;
            }
        };
        let mut page1 = page1;
        self.header(&mut page1[..100]);
        let mut out = self.out.take().expect("writer is open");
        let io = |e| Error::io(&self.temp, e);
        out.seek(SeekFrom::Start(0)).map_err(io)?;
        out.write_all(&page1).map_err(io)?;
        let file = out.into_inner().map_err(|e| io(e.into_error()))?;
        file.sync_all().map_err(io)?;
        drop(file);
        std::fs::rename(&self.temp, &self.path).map_err(|e| Error::io(&self.path, e))
    }

    fn header(&self, h: &mut [u8]) {
        let o = &self.options;
        h[..16].copy_from_slice(b"SQLite format 3\0");
        let ps = if o.page_size == 65536 {
            1
        } else {
            o.page_size as u16
        };
        h[16..18].copy_from_slice(&ps.to_be_bytes());
        h[18] = 1; // legacy (rollback journal) write and read versions
        h[19] = 1;
        h[20] = (o.page_size - self.usable) as u8;
        h[21] = 64;
        h[22] = 32;
        h[23] = 32;
        let be = |h: &mut [u8], at: usize, v: u32| h[at..at + 4].copy_from_slice(&v.to_be_bytes());
        be(h, 24, 1); // file change counter
        be(h, 28, self.pages); // database size in pages
        be(h, 32, 0); // freelist trunk
        be(h, 36, 0); // freelist pages
        be(h, 40, 1); // schema cookie
        be(h, 44, 4); // schema format
        be(h, 48, 0); // default cache size
        be(h, 52, 0); // no auto-vacuum
        be(h, 56, 1); // UTF-8
        be(h, 60, o.user_version);
        be(h, 64, 0);
        be(h, 68, o.application_id);
        h[72..92].fill(0);
        be(h, 92, 1); // version-valid-for = change counter
        be(h, 96, SQLITE_VERSION_NUMBER);
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        // An unfinished database leaves no file behind.
        if self.out.take().is_some() {
            let _ = std::fs::remove_file(&self.temp);
        }
    }
}

/// Index cell bodies packed into one buffer.
struct Bodies {
    arena: Vec<u8>,
    spans: Vec<(usize, usize)>,
}

impl Bodies {
    fn get(&self, k: usize) -> &[u8] {
        let (at, len) = self.spans[k];
        &self.arena[at..at + len]
    }
}

/// A table being written.
pub struct TableWriter<'w> {
    w: &'w mut Writer,
    name: String,
    sql: String,
    leaf: PageBuf,
    children: Vec<(u32, i64)>,
    last: Option<i64>,
}

impl TableWriter<'_> {
    /// Adds a row; rowids must increase. A rowid-alias (INTEGER PRIMARY KEY)
    /// column is stored as NULL, as SQLite does.
    pub fn push(&mut self, rowid: i64, values: &[ValueRef]) -> Result<()> {
        if self.last.is_some_and(|l| rowid <= l) {
            return Err(Error::Unsupported(format!(
                "{}: rowid {rowid} after {}",
                self.name,
                self.last.unwrap()
            )));
        }
        let mut record = std::mem::take(&mut self.w.record);
        encode_record(values, &mut record);
        self.w.record = record;
        let mut cell = std::mem::take(&mut self.w.cell);
        self.w.payload_cell(Some(rowid), false, &mut cell)?;
        if !self.leaf.fits(cell.len()) {
            let page = self.w.append(self.leaf.finish(None))?;
            self.children.push((page, self.last.unwrap()));
            self.leaf.reset();
        }
        self.leaf.add(&cell);
        self.w.cell = cell;
        self.last = Some(rowid);
        Ok(())
    }

    /// Finishes the table and records it in the schema.
    pub fn finish(mut self) -> Result<()> {
        if self.leaf.ncells > 0 || self.children.is_empty() {
            let page = self.w.append(self.leaf.finish(None))?;
            self.children.push((page, self.last.unwrap_or(0)));
        }
        let children = std::mem::take(&mut self.children);
        let root = self.w.build_table_interior(children)?;
        self.w.schema.push(SchemaRow {
            kind: "table",
            name: self.name.clone(),
            table: self.name.clone(),
            root,
            sql: Some(self.sql.clone()),
        });
        Ok(())
    }
}

/// A b-tree page under construction: cells grow down from the end, their
/// pointers up after the header.
struct PageBuf {
    data: Vec<u8>,
    header_at: usize,
    kind: u8,
    ncells: usize,
    content_start: usize,
}

impl PageBuf {
    fn new(page_size: usize, header_at: usize, kind: u8) -> PageBuf {
        PageBuf {
            data: vec![0u8; page_size],
            header_at,
            kind,
            ncells: 0,
            content_start: page_size,
        }
    }

    fn header_len(&self) -> usize {
        if self.kind == TABLE_INTERIOR || self.kind == INDEX_INTERIOR {
            12
        } else {
            8
        }
    }

    fn fits(&self, cell_len: usize) -> bool {
        let pointers_end = self.header_at + self.header_len() + 2 * (self.ncells + 1);
        self.content_start >= pointers_end + cell_len
    }

    fn add(&mut self, cell: &[u8]) {
        debug_assert!(self.fits(cell.len()));
        self.content_start -= cell.len();
        self.data[self.content_start..self.content_start + cell.len()].copy_from_slice(cell);
        let at = self.header_at + self.header_len() + 2 * self.ncells;
        self.data[at..at + 2].copy_from_slice(&(self.content_start as u16).to_be_bytes());
        self.ncells += 1;
    }

    fn finish(&mut self, right: Option<u32>) -> &[u8] {
        let h = self.header_at;
        self.data[h] = self.kind;
        self.data[h + 1..h + 3].fill(0);
        self.data[h + 3..h + 5].copy_from_slice(&(self.ncells as u16).to_be_bytes());
        // 65536 is stored as 0.
        self.data[h + 5..h + 7].copy_from_slice(&(self.content_start as u16).to_be_bytes());
        self.data[h + 7] = 0;
        if let Some(r) = right {
            self.data[h + 8..h + 12].copy_from_slice(&r.to_be_bytes());
        }
        // Unused space between the pointers and the content stays zero.
        let free_from = h + self.header_len() + 2 * self.ncells;
        self.data[free_from..self.content_start].fill(0);
        &self.data
    }

    fn reset(&mut self) {
        self.ncells = 0;
        self.content_start = self.data.len();
    }
}

/// Splits `items` into pages of at most `capacity` bytes (by `cost`) with one
/// item between consecutive pages taken out as a divider; returns each page's
/// range and the divider after it. Every page gets at least one item.
fn split_with_dividers<T>(
    items: &[T],
    capacity: usize,
    cost: impl Fn(&T) -> usize,
) -> Vec<(std::ops::Range<usize>, Option<usize>)> {
    let n = items.len();
    let mut out: Vec<(std::ops::Range<usize>, Option<usize>)> = Vec::new();
    let mut i = 0;
    loop {
        let start = i;
        let mut used = 0;
        while i < n && (i == start || used + cost(&items[i]) <= capacity) {
            used += cost(&items[i]);
            i += 1;
        }
        if i >= n {
            out.push((start..i, None));
            return out;
        }
        if i == n - 1 {
            // A lone last item would leave an empty page: make this page's
            // last item the divider instead.
            assert!(i - start >= 2, "pages hold several entries");
            out.push((start..i - 1, Some(i - 1)));
            out.push((i..n, None));
            return out;
        }
        out.push((start..i, Some(i)));
        i += 1;
    }
}

/// Groups `(page, key)` children into interior pages of at most `capacity`
/// bytes: every child but a page's last becomes a cell, the last its right
/// pointer; every page gets at least two children.
fn group_children(items: &[(u32, i64)], capacity: usize) -> Vec<Vec<(u32, i64)>> {
    let mut groups: Vec<Vec<(u32, i64)>> = Vec::new();
    let mut cur: Vec<(u32, i64)> = Vec::new();
    let mut used = 0;
    for &item in items {
        if let Some(&(_, key)) = cur.last() {
            let cost = 2 + 4 + varint_len(key as u64);
            if used + cost > capacity && cur.len() >= 2 {
                groups.push(std::mem::take(&mut cur));
                used = 0;
            } else {
                used += cost;
            }
        }
        cur.push(item);
    }
    if cur.len() == 1
        && let Some(prev) = groups.last_mut()
    {
        cur.insert(0, prev.pop().unwrap());
    }
    groups.push(cur);
    groups
}

/// SQLite's order of values (BINARY collation): NULL, numbers, text, blobs.
fn compare(a: &SqlValue, b: &SqlValue) -> Ordering {
    use SqlValue::*;
    let class = |v: &SqlValue| match v {
        Null => 0,
        Integer(_) | Real(_) => 1,
        Text(_) => 2,
        Blob(_) => 3,
    };
    match (a, b) {
        (Integer(x), Integer(y)) => x.cmp(y),
        (Integer(_) | Real(_), Integer(_) | Real(_)) => {
            let (x, y) = (a.as_f64().unwrap(), b.as_f64().unwrap());
            x.partial_cmp(&y).unwrap_or(Ordering::Equal)
        }
        (Text(x), Text(y)) => x.as_bytes().cmp(y.as_bytes()),
        (Blob(x), Blob(y)) => x.cmp(y),
        _ => class(a).cmp(&class(b)),
    }
}

fn varint_len(v: u64) -> usize {
    if v >> 56 != 0 {
        return 9;
    }
    let mut n = 1;
    let mut x = v >> 7;
    while x != 0 {
        n += 1;
        x >>= 7;
    }
    n
}

fn put_varint(out: &mut Vec<u8>, v: u64) {
    if v >> 56 != 0 {
        // Nine bytes: eight groups of seven bits, then a whole byte.
        let mut bytes = [0u8; 9];
        bytes[8] = v as u8;
        let mut x = v >> 8;
        for b in bytes[..8].iter_mut().rev() {
            *b = (x as u8 & 0x7f) | 0x80;
            x >>= 7;
        }
        out.extend_from_slice(&bytes);
        return;
    }
    let n = varint_len(v);
    for k in (0..n).rev() {
        let group = ((v >> (7 * k)) & 0x7f) as u8;
        out.push(if k == 0 { group } else { group | 0x80 });
    }
}

/// The serial type of a value and its body length.
fn serial_type(v: &ValueRef) -> (u64, usize) {
    match *v {
        ValueRef::Null => (0, 0),
        ValueRef::Integer(0) => (8, 0),
        ValueRef::Integer(1) => (9, 0),
        ValueRef::Integer(i) => {
            if (-128..=127).contains(&i) {
                (1, 1)
            } else if (-32768..=32767).contains(&i) {
                (2, 2)
            } else if (-8_388_608..=8_388_607).contains(&i) {
                (3, 3)
            } else if (i32::MIN as i64..=i32::MAX as i64).contains(&i) {
                (4, 4)
            } else if (-(1i64 << 47)..(1i64 << 47)).contains(&i) {
                (5, 6)
            } else {
                (6, 8)
            }
        }
        ValueRef::Real(_) => (7, 8),
        ValueRef::Text(t) => (13 + 2 * t.len() as u64, t.len()),
        ValueRef::Blob(b) => (12 + 2 * b.len() as u64, b.len()),
    }
}

/// Encodes a record: header (its length, then serial types), then the values.
fn encode_record(values: &[ValueRef], out: &mut Vec<u8>) {
    out.clear();
    let types_len: usize = values.iter().map(|v| varint_len(serial_type(v).0)).sum();
    let mut header_len = types_len + 1;
    if varint_len(header_len as u64) > 1 {
        header_len = types_len + varint_len((types_len + 2) as u64);
    }
    put_varint(out, header_len as u64);
    for v in values {
        put_varint(out, serial_type(v).0);
    }
    for v in values {
        match *v {
            ValueRef::Integer(i) => {
                let (_, len) = serial_type(v);
                out.extend_from_slice(&i.to_be_bytes()[8 - len..]);
            }
            ValueRef::Real(r) => out.extend_from_slice(&r.to_bits().to_be_bytes()),
            ValueRef::Text(t) => out.extend_from_slice(t.as_bytes()),
            ValueRef::Blob(b) => out.extend_from_slice(b),
            ValueRef::Null => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varints_round_trip() {
        for v in [
            0u64,
            1,
            127,
            128,
            16383,
            16384,
            1 << 35,
            (1 << 56) - 1,
            1 << 56,
            u64::MAX,
        ] {
            let mut b = Vec::new();
            put_varint(&mut b, v);
            assert_eq!(b.len(), varint_len(v), "{v}");
            assert_eq!(crate::sqlite::varint(&b, 0).unwrap(), (v, b.len()), "{v}");
        }
    }

    #[test]
    fn dividers_leave_no_empty_page() {
        let items = vec![10usize; 9];
        // Three per page: 0..3 | 3 | 4..7 | 7 | 8 would leave one; ends fine.
        let groups = split_with_dividers(&items, 30, |&c| c);
        assert!(groups.iter().all(|(r, _)| !r.is_empty()));
        let total: usize = groups
            .iter()
            .map(|(r, d)| r.len() + d.is_some() as usize)
            .sum();
        assert_eq!(total, 9);
    }
}
