//! FileGDB geometry blobs: scaled-integer, delta-encoded Esri shapes.

use unigis_core::{Geometry, GeometryKind, PartType, Result, Segment, SegmentKind};

use crate::bytes::{Reader, err};
use crate::table::GeometryDef;

const FLAG_Z: u64 = 0x8000_0000;
const FLAG_M: u64 = 0x4000_0000;
const FLAG_CURVES: u64 = 0x2000_0000;
/// Single byte written in place of an M array whose values are all NaN.
const ALL_NAN_M: u8 = 0x42;

struct Shape {
    kind: GeometryKind,
    z: bool,
    m: bool,
    curves: bool,
}

fn shape(code: u64) -> Result<Option<Shape>> {
    use GeometryKind::*;
    let base = code & 0xff;
    let (kind, z, m) = match base {
        0 => return Ok(None),
        1 => (Point, false, false),
        9 => (Point, true, false),
        21 => (Point, false, true),
        11 => (Point, true, true),
        8 => (Multipoint, false, false),
        20 => (Multipoint, true, false),
        28 => (Multipoint, false, true),
        18 => (Multipoint, true, true),
        3 => (Polyline, false, false),
        10 => (Polyline, true, false),
        23 => (Polyline, false, true),
        13 => (Polyline, true, true),
        5 => (Polygon, false, false),
        19 => (Polygon, true, false),
        25 => (Polygon, false, true),
        15 => (Polygon, true, true),
        32 => (MultiPatch, true, false),
        31 => (MultiPatch, true, true),
        50..=54 => {
            let kind = [Polyline, Polygon, Point, Multipoint, MultiPatch][(base - 50) as usize];
            let z = code & FLAG_Z != 0 || kind == MultiPatch;
            return Ok(Some(Shape {
                kind,
                z,
                m: code & FLAG_M != 0,
                curves: code & FLAG_CURVES != 0,
            }));
        }
        other => return Err(err(format!("unknown geometry type {other}"))),
    };
    Ok(Some(Shape {
        kind,
        z,
        m,
        curves: false,
    }))
}

/// Decodes a geometry blob. The result carries Z/M as the layer declares them:
/// missing Z becomes 0, missing M becomes NaN.
pub fn decode(
    blob: &[u8],
    def: &GeometryDef,
    layer_z: bool,
    layer_m: bool,
) -> Result<Option<Geometry>> {
    let mut r = Reader::new(blob);
    let Some(s) = shape(r.varuint()?)? else {
        return Ok(None);
    };
    let mut g = Geometry {
        kind: s.kind,
        parts: Vec::new(),
        part_types: None,
        xy: Vec::new(),
        z: None,
        m: None,
        curves: Vec::new(),
    };

    if s.kind == GeometryKind::Point {
        let x = r.varuint()?;
        let y = r.varuint()?;
        let z = if s.z { Some(r.varuint()?) } else { None };
        let m = if s.m && r.remaining() > 0 {
            Some(r.varuint()?)
        } else {
            None
        };
        if x != 0 && y != 0 {
            g.xy.push([
                (x - 1) as f64 / def.xy_scale + def.x_origin,
                (y - 1) as f64 / def.xy_scale + def.y_origin,
            ]);
            g.z = z.map(|v| vec![v.saturating_sub(1) as f64 / def.z_scale + def.z_origin]);
            g.m = m.map(|v| {
                vec![if v == 0 {
                    f64::NAN
                } else {
                    (v - 1) as f64 / def.m_scale + def.m_origin
                }]
            });
        }
        return Ok(Some(conform(g, layer_z, layer_m)));
    }

    let npts = r.count(2)?;
    if npts == 0 {
        return Ok(Some(conform(g, layer_z, layer_m)));
    }
    if s.kind == GeometryKind::MultiPatch {
        r.varuint()?; // size of the equivalent extended shape buffer (all multipatch types)
    }
    let nparts = if s.kind == GeometryKind::Multipoint {
        0
    } else {
        r.count(1)?
    };
    let ncurves = if s.curves { r.count(1)? } else { 0 };
    for _ in 0..4 {
        r.varuint()?; // bounding box
    }
    if s.kind != GeometryKind::Multipoint {
        if nparts == 0 {
            return Err(err("geometry with points but no parts"));
        }
        let mut start = 0usize;
        g.parts.push(0);
        for _ in 1..nparts {
            start += r.count(0)?;
            if start > npts {
                return Err(err("part point counts exceed the total"));
            }
            g.parts.push(start as u32);
        }
    }
    if s.kind == GeometryKind::MultiPatch {
        let mut types = Vec::with_capacity(nparts);
        for _ in 0..nparts {
            let t = r.varuint()? & 0xf;
            types.push(
                PartType::from_code(t as u32)
                    .ok_or_else(|| err(format!("unknown part type {t}")))?,
            );
        }
        g.part_types = Some(types);
    }

    let (mut dx, mut dy) = (0i64, 0i64);
    g.xy.reserve(npts);
    for _ in 0..npts {
        dx += r.varint()?;
        dy += r.varint()?;
        g.xy.push([
            dx as f64 / def.xy_scale + def.x_origin,
            dy as f64 / def.xy_scale + def.y_origin,
        ]);
    }
    if s.z {
        g.z = Some(deltas(&mut r, npts, def.z_scale, def.z_origin)?);
    }
    if s.m && r.remaining() > 0 {
        if r.peek() == Some(ALL_NAN_M) {
            r.u8()?;
            g.m = Some(vec![f64::NAN; npts]);
        } else {
            g.m = Some(deltas(&mut r, npts, def.m_scale, def.m_origin)?);
        }
    }
    for _ in 0..ncurves {
        g.curves.push(read_segment(&mut r)?);
    }
    Ok(Some(conform(g, layer_z, layer_m)))
}

fn deltas(r: &mut Reader, n: usize, scale: f64, origin: f64) -> Result<Vec<f64>> {
    let mut d = 0i64;
    (0..n)
        .map(|_| {
            d += r.varint()?;
            Ok(d as f64 / scale + origin)
        })
        .collect()
}

fn read_segment(r: &mut Reader) -> Result<Segment> {
    // Unlike the extended shape buffer, FileGDB stores these two as varuints.
    let start = u32::try_from(r.varuint()?).map_err(|_| err("curve start index overflow"))?;
    let kind = match r.varuint()? {
        1 => {
            let point = [r.f64()?, r.f64()?];
            SegmentKind::CircularArc {
                point,
                bits: r.u32()?,
            }
        }
        4 => SegmentKind::Bezier {
            controls: [[r.f64()?, r.f64()?], [r.f64()?, r.f64()?]],
        },
        5 => SegmentKind::EllipticArc {
            center: [r.f64()?, r.f64()?],
            rotation: r.f64()?,
            semi_major: r.f64()?,
            minor_major_ratio: r.f64()?,
            bits: r.u32()?,
        },
        t => return Err(err(format!("unknown curve segment type {t}"))),
    };
    Ok(Segment { start, kind })
}

fn conform(mut g: Geometry, layer_z: bool, layer_m: bool) -> Geometry {
    let n = g.xy.len();
    if layer_z && g.z.is_none() {
        g.z = Some(vec![0.0; n]);
    }
    if layer_m && g.m.is_none() {
        g.m = Some(vec![f64::NAN; n]);
    }
    g
}
