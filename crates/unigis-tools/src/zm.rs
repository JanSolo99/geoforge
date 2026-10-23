//! Restoring Z and M after a 2D overlay: each output vertex takes the values of the
//! source vertex it coincides with, or interpolates along the source segment it
//! lies on: the first source in input order that has Z (or M) there. Vertices on
//! none (e.g. clip-polygon corners inside a polygon) get Z = 0 and no measure. Output vertices
//! within a hair of a source vertex are put back on it (the overlay's integer grid
//! moves them by ~1e-7 m).

use unigis_core::Geometry;

/// ArcGIS's default Z and M domain: false origin -100000, resolution 0.0001.
const ORIGIN: f64 = -100000.0;
const RESOLUTION: f64 = 0.0001;

/// A value on ArcGIS's Z/M grid, as its overlay tools write them.
fn snap(v: f64) -> f64 {
    if v.is_nan() {
        v
    } else {
        ORIGIN + ((v - ORIGIN) / RESOLUTION).round() * RESOLUTION
    }
}

/// Distance within which an output vertex is taken to lie on a source.
const ON: f64 = 1e-5;

/// Z and M for `out` from `sources` (in priority order); `grid` rounds them to
/// ArcGIS's Z/M resolution.
pub fn restore_from(
    out: &mut Geometry,
    sources: &[&Geometry],
    has_z: bool,
    has_m: bool,
    grid: bool,
) {
    if !has_z && !has_m {
        return;
    }
    let mut zs = Vec::with_capacity(out.xy.len());
    let mut ms = Vec::with_capacity(out.xy.len());
    for p in &mut out.xy {
        // Back onto a source vertex the overlay moved it off.
        if let Some(q) = sources
            .iter()
            .find_map(|s| locate(s, *p).and_then(|(.., v)| v.map(|v| s.xy[v])))
        {
            *p = q;
        }
        // Each value from the first source that has it and that the vertex lies on.
        let value = |pick: fn(&Geometry) -> Option<&Vec<f64>>| {
            sources.iter().find_map(|src| {
                let v = pick(src)?;
                let (a, b, t, vertex) = locate(src, *p)?;
                Some(match vertex {
                    Some(k) => v[k],
                    None => v[a] + (v[b] - v[a]) * t,
                })
            })
        };
        zs.push(value(|g| g.z.as_ref()).unwrap_or(0.0));
        ms.push(value(|g| g.m.as_ref()).unwrap_or(f64::NAN));
    }
    if grid {
        zs.iter_mut().for_each(|z| *z = snap(*z));
        ms.iter_mut().for_each(|m| *m = snap(*m));
    }
    out.z = has_z.then_some(zs);
    out.m = has_m.then_some(ms);
}

/// The source segment (a, b, parameter) `p` lies on, and the source vertex it
/// coincides with, if any.
fn locate(source: &Geometry, p: [f64; 2]) -> Option<(usize, usize, f64, Option<usize>)> {
    let close = |q: [f64; 2]| (p[0] - q[0]).hypot(p[1] - q[1]) <= ON;
    if let Some(k) = source.xy.iter().position(|&q| close(q)) {
        return Some((k, k, 0.0, Some(k)));
    }
    let mut best: Option<(f64, usize, usize, f64)> = None;
    for r in source.part_ranges() {
        for a in r.start..r.end.saturating_sub(1) {
            let (pa, pb) = (source.xy[a], source.xy[a + 1]);
            let (dx, dy) = (pb[0] - pa[0], pb[1] - pa[1]);
            let len2 = dx * dx + dy * dy;
            let t = if len2 == 0.0 {
                0.0
            } else {
                (((p[0] - pa[0]) * dx + (p[1] - pa[1]) * dy) / len2).clamp(0.0, 1.0)
            };
            let d = (p[0] - pa[0] - t * dx).hypot(p[1] - pa[1] - t * dy);
            if best.is_none_or(|b| d < b.0) {
                best = Some((d, a, a + 1, t));
            }
        }
    }
    best.filter(|b| b.0 <= ON)
        .map(|(_, a, b, t)| (a, b, t, None))
}
