use std::fs::File;
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};

use unigis_core::{Envelope, Error, Feature, FeatureClass, Field, GeometryKind, Result, Value};

use crate::dbf::DbfReader;
use crate::shp::{self, HEADER_LEN, Header};
use crate::{ShapeType, sibling};

/// Reads a shapefile (`.shp` + `.shx` + `.dbf` + `.prj` + `.cpg`).
///
/// Feature IDs are 0-based record numbers, as ArcGIS reports FID for shapefiles.
pub struct ShapefileReader {
    shp: File,
    shp_path: PathBuf,
    header: Header,
    /// (byte offset of the record header, content length) per record.
    index: Vec<(u64, u32)>,
    dbf: Option<DbfReader>,
    fields: Vec<Field>,
    prj: Option<String>,
}

impl ShapefileReader {
    /// Opens `path`, which may name the `.shp` or omit the extension.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let shp_path = sibling(path, "shp")
            .ok_or_else(|| Error::io(path, std::io::ErrorKind::NotFound.into()))?;
        let shp = File::open(&shp_path).map_err(|e| Error::io(&shp_path, e))?;
        let shp_len = shp.metadata().map_err(|e| Error::io(&shp_path, e))?.len();
        let mut head = [0u8; HEADER_LEN];
        shp.read_exact_at(&mut head, 0)
            .map_err(|e| Error::io(&shp_path, e))?;
        let header = Header::parse(&head)?;

        let index = match sibling(path, "shx") {
            Some(shx) => read_shx(&shx, shp_len).or_else(|_| scan_shp(&shp, &shp_path, shp_len))?,
            None => scan_shp(&shp, &shp_path, shp_len)?,
        };

        let read_text = |ext| {
            sibling(path, ext)
                .and_then(|p| std::fs::read(p).ok())
                .map(|b| {
                    String::from_utf8_lossy(&b)
                        .trim_start_matches('\u{feff}')
                        .trim()
                        .to_owned()
                })
        };
        let cpg = read_text("cpg");
        let prj = read_text("prj").filter(|s| !s.is_empty());
        let dbf = match sibling(path, "dbf") {
            Some(p) => Some(DbfReader::open(&p, cpg.as_deref())?),
            None => None,
        };
        let fields = dbf
            .as_ref()
            .map(|d| d.header().fields.iter().map(|f| f.to_field()).collect())
            .unwrap_or_default();

        Ok(Self {
            shp,
            shp_path,
            header,
            index,
            dbf,
            fields,
            prj,
        })
    }

    pub fn shape_type(&self) -> ShapeType {
        self.header.shape_type
    }

    /// The extent stored in the `.shp` header.
    pub fn extent(&self) -> Envelope {
        self.header.envelope
    }

    pub fn fields(&self) -> &[Field] {
        &self.fields
    }

    pub fn dbf(&self) -> Option<&DbfReader> {
        self.dbf.as_ref()
    }

    /// Contents of the `.prj` (Esri WKT1), if present.
    pub fn spatial_reference_wkt(&self) -> Option<&str> {
        self.prj.as_deref()
    }

    /// Number of records, including any marked deleted in the `.dbf`.
    pub fn record_count(&self) -> usize {
        self.index.len()
    }

    /// Reads record `fid`. Returns `None` if it is marked deleted.
    pub fn get(&self, fid: usize) -> Result<Option<Feature>> {
        let &(offset, len) = self
            .index
            .get(fid)
            .ok_or_else(|| Error::format("shapefile", format!("FID {fid} out of range")))?;
        let mut content = vec![0u8; len as usize];
        self.shp
            .read_exact_at(&mut content, offset + 8)
            .map_err(|e| Error::io(&self.shp_path, e))?;
        let geometry = shp::decode_record(&content)?;

        let attributes = match &self.dbf {
            Some(dbf) if (fid as u64) < u64::from(dbf.num_records()) => {
                let (values, deleted) = dbf.read(fid as u32)?;
                if deleted {
                    return Ok(None);
                }
                values
            }
            _ => vec![Value::Null; self.fields.len()],
        };
        Ok(Some(Feature {
            oid: fid as i64,
            geometry,
            attributes,
        }))
    }

    /// All features not marked deleted, in FID order.
    pub fn features(&self) -> impl Iterator<Item = Result<Feature>> + '_ {
        (0..self.index.len()).filter_map(|i| self.get(i).transpose())
    }
}

/// Parses `.shx`, rejecting it if any entry points outside the `.shp`.
fn read_shx(path: &Path, shp_len: u64) -> Result<Vec<(u64, u32)>> {
    let bytes = std::fs::read(path).map_err(|e| Error::io(path, e))?;
    Header::parse(&bytes)?;
    bytes[HEADER_LEN..]
        .as_chunks::<8>()
        .0
        .iter()
        .map(|e| {
            let offset = u64::from(u32::from_be_bytes(e[..4].try_into().unwrap())) * 2;
            let len = u32::from_be_bytes(e[4..].try_into().unwrap()).saturating_mul(2);
            if offset < HEADER_LEN as u64 || offset + 8 + u64::from(len) > shp_len {
                return Err(Error::format(
                    "shapefile",
                    "index entry points outside the .shp",
                ));
            }
            Ok((offset, len))
        })
        .collect()
}

/// Rebuilds the index by walking `.shp` record headers (missing or corrupt `.shx`).
fn scan_shp(shp: &File, path: &Path, shp_len: u64) -> Result<Vec<(u64, u32)>> {
    let mut index = Vec::new();
    let mut offset = HEADER_LEN as u64;
    let mut rec = [0u8; 8];
    while offset + 8 <= shp_len {
        shp.read_exact_at(&mut rec, offset)
            .map_err(|e| Error::io(path, e))?;
        let len = u32::from_be_bytes(rec[4..].try_into().unwrap()).saturating_mul(2);
        if offset + 8 + u64::from(len) > shp_len {
            break;
        }
        index.push((offset, len));
        offset += 8 + u64::from(len);
    }
    Ok(index)
}

impl FeatureClass for ShapefileReader {
    fn geometry_kind(&self) -> Option<GeometryKind> {
        self.shape_type().kind()
    }

    fn has_z(&self) -> bool {
        self.shape_type().has_z()
    }

    fn has_m(&self) -> bool {
        self.shape_type().has_m()
    }

    fn fields(&self) -> &[Field] {
        ShapefileReader::fields(self)
    }

    fn oid_field_name(&self) -> &str {
        "FID"
    }

    fn shape_field_name(&self) -> &str {
        "Shape"
    }

    fn spatial_reference_wkt(&self) -> Option<&str> {
        ShapefileReader::spatial_reference_wkt(self)
    }

    fn stored_extent(&self) -> Option<Envelope> {
        Some(self.extent())
    }

    fn features(&self) -> Box<dyn Iterator<Item = Result<Feature>> + '_> {
        Box::new(ShapefileReader::features(self))
    }
}
