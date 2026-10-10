//! `.shp` / `.shx` binary layout: the 100-byte header and record contents.

use geoforge_core::{Envelope, Error, Geometry, GeometryKind, PartType, Result};

use crate::ShapeType;

pub const HEADER_LEN: usize = 100;
pub const FILE_CODE: i32 = 9994;
pub const VERSION: i32 = 1000;
/// Measures below this are "no data" (ESRI Shapefile Technical Description).
pub const NO_DATA_M_THRESHOLD: f64 = -1e38;

fn err(message: impl Into<String>) -> Error {
    Error::format("shapefile", message)
}

/// The header shared by `.shp` and `.shx`.
#[derive(Debug, Clone, PartialEq)]
pub struct Header {
    /// File length in bytes (stored on disk in 16-bit words).
    pub file_length: u64,
    pub shape_type: ShapeType,
    pub envelope: Envelope,
}

impl Header {
    pub fn parse(b: &[u8]) -> Result<Self> {
        if b.len() < HEADER_LEN {
            return Err(err("file shorter than the 100-byte header"));
        }
        let mut c = Cursor::new(b);
        if c.i32_be()? != FILE_CODE {
            return Err(err("bad file code (expected 9994)"));
        }
        c.skip(20)?;
        let file_length = u64::from(c.i32_be()? as u32) * 2;
        let _version = c.i32_le()?;
        let code = c.i32_le()?;
        let shape_type =
            ShapeType::from_code(code).ok_or_else(|| err(format!("unknown shape type {code}")))?;
        let [xmin, ymin, xmax, ymax, zmin, zmax, mmin, mmax] = c.f64s::<8>()?;
        let envelope = Envelope {
            xmin,
            ymin,
            xmax,
            ymax,
            z: shape_type.has_z().then_some((zmin, zmax)),
            m: (shape_type.has_m() && mmin > NO_DATA_M_THRESHOLD).then_some((mmin, mmax)),
        };
        Ok(Self {
            file_length,
            shape_type,
            envelope,
        })
    }

    pub fn write(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&FILE_CODE.to_be_bytes());
        out.extend_from_slice(&[0; 20]);
        out.extend_from_slice(&((self.file_length / 2) as u32).to_be_bytes());
        out.extend_from_slice(&VERSION.to_le_bytes());
        out.extend_from_slice(&self.shape_type.code().to_le_bytes());
        let e = &self.envelope;
        let (zmin, zmax) = e.z.unwrap_or((0.0, 0.0));
        let (mmin, mmax) = e.m.unwrap_or((0.0, 0.0));
        for v in [e.xmin, e.ymin, e.xmax, e.ymax, zmin, zmax, mmin, mmax] {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
}

/// Decodes one record's content (everything after the 8-byte record header).
/// Returns `None` for a null shape.
pub fn decode_record(content: &[u8]) -> Result<Option<Geometry>> {
    let mut c = Cursor::new(content);
    let code = c.i32_le()?;
    let st = ShapeType::from_code(code)
        .ok_or_else(|| err(format!("unknown record shape type {code}")))?;
    let Some(kind) = st.kind() else {
        return Ok(None);
    };
    let (has_z, has_m) = (st.has_z(), st.has_m());

    if kind == GeometryKind::Point {
        let [x, y] = c.f64s::<2>()?;
        // Esri stores an empty point as NaN coordinates.
        if x.is_nan() || y.is_nan() {
            return Ok(Some(Geometry {
                xy: Vec::new(),
                ..Geometry::point(0.0, 0.0)
            }));
        }
        let mut g = Geometry::point(x, y);
        if has_z {
            g.z = Some(vec![c.f64_le()?]);
        }
        if has_m {
            // M is optional for PointZ, and some writers omit it for PointM too.
            let m = if c.remaining() >= 8 {
                c.f64_le()?
            } else {
                f64::NAN
            };
            g.m = Some(vec![normalize_m(m)]);
        }
        return Ok(Some(g));
    }

    c.skip(32)?; // bounding box: recomputed from vertices when needed
    let (num_parts, num_points) = if kind == GeometryKind::Multipoint {
        (0, c.count()?)
    } else {
        (c.count()?, c.count()?)
    };
    // Points without parts cannot form a path or ring: read as empty (as GDAL does).
    if kind != GeometryKind::Multipoint && num_parts == 0 {
        return Ok(Some(Geometry {
            kind,
            parts: Vec::new(),
            part_types: (kind == GeometryKind::MultiPatch).then(Vec::new),
            xy: Vec::new(),
            z: has_z.then(Vec::new),
            m: has_m.then(Vec::new),
            curves: Vec::new(),
        }));
    }
    // Each part needs 4 bytes (+4 type for MultiPatch), each point 16.
    if num_parts * 4 + num_points * 16 > c.remaining() {
        return Err(err(format!(
            "record declares {num_parts} parts / {num_points} points but has only {} bytes",
            c.remaining()
        )));
    }

    let mut parts = Vec::with_capacity(num_parts);
    for _ in 0..num_parts {
        let start = c.i32_le()?;
        if start < 0 || start as usize > num_points {
            return Err(err(format!("part start {start} out of range")));
        }
        parts.push(start as u32);
    }
    if parts.first().is_some_and(|&p| p != 0) || parts.windows(2).any(|w| w[0] > w[1]) {
        return Err(err("part offsets must start at 0 and be non-decreasing"));
    }

    let part_types = if kind == GeometryKind::MultiPatch {
        let mut types = Vec::with_capacity(num_parts);
        for _ in 0..num_parts {
            let t = c.i32_le()?;
            types.push(
                PartType::from_code(t as u32)
                    .filter(|&t| t != PartType::Triangles)
                    .ok_or_else(|| err(format!("unknown multipatch part type {t}")))?,
            );
        }
        Some(types)
    } else {
        None
    };

    let mut xy = Vec::with_capacity(num_points);
    for _ in 0..num_points {
        xy.push(c.f64s::<2>()?);
    }
    let z = if has_z {
        c.skip(16)?; // Z range
        Some(c.f64_vec(num_points)?)
    } else {
        None
    };
    let m = if has_m {
        // Optional for Z types (and absent in some non-conformant M files).
        if c.remaining() >= 16 + num_points * 8 {
            c.skip(16)?;
            Some(
                c.f64_vec(num_points)?
                    .into_iter()
                    .map(normalize_m)
                    .collect(),
            )
        } else {
            Some(vec![f64::NAN; num_points])
        }
    } else {
        None
    };

    Ok(Some(Geometry {
        kind,
        parts,
        part_types,
        xy,
        z,
        m,
        curves: Vec::new(),
    }))
}

/// Encodes `geometry` as record content for a file of type `st`.
/// The caller guarantees `geometry.kind` matches `st`.
pub fn encode_record(st: ShapeType, geometry: Option<&Geometry>, out: &mut Vec<u8>) {
    let Some(g) = geometry else {
        out.extend_from_slice(&0i32.to_le_bytes());
        return;
    };
    let put = |out: &mut Vec<u8>, v: f64| out.extend_from_slice(&v.to_le_bytes());
    let m_at = |i: usize| g.m.as_ref().map_or(f64::NAN, |m| m[i]);
    out.extend_from_slice(&st.code().to_le_bytes());

    if g.kind == GeometryKind::Point {
        let [x, y] = g.xy.first().copied().unwrap_or([f64::NAN; 2]);
        put(out, x);
        put(out, y);
        if st.has_z() {
            put(
                out,
                g.z.as_ref().and_then(|z| z.first().copied()).unwrap_or(0.0),
            );
        }
        if st.has_m() {
            put(
                out,
                encode_m(if g.xy.is_empty() { f64::NAN } else { m_at(0) }),
            );
        }
        return;
    }

    let env = g.envelope();
    for v in [env.xmin, env.ymin, env.xmax, env.ymax] {
        put(out, v);
    }
    let n = g.xy.len();
    if g.kind != GeometryKind::Multipoint {
        out.extend_from_slice(&(g.parts.len() as i32).to_le_bytes());
    }
    out.extend_from_slice(&(n as i32).to_le_bytes());
    for &p in &g.parts {
        out.extend_from_slice(&(p as i32).to_le_bytes());
    }
    if let Some(types) = &g.part_types {
        for &t in types {
            out.extend_from_slice(&(t as i32).to_le_bytes());
        }
    }
    for &[x, y] in &g.xy {
        put(out, x);
        put(out, y);
    }
    if st.has_z() {
        let (lo, hi) = env.z.unwrap_or((0.0, 0.0));
        put(out, lo);
        put(out, hi);
        for i in 0..n {
            put(out, g.z.as_ref().map_or(0.0, |z| z[i]));
        }
    }
    if st.has_m() {
        let no_data = encode_m(f64::NAN);
        let (lo, hi) = env.m.unwrap_or((no_data, no_data));
        put(out, lo);
        put(out, hi);
        for i in 0..n {
            put(out, encode_m(m_at(i)));
        }
    }
}

fn normalize_m(m: f64) -> f64 {
    if m <= NO_DATA_M_THRESHOLD {
        f64::NAN
    } else {
        m
    }
}

/// On disk, "no measure" is any value below -1e38; ArcGIS writes -f64::MAX.
fn encode_m(m: f64) -> f64 {
    if m.is_nan() { -f64::MAX } else { m }
}

/// Bounds-checked little/big-endian reader over a byte slice.
pub(crate) struct Cursor<'a> {
    b: &'a [u8],
    p: usize,
}

impl<'a> Cursor<'a> {
    pub fn new(b: &'a [u8]) -> Self {
        Self { b, p: 0 }
    }

    pub fn remaining(&self) -> usize {
        self.b.len() - self.p
    }

    fn take<const N: usize>(&mut self) -> Result<[u8; N]> {
        let s = self
            .b
            .get(self.p..self.p + N)
            .ok_or_else(|| err("unexpected end of record"))?;
        self.p += N;
        Ok(s.try_into().unwrap())
    }

    pub fn skip(&mut self, n: usize) -> Result<()> {
        if n > self.remaining() {
            return Err(err("unexpected end of record"));
        }
        self.p += n;
        Ok(())
    }

    pub fn i32_be(&mut self) -> Result<i32> {
        Ok(i32::from_be_bytes(self.take()?))
    }

    pub fn i32_le(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.take()?))
    }

    pub fn f64_le(&mut self) -> Result<f64> {
        Ok(f64::from_le_bytes(self.take()?))
    }

    pub fn f64s<const N: usize>(&mut self) -> Result<[f64; N]> {
        let mut out = [0.0; N];
        for v in &mut out {
            *v = self.f64_le()?;
        }
        Ok(out)
    }

    fn f64_vec(&mut self, n: usize) -> Result<Vec<f64>> {
        (0..n).map(|_| self.f64_le()).collect()
    }

    /// A non-negative i32 count.
    fn count(&mut self) -> Result<usize> {
        let n = self.i32_le()?;
        usize::try_from(n).map_err(|_| err(format!("negative count {n}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(st: ShapeType, g: &Geometry) -> Geometry {
        let mut buf = Vec::new();
        encode_record(st, Some(g), &mut buf);
        decode_record(&buf).unwrap().unwrap()
    }

    #[test]
    fn point_zm_roundtrip_with_nodata_m() {
        let mut g = Geometry::point(1.0, 2.0);
        g.z = Some(vec![3.0]);
        g.m = Some(vec![f64::NAN]);
        let back = roundtrip(ShapeType::PointZ, &g);
        assert_eq!(back.xy, g.xy);
        assert_eq!(back.z, g.z);
        assert!(back.m.unwrap()[0].is_nan());
    }

    #[test]
    fn multipatch_roundtrip_keeps_part_types() {
        let g = Geometry {
            kind: GeometryKind::MultiPatch,
            parts: vec![0, 3],
            part_types: Some(vec![PartType::TriangleStrip, PartType::OuterRing]),
            xy: vec![
                [0.0, 0.0],
                [1.0, 0.0],
                [0.0, 1.0],
                [5.0, 5.0],
                [6.0, 5.0],
                [5.0, 5.0],
            ],
            z: Some(vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0]),
            m: Some(vec![1.0; 6]),
            curves: Vec::new(),
        };
        assert_eq!(roundtrip(ShapeType::MultiPatch, &g), g);
    }

    #[test]
    fn polyline_m_without_m_block_reads_as_nan() {
        let g = Geometry {
            kind: GeometryKind::Polyline,
            parts: vec![0],
            part_types: None,
            xy: vec![[0.0, 0.0], [1.0, 1.0]],
            z: None,
            m: None,
            curves: Vec::new(),
        };
        let mut buf = Vec::new();
        encode_record(ShapeType::PolyLine, Some(&g), &mut buf);
        buf[..4].copy_from_slice(&ShapeType::PolyLineM.code().to_le_bytes());
        let back = decode_record(&buf).unwrap().unwrap();
        assert!(back.m.unwrap().iter().all(|m| m.is_nan()));
    }

    #[test]
    fn empty_point_roundtrips_as_nan() {
        let empty = Geometry {
            xy: Vec::new(),
            ..Geometry::point(0.0, 0.0)
        };
        assert_eq!(
            roundtrip(ShapeType::Point, &empty).xy,
            Vec::<[f64; 2]>::new()
        );
    }

    #[test]
    fn truncated_record_is_an_error() {
        let mut buf = Vec::new();
        encode_record(ShapeType::Point, Some(&Geometry::point(1.0, 2.0)), &mut buf);
        assert!(decode_record(&buf[..10]).is_err());
    }
}
