//! Restoring Z and M after a 2D overlay: each output vertex takes the values of the
//! source vertex it coincides with, or interpolates along the source segment it
//! lies on. Vertices created elsewhere (e.g. clip-polygon corners inside a polygon)
//! get Z = 0 and no measure.

use geoforge_core::Geometry;

pub fn restore(out: &mut Geometry, source: &Geometry, has_z: bool, has_m: bool) {
    if !has_z && !has_m {
        return;
    }
    let src_z = source.z.as_deref();
    let src_m = source.m.as_deref();
    let segments: Vec<(usize, usize)> = source
        .part_ranges()
        .flat_map(|r| (r.start..r.end.saturating_sub(1)).map(|i| (i, i + 1)))
        .collect();
    let env = source.envelope();
    let tol = 1e-9 * (env.xmax - env.xmin).hypot(env.ymax - env.ymin).max(1.0);
    let mut zs = Vec::with_capacity(out.xy.len());
    let mut ms = Vec::with_capacity(out.xy.len());
    for &p in &out.xy {
        let mut best: Option<(f64, usize, usize, f64)> = None;
        for &(a, b) in &segments {
            let (pa, pb) = (source.xy[a], source.xy[b]);
            let (dx, dy) = (pb[0] - pa[0], pb[1] - pa[1]);
            let len2 = dx * dx + dy * dy;
            let t = if len2 == 0.0 {
                0.0
            } else {
                (((p[0] - pa[0]) * dx + (p[1] - pa[1]) * dy) / len2).clamp(0.0, 1.0)
            };
            let d = (p[0] - pa[0] - t * dx).hypot(p[1] - pa[1] - t * dy);
            if best.is_none_or(|b| d < b.0) {
                best = Some((d, a, b, t));
            }
        }
        match best.filter(|b| b.0 <= tol) {
            Some((_, a, b, t)) => {
                let lerp = |v: Option<&[f64]>| v.map_or(f64::NAN, |v| v[a] + (v[b] - v[a]) * t);
                zs.push(if has_z { lerp(src_z) } else { 0.0 });
                ms.push(lerp(src_m));
            }
            None => {
                zs.push(0.0);
                ms.push(f64::NAN);
            }
        }
    }
    if has_z {
        out.z = Some(zs);
    }
    if has_m {
        out.m = Some(ms);
    }
}
