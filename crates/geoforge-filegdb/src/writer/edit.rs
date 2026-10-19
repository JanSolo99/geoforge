//! Editing rows of an existing table in place (insert, update, delete), keeping the
//! `.gdbtable`, `.gdbtablx`, spatial and attribute indexes consistent, so ArcGIS reads
//! the result as one of its own.
//!
//! New and changed rows are appended to the `.gdbtable`, as ArcGIS does; the space of a
//! replaced or deleted row is simply left behind (a `.freelist` that would let ArcGIS
//! reuse it is removed so it can never point at space we have filled; `Compact`
//! reclaims it).
//!
//! The edit is one transaction (see [`super::txn`]): until [`TableEditor::finish`]
//! returns, a crash or a dropped editor leaves the table as it was.

use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use geoforge_core::{Envelope, Error, Feature, GeometryKind, Result};

use super::btree::{grid_size, spatial_entries, write_index_for};
use super::rebuild::rebuild_attribute_indexes;
use super::table::{
    RowLayout, encode_row, offset_size_for, write_dense_tablx, write_header_counts,
};
use super::txn;
use crate::bytes::err;
use crate::table::{GdbField, Table};

/// Opens an existing table for editing. Call [`TableEditor::finish`] to write the
/// changes; dropping the editor without it rolls them back.
pub struct TableEditor {
    base: PathBuf,
    /// The table as opened (reads rows by offset, including rows written since).
    table: Table,

    fields: Vec<GdbField>,
    geometry_kind: Option<GeometryKind>,
    has_z: bool,
    has_m: bool,
    nullable_count: usize,
    file: File,
    /// End of the `.gdbtable` (where the next row goes).
    pos: u64,
    /// Row offset per OBJECTID (index = OBJECTID - 1); 0 = deleted.
    offsets: Vec<u64>,
    offset_size: usize,
    max_row: u32,
    field_section_len: u32,
    /// Version 4 table (64-bit OBJECTIDs).
    v4: bool,
    subsecond: Vec<String>,
    row: Vec<u8>,
    changed: bool,
    /// Last, so the files are closed before a rollback on drop.
    txn: Option<txn::Guard>,
}

impl TableEditor {
    /// `base` is the table's path with or without `.gdbtable`.
    pub fn open(base: impl AsRef<Path>) -> Result<Self> {
        let base = base.as_ref().with_extension("");
        let dir = base
            .parent()
            .filter(|d| !d.as_os_str().is_empty())
            .unwrap_or(Path::new("."))
            .to_path_buf();
        let guard = txn::begin(&dir)?;
        let table = Table::open(&base)?;
        let fields = table.all_fields().to_vec();
        let mut offsets = Vec::with_capacity(table.max_oid() as usize);
        for oid in 1..=table.max_oid() {
            offsets.push(table.raw_offset(oid)?.unwrap_or(0));
        }
        let path = base.with_extension("gdbtable");
        let mut file = txn::open_for_append(&path)?;
        let pos = file
            .seek(SeekFrom::End(0))
            .map_err(|e| Error::io(&path, e))?;
        let tablx = std::fs::read(base.with_extension("gdbtablx"))
            .map_err(|e| Error::io(base.with_extension("gdbtablx"), e))?;
        let offset_size = tablx
            .get(12..16)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()) as usize)
            .filter(|s| (4..=6).contains(s))
            .ok_or_else(|| err("unreadable .gdbtablx header"))?;
        let mut header = [0u8; 12];
        std::io::Read::read_exact(
            &mut File::open(&path).map_err(|e| Error::io(&path, e))?,
            &mut header,
        )
        .map_err(|e| Error::io(&path, e))?;
        let max_row = u32::from_le_bytes(header[8..12].try_into().unwrap());
        Ok(Self {
            nullable_count: fields.iter().filter(|f| f.nullable).count(),
            geometry_kind: table.geometry_kind(),
            has_z: table.has_z(),
            has_m: table.has_m(),
            field_section_len: table.field_section_size(),
            v4: table.is_version_4(),
            fields,
            base,
            table,
            file,
            pos,
            offsets,
            offset_size,
            max_row,
            subsecond: Vec::new(),
            row: Vec::new(),
            changed: false,
            txn: Some(guard),
        })
    }

    /// The attribute fields, in the order `Feature::attributes` follows.
    pub fn fields(&self) -> &[geoforge_core::Field] {
        self.table.fields()
    }

    /// The row with OBJECTID `oid` as it stands in this edit, or `None` if there is none.
    pub fn get(&self, oid: u64) -> Result<Option<Feature>> {
        match self.offsets.get((oid as usize).wrapping_sub(1)) {
            Some(&at) if at != 0 => self.table.read_at(oid, at).map(Some),
            _ => Ok(None),
        }
    }

    /// Sets attribute `field` (case-insensitive name) of row `oid` to `value`.
    pub fn set_value(&mut self, oid: u64, field: &str, value: geoforge_core::Value) -> Result<()> {
        let i = self
            .fields()
            .iter()
            .position(|f| f.name.eq_ignore_ascii_case(field))
            .ok_or_else(|| err(format!("no field {field}")))?;
        let mut f = self
            .get(oid)?
            .ok_or_else(|| err(format!("OBJECTID {oid} does not exist")))?;
        f.attributes[i] = value;
        self.update(oid, &f)
    }

    /// Replaces the geometry of row `oid` (its attributes stay).
    pub fn set_geometry(
        &mut self,
        oid: u64,
        geometry: Option<geoforge_core::Geometry>,
    ) -> Result<()> {
        if geometry.is_some() && self.geometry_kind.is_none() {
            return Err(Error::Unsupported(
                "geometry in a table without a geometry field".into(),
            ));
        }
        let mut f = self
            .get(oid)?
            .ok_or_else(|| err(format!("OBJECTID {oid} does not exist")))?;
        f.geometry = geometry;
        self.update(oid, &f)
    }

    /// OBJECTIDs of the rows currently in the table, ascending.
    pub fn oids(&self) -> impl Iterator<Item = u64> + '_ {
        self.offsets
            .iter()
            .enumerate()
            .filter(|(_, o)| **o != 0)
            .map(|(i, _)| i as u64 + 1)
    }

    /// Rows currently in the table.
    pub fn row_count(&self) -> u64 {
        self.offsets.iter().filter(|o| **o != 0).count() as u64
    }

    /// The largest OBJECTID ever used (deleted rows included).
    pub fn max_oid(&self) -> u64 {
        self.offsets.len() as u64
    }

    fn append(&mut self, oid: u64, f: &Feature, measures: bool) -> Result<()> {
        encode_row(
            &RowLayout {
                fields: &self.fields,
                geometry_kind: self.geometry_kind,
                has_z: self.has_z,
                has_m: self.has_m,
                nullable_count: self.nullable_count,
                measures,
            },
            &mut self.subsecond,
            oid,
            f.geometry.as_ref(),
            &f.attributes,
            &mut self.row,
        )?;
        let path = self.base.with_extension("gdbtable");
        let io = |e| Error::io(&path, e);
        self.file.seek(SeekFrom::Start(self.pos)).map_err(io)?;
        self.file
            .write_all(&(self.row.len() as u32).to_le_bytes())
            .map_err(io)?;
        self.file.write_all(&self.row).map_err(io)?;
        let at = self.pos;
        self.pos += 4 + self.row.len() as u64;
        self.max_row = self.max_row.max(self.row.len() as u32);
        self.offsets[oid as usize - 1] = at;
        self.changed = true;
        Ok(())
    }

    /// Adds a row with the next OBJECTID (`feature.oid` is ignored); returns it.
    /// `feature.attributes` follow the non-OBJECTID, non-geometry fields.
    pub fn insert(&mut self, feature: &Feature) -> Result<u64> {
        let oid = self.offsets.len() as u64 + 1;
        self.offsets.push(0);
        if let Err(e) = self.append(oid, feature, true) {
            self.offsets.pop();
            return Err(e);
        }
        Ok(oid)
    }

    /// Replaces the row with OBJECTID `oid`.
    pub fn update(&mut self, oid: u64, feature: &Feature) -> Result<()> {
        match self.offsets.get(oid as usize - 1) {
            Some(&o) if o != 0 => {
                // The measures follow a changed geometry; otherwise they stay as given.
                let changed = self.table.read_at(oid, o)?.geometry != feature.geometry;
                self.append(oid, feature, changed)
            }
            _ => Err(err(format!("OBJECTID {oid} does not exist"))),
        }
    }

    /// Deletes the row with OBJECTID `oid`.
    pub fn delete(&mut self, oid: u64) -> Result<()> {
        match self.offsets.get_mut(oid as usize - 1) {
            Some(o) if *o != 0 => {
                *o = 0;
                self.changed = true;
                Ok(())
            }
            _ => Err(err(format!("OBJECTID {oid} does not exist"))),
        }
    }

    /// Writes the header, `.gdbtablx` and indexes. Returns the number of rows.
    pub fn finish(mut self) -> Result<u64> {
        let guard = self.txn.take().expect("transaction");
        if !self.changed {
            let n = self.row_count();
            drop(self);
            guard.commit()?;
            return Ok(n);
        }
        let path = self.base.with_extension("gdbtable");
        let io = |e| Error::io(&path, e);
        let rows = self.row_count();
        write_header_counts(
            &mut self.file,
            self.v4,
            rows,
            rows < self.offsets.len() as u64,
            self.max_row.max(self.field_section_len),
            self.pos,
        )
        .map_err(io)?;
        self.file.flush().map_err(io)?;
        self.file.sync_all().map_err(io)?;

        write_dense_tablx(
            &self.base.with_extension("gdbtablx"),
            &self.offsets,
            self.offset_size.max(offset_size_for(self.pos)),
            self.v4,
        )?;
        // A free list would let ArcGIS write into space that now holds rows.
        txn::remove_file(&self.base.with_extension("freelist"))?;

        if let Some(def) = self.fields.iter().find_map(|f| f.geometry.clone()) {
            self.update_spatial_index(&def)?;
        }
        rebuild_attribute_indexes(&self.base)?;
        drop(self);
        guard.commit()?;
        Ok(rows)
    }

    /// Rebuilds the `.spx` from the stored geometries and widens the stored extent.
    fn update_spatial_index(&self, def: &crate::table::GeometryDef) -> Result<()> {
        let table = Table::open(&self.base)?;
        let mut envelopes: Vec<(u64, Envelope)> = Vec::new();
        let mut extent = Envelope::EMPTY;
        for f in table.features() {
            let f = f?;
            if let Some(g) = &f.geometry {
                let e = g.curve_envelope();
                if !e.is_empty() {
                    extent.union(&e);
                    envelopes.push((f.oid as u64, e));
                }
            }
        }
        let resolution = 1.0 / def.xy_scale;
        let grid = def
            .grid_sizes
            .first()
            .copied()
            .filter(|g| g.is_finite() && *g > 0.0)
            .unwrap_or_else(|| grid_size(&envelopes, &extent, resolution));
        let path = self.base.with_extension("gdbtable");
        let io = |e| Error::io(&path, e);
        let mut file = OpenOptions::new().write(true).open(&path).map_err(io)?;
        if let Some(at) = def.grid_at
            && def
                .grid_sizes
                .first()
                .is_none_or(|g| !(g.is_finite() && *g > 0.0))
        {
            file.seek(SeekFrom::Start(at)).map_err(io)?;
            file.write_all(&grid.to_le_bytes()).map_err(io)?;
        }
        // The extent only grows (as ArcGIS leaves it until a compact).
        if let (Some(at), false) = (def.extent_at, extent.is_empty()) {
            let old = def.extent;
            let mut e = extent;
            if !old.is_empty() {
                e.union(&old);
            }
            file.seek(SeekFrom::Start(at)).map_err(io)?;
            for v in [e.xmin, e.ymin, e.xmax, e.ymax] {
                file.write_all(&v.to_le_bytes()).map_err(io)?;
            }
            if self.has_z {
                let (lo, hi) = e.z.or(old.z).unwrap_or((f64::NAN, f64::NAN));
                file.write_all(&lo.to_le_bytes()).map_err(io)?;
                file.write_all(&hi.to_le_bytes()).map_err(io)?;
            }
            if self.has_m {
                let (lo, hi) = e.m.or(old.m).unwrap_or((f64::NAN, f64::NAN));
                file.write_all(&lo.to_le_bytes()).map_err(io)?;
                file.write_all(&hi.to_le_bytes()).map_err(io)?;
            }
        }
        file.flush().map_err(io)?;
        write_index_for(
            &self.base.with_extension("spx"),
            8,
            0x40,
            &spatial_entries(&envelopes, grid),
            self.v4,
        )
    }
}

/// Deletes the rows of the table at `base` for which `doomed` is true (`arcpy.management.
/// DeleteRows` / `DeleteFeatures` on a selection); OBJECTIDs are not reused. Returns the
/// number of rows deleted.
pub fn delete_rows(
    base: impl AsRef<Path>,
    mut doomed: impl FnMut(&Feature) -> bool,
) -> Result<u64> {
    let mut ed = TableEditor::open(base)?;
    let oids: Vec<u64> = ed.oids().collect();
    let mut n = 0;
    for oid in oids {
        if let Some(f) = ed.get(oid)?
            && doomed(&f)
        {
            ed.delete(oid)?;
            n += 1;
        }
    }
    ed.finish()?;
    Ok(n)
}
