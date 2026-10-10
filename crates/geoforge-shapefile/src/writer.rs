use std::collections::HashSet;
use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use geoforge_core::{Envelope, Error, Feature, Field, Geometry, GeometryKind, PartType, Result};

use geoforge_core::curves::arcgis_densify_deviation;

use crate::ShapeType;
use crate::dbf::{DbfField, DbfWriter, MAX_FIELD_NAME_LEN};
use crate::shp::{self, HEADER_LEN, Header};

/// ArcGIS's limit for any shapefile component file.
pub const MAX_FILE_BYTES: u64 = (1 << 31) - 1;

/// Writes a shapefile the way ArcGIS does: clockwise exterior rings, 10-character
/// field names, UTF-8 `.cpg`, and ArcGIS null substitution in the `.dbf`.
pub struct ShapefileWriter {
    shape_type: ShapeType,
    shp: BufWriter<File>,
    shx: BufWriter<File>,
    shp_path: PathBuf,
    dbf: DbfWriter,
    shp_len: u64,
    extent: Envelope,
    count: u32,
    field_names: Vec<String>,
    record: Vec<u8>,
    densify_deviation: f64,
}

impl ShapefileWriter {
    /// Creates `path` (`.shp` extension optional) and its companion files, overwriting them.
    pub fn create(
        path: impl AsRef<Path>,
        kind: GeometryKind,
        has_z: bool,
        has_m: bool,
        fields: &[Field],
        spatial_reference_wkt: Option<&str>,
    ) -> Result<Self> {
        let base = path.as_ref().with_extension("");
        let file = |ext: &str| {
            let p = base.with_extension(ext);
            File::create(&p)
                .map(BufWriter::new)
                .map_err(|e| Error::io(&p, e))
        };
        let shape_type = ShapeType::for_geometry(kind, has_z, has_m);

        let field_names = validate_field_names(fields.iter().map(|f| f.name.as_str()));
        let dbf_fields = fields
            .iter()
            .zip(&field_names)
            .map(|(f, name)| DbfField::from_field(name.clone(), f))
            .collect::<Result<Vec<_>>>()?;
        let today = chrono::Local::now().date_naive();
        let dbf = DbfWriter::create(&base.with_extension("dbf"), dbf_fields, today)?;

        if let Some(wkt) = spatial_reference_wkt {
            let p = base.with_extension("prj");
            std::fs::write(&p, wkt).map_err(|e| Error::io(&p, e))?;
        }
        let cpg = base.with_extension("cpg");
        std::fs::write(&cpg, "UTF-8").map_err(|e| Error::io(&cpg, e))?;

        let mut shp = file("shp")?;
        let mut shx = file("shx")?;
        let placeholder = [0u8; HEADER_LEN];
        let shp_path = base.with_extension("shp");
        shp.write_all(&placeholder)
            .map_err(|e| Error::io(&shp_path, e))?;
        shx.write_all(&placeholder)
            .map_err(|e| Error::io(&shp_path, e))?;

        Ok(Self {
            shape_type,
            shp,
            shx,
            shp_path,
            dbf,
            shp_len: HEADER_LEN as u64,
            extent: Envelope::EMPTY,
            count: 0,
            field_names,
            record: Vec::new(),
            densify_deviation: arcgis_densify_deviation(
                spatial_reference_wkt.is_some_and(|w| w.trim_start().starts_with("GEOGCS")),
            ),
        })
    }

    pub fn shape_type(&self) -> ShapeType {
        self.shape_type
    }

    /// The `.dbf` field names actually written, in input field order.
    pub fn field_names(&self) -> &[String] {
        &self.field_names
    }

    /// Appends a feature. `feature.attributes` follow the field order given to `create`.
    pub fn write(&mut self, feature: &Feature) -> Result<()> {
        // Encode the attributes first: a value that does not fit means the row is
        // skipped entirely (ArcGIS WARNING 160277), so nothing may be written yet.
        self.dbf.encode(&feature.attributes)?;
        let geometry = match &feature.geometry {
            Some(g) => Some(self.conform(g)?),
            None => None,
        };
        self.record.clear();
        shp::encode_record(self.shape_type, geometry.as_ref(), &mut self.record);
        let content_len = self.record.len() as u64;
        let new_shp_len = self.shp_len + 8 + content_len;
        let new_dbf_len = self.dbf.len_bytes() + self.dbf.record_len();
        if new_shp_len > MAX_FILE_BYTES || new_dbf_len > MAX_FILE_BYTES {
            return Err(Error::Limit(
                "shapefile component files are limited to 2 GB".into(),
            ));
        }

        let io = |e| Error::io(&self.shp_path, e);
        let words = |n: u64| ((n / 2) as u32).to_be_bytes();
        self.shx.write_all(&words(self.shp_len)).map_err(io)?;
        self.shx.write_all(&words(content_len)).map_err(io)?;
        self.shp
            .write_all(&(self.count + 1).to_be_bytes())
            .map_err(io)?;
        self.shp.write_all(&words(content_len)).map_err(io)?;
        self.shp.write_all(&self.record).map_err(io)?;
        self.dbf.write_encoded()?;

        if let Some(g) = &geometry {
            self.extent.union(&g.envelope());
        }
        self.shp_len = new_shp_len;
        self.count += 1;
        Ok(())
    }

    /// Makes `g` storable in this file's shape type: matching Z/M, Esri ring order.
    fn conform(&self, g: &Geometry) -> Result<Geometry> {
        let st = self.shape_type;
        if Some(g.kind) != st.kind() {
            return Err(Error::Unsupported(format!(
                "{:?} geometry in a {} shapefile",
                g.kind,
                st.esri_name()
            )));
        }
        // Shapefiles cannot store true curves; ArcGIS densifies them on export.
        let mut g = g.densify_curves_arcgis(self.densify_deviation)?;
        let n = g.xy.len();
        if !st.has_z() {
            g.z = None;
        } else if g.z.is_none() {
            g.z = Some(vec![0.0; n]);
        }
        if !st.has_m() {
            g.m = None;
        } else if g.m.is_none() {
            g.m = Some(vec![f64::NAN; n]);
        }
        g.orient_rings_esri();
        split_triangles(&mut g);
        Ok(g)
    }

    /// Writes the headers. Must be called; dropping the writer leaves invalid files.
    pub fn finish(mut self) -> Result<()> {
        let mut extent = if self.extent.is_empty() {
            Envelope {
                xmin: 0.0,
                ymin: 0.0,
                xmax: 0.0,
                ymax: 0.0,
                z: None,
                m: None,
            }
        } else {
            self.extent
        };
        if self.shape_type.has_z() && extent.z.is_none() {
            extent.z = Some((0.0, 0.0));
        }
        let header = |file_length| {
            let mut out = Vec::with_capacity(HEADER_LEN);
            Header {
                file_length,
                shape_type: self.shape_type,
                envelope: extent,
            }
            .write(&mut out);
            out
        };
        let shp_header = header(self.shp_len);
        let shx_header = header(HEADER_LEN as u64 + 8 * u64::from(self.count));
        let io = |e| Error::io(&self.shp_path, e);
        for (f, h) in [(&mut self.shp, shp_header), (&mut self.shx, shx_header)] {
            f.seek(SeekFrom::Start(0)).map_err(io)?;
            f.write_all(&h).map_err(io)?;
            f.flush().map_err(io)?;
        }
        self.dbf.finish()
    }
}

/// Shapefiles have no `Triangles` MultiPatch part (FileGDB only); each triangle
/// becomes its own three-point triangle strip, which describes the same surface.
fn split_triangles(g: &mut Geometry) {
    let Some(types) = &g.part_types else { return };
    if !types.contains(&PartType::Triangles) {
        return;
    }
    let ranges: Vec<_> = g.part_ranges().collect();
    let mut parts = Vec::new();
    let mut new_types = Vec::new();
    for (r, &t) in ranges.into_iter().zip(types) {
        if t == PartType::Triangles {
            for start in r.step_by(3) {
                parts.push(start as u32);
                new_types.push(PartType::TriangleStrip);
            }
        } else {
            parts.push(r.start as u32);
            new_types.push(t);
        }
    }
    g.parts = parts;
    g.part_types = Some(new_types);
}

/// ArcGIS field-name rules for shapefiles: ASCII letters, digits and `_`; must not
/// start with a digit; at most 10 characters; unique (case-insensitively), with
/// collisions renamed `NAME_1`, `NAME_2`, ...
pub fn validate_field_names<'a>(names: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut seen = HashSet::new();
    names
        .into_iter()
        .map(|name| {
            let mut base: String = name
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || c == '_' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect();
            if base.is_empty() || base.starts_with(|c: char| c.is_ascii_digit()) {
                base.insert(0, 'F');
            }
            base.truncate(MAX_FIELD_NAME_LEN);
            let mut candidate = base.clone();
            let mut n = 1;
            while !seen.insert(candidate.to_ascii_uppercase()) {
                let suffix = format!("_{n}");
                let keep = MAX_FIELD_NAME_LEN - suffix.len();
                candidate = format!("{}{suffix}", &base[..base.len().min(keep)]);
                n += 1;
            }
            candidate
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_names_follow_arcgis_rules() {
        let names = validate_field_names([
            "population_2020",
            "population_2021",
            "2nd value",
            "Name",
            "NAME",
            "naïve",
        ]);
        assert_eq!(
            names,
            [
                "population",
                "populati_1",
                "F2nd_value",
                "Name",
                "NAME_1",
                "na_ve"
            ]
        );
    }
}
