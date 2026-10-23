//! Encodes geometries as FileGDB blobs (the inverse of `crate::geometry`).

use unigis_core::{Error, Geometry, GeometryKind, Result, SegmentKind};

use super::table::{put_varint, put_varuint};
use crate::table::GeometryDef;

const FLAG_Z: u64 = 0x8000_0000;
const FLAG_M: u64 = 0x4000_0000;
const FLAG_CURVES: u64 = 0x2000_0000;
const ALL_NAN_M: u8 = 0x42;

fn err(message: impl Into<String>) -> Error {
    Error::format("FileGDB", message)
}

/// Scaled integer of `v`, which must lie inside the coordinate domain.
fn scaled(v: f64, origin: f64, scale: f64, what: &str) -> Result<i64> {
    let s = ((v - origin) * scale).round();
    if !s.is_finite() || s < 0.0 || s >= 9.0e15 {
        return Err(err(format!(
            "{what} {v} lies outside the spatial reference domain"
        )));
    }
    Ok(s as i64)
}

/// Writes `g` for a layer of `kind` with the layer's Z/M awareness.
pub fn encode(
    g: &Geometry,
    def: &GeometryDef,
    kind: Option<GeometryKind>,
    has_z: bool,
    has_m: bool,
    out: &mut Vec<u8>,
) -> Result<()> {
    if kind.is_some_and(|k| k != g.kind) {
        return Err(Error::Unsupported(format!(
            "{:?} geometry in a {:?} feature class",
            g.kind,
            kind.unwrap()
        )));
    }
    let n = g.xy.len();
    let z_at = |i: usize| g.z.as_ref().map_or(0.0, |z| z[i]);
    let m_at = |i: usize| g.m.as_ref().map_or(f64::NAN, |m| m[i]);
    let curves = !g.curves.is_empty();

    let code: u64 = match (g.kind, curves) {
        (GeometryKind::Point, _) => match (has_z, has_m) {
            (false, false) => 1,
            (true, false) => 9,
            (false, true) => 21,
            (true, true) => 11,
        },
        (GeometryKind::Multipoint, _) => match (has_z, has_m) {
            (false, false) => 8,
            (true, false) => 20,
            (false, true) => 28,
            (true, true) => 18,
        },
        (GeometryKind::Polyline, false) => match (has_z, has_m) {
            (false, false) => 3,
            (true, false) => 10,
            (false, true) => 23,
            (true, true) => 13,
        },
        (GeometryKind::Polygon, false) => match (has_z, has_m) {
            (false, false) => 5,
            (true, false) => 19,
            (false, true) => 25,
            (true, true) => 15,
        },
        (GeometryKind::Polyline, true) => {
            50 | FLAG_CURVES | if has_z { FLAG_Z } else { 0 } | if has_m { FLAG_M } else { 0 }
        }
        (GeometryKind::Polygon, true) => {
            51 | FLAG_CURVES | if has_z { FLAG_Z } else { 0 } | if has_m { FLAG_M } else { 0 }
        }
        (GeometryKind::MultiPatch, _) => 54 | FLAG_Z | if has_m { FLAG_M } else { 0 },
    };
    put_varuint(out, code);

    if g.kind == GeometryKind::Point {
        let Some(&[x, y]) = g.xy.first() else {
            // Empty point: zeros.
            out.extend(std::iter::repeat_n(
                0,
                2 + usize::from(has_z) + usize::from(has_m),
            ));
            return Ok(());
        };
        put_varuint(out, scaled(x, def.x_origin, def.xy_scale, "x")? as u64 + 1);
        put_varuint(out, scaled(y, def.y_origin, def.xy_scale, "y")? as u64 + 1);
        if has_z {
            put_varuint(
                out,
                scaled(z_at(0), def.z_origin, def.z_scale, "z")? as u64 + 1,
            );
        }
        if has_m {
            let m = m_at(0);
            put_varuint(
                out,
                if m.is_nan() {
                    0
                } else {
                    scaled(m, def.m_origin, def.m_scale, "m")? as u64 + 1
                },
            );
        }
        return Ok(());
    }

    // Paths and rings need parts; points without any are written as empty.
    if n == 0 || (g.kind != GeometryKind::Multipoint && g.parts.is_empty()) {
        put_varuint(out, 0);
        return Ok(());
    }
    put_varuint(out, n as u64);
    if g.kind == GeometryKind::MultiPatch {
        put_varuint(out, multipatch_buffer_size(g, has_m) as u64);
    }
    let parts: Vec<std::ops::Range<usize>> = g.part_ranges().collect();
    if g.kind != GeometryKind::Multipoint {
        put_varuint(out, parts.len() as u64);
    }
    if curves {
        put_varuint(out, g.curves.len() as u64);
    }
    let xs: Vec<i64> =
        g.xy.iter()
            .map(|p| scaled(p[0], def.x_origin, def.xy_scale, "x"))
            .collect::<Result<_>>()?;
    let ys: Vec<i64> =
        g.xy.iter()
            .map(|p| scaled(p[1], def.y_origin, def.xy_scale, "y"))
            .collect::<Result<_>>()?;
    let (xmin, xmax) = (*xs.iter().min().unwrap(), *xs.iter().max().unwrap());
    let (ymin, ymax) = (*ys.iter().min().unwrap(), *ys.iter().max().unwrap());
    put_varuint(out, xmin as u64);
    put_varuint(out, ymin as u64);
    put_varuint(out, (xmax - xmin) as u64);
    put_varuint(out, (ymax - ymin) as u64);
    if g.kind != GeometryKind::Multipoint {
        for r in &parts[..parts.len() - 1] {
            put_varuint(out, r.len() as u64);
        }
    }
    if let Some(types) = &g.part_types {
        for &t in types {
            put_varuint(out, t as u64);
        }
    }
    let (mut px, mut py) = (0i64, 0i64);
    for i in 0..n {
        put_varint(out, xs[i] - px);
        put_varint(out, ys[i] - py);
        (px, py) = (xs[i], ys[i]);
    }
    if has_z {
        let mut prev = 0i64;
        for i in 0..n {
            let v = scaled(z_at(i), def.z_origin, def.z_scale, "z")?;
            put_varint(out, v - prev);
            prev = v;
        }
    }
    if has_m {
        let nan = (0..n).filter(|&i| m_at(i).is_nan()).count();
        if nan == n {
            out.push(ALL_NAN_M);
        } else if nan > 0 {
            return Err(Error::Unsupported(
                "mixing missing and present M values in one geometry".into(),
            ));
        } else {
            let mut prev = 0i64;
            for i in 0..n {
                let v = scaled(m_at(i), def.m_origin, def.m_scale, "m")?;
                put_varint(out, v - prev);
                prev = v;
            }
        }
    }
    for s in &g.curves {
        put_varuint(out, u64::from(s.start));
        let put = |out: &mut Vec<u8>, v: f64| out.extend_from_slice(&v.to_le_bytes());
        match &s.kind {
            SegmentKind::CircularArc { point, bits } => {
                put_varuint(out, 1);
                put(out, point[0]);
                put(out, point[1]);
                out.extend_from_slice(&bits.to_le_bytes());
            }
            SegmentKind::Bezier { controls } => {
                put_varuint(out, 4);
                for c in controls {
                    put(out, c[0]);
                    put(out, c[1]);
                }
            }
            SegmentKind::EllipticArc {
                center,
                rotation,
                semi_major,
                minor_major_ratio,
                bits,
            } => {
                put_varuint(out, 5);
                for v in [
                    center[0],
                    center[1],
                    *rotation,
                    *semi_major,
                    *minor_major_ratio,
                ] {
                    put(out, v);
                }
                out.extend_from_slice(&bits.to_le_bytes());
            }
        }
    }
    Ok(())
}

/// Size of the same MultiPatch as an Esri extended shape buffer (shapefile layout).
fn multipatch_buffer_size(g: &Geometry, has_m: bool) -> usize {
    let (n, parts) = (g.xy.len(), g.parts.len());
    4 + 32
        + 4
        + 4
        + 4 * parts
        + 4 * parts
        + 16 * n
        + 16
        + 8 * n
        + if has_m { 16 + 8 * n } else { 0 }
}
