//! Tools that build geometry from simple parameters or reshape lines: Densify, Generate Points
//! Along Lines, Create Fishnet and Polygon To Line.
//!
//! Matched to ArcGIS Pro 3.7 (`testdata/arcgis-reference/construct.json`):
//! - Densify splits every segment longer than `distance` (coordinate units) into `floor(length /
//!   distance) + 1` equal parts, keeping its vertices; a segment no longer than `distance` stays whole.
//! - Generate Points Along Lines walks the parts of a feature one after the other, as if joined end
//!   to end (the gaps between parts do not count; polygons use their rings, holes included, in the
//!   stored order). Points go at `d, 2d, ...` strictly before the total length (percentages: `p,
//!   2p, ...` strictly below 100 %). END_POINTS adds the first vertex of the first part and the last
//!   vertex of the last (twice on a closed ring). Fields: `Id` (0), `ORIG_FID`, then the input's
//!   fields (`Id` becomes `Id_1`).
//! - Create Fishnet makes `rows x columns` cells, a row at a time from the origin, along the
//!   y-axis direction given by the second point (`x` is that direction turned a quarter clockwise); a
//!   corner point replaces the cell size. Polylines are the `rows + 1` horizontal lines, then the
//!   `columns + 1` vertical ones. Labels are not made.
//! - Polygon To Line (IGNORE_NEIGHBORS): the rings of each polygon as the parts of one line, with
//!   `ORIG_FID` last. IDENTIFY_NEIGHBORS is in [`crate::planar`].

use unigis_core::{
    Error, Feature, FeatureClass, Field, FieldType, Geometry, GeometryKind, Result, Value,
};

use crate::{ToolOutput, push_unique};

fn err(msg: impl Into<String>) -> Error {
    Error::Unsupported(msg.into())
}

fn id_field() -> Field {
    let mut f = Field::new("Id", FieldType::Integer);
    f.precision = 6;
    f
}

/// The parts of a polyline or polygon as vertex runs.
fn parts(g: &Geometry) -> Vec<&[[f64; 2]]> {
    let starts = if g.parts.is_empty() {
        vec![0u32]
    } else {
        g.parts.clone()
    };
    starts
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let end = starts.get(i + 1).map_or(g.xy.len(), |e| *e as usize);
            &g.xy[*s as usize..end]
        })
        .collect()
}

fn require_lines_or_polygons(input: &dyn FeatureClass) -> Result<GeometryKind> {
    match input.geometry_kind() {
        Some(k @ (GeometryKind::Polyline | GeometryKind::Polygon)) => Ok(k),
        _ => Err(err("ERROR 000368: the input must be lines or polygons")),
    }
}

fn shape(kind: GeometryKind, runs: Vec<Vec<[f64; 2]>>) -> Geometry {
    let mut g = Geometry::point(0.0, 0.0);
    g.kind = kind;
    g.parts = Vec::new();
    g.xy = Vec::new();
    for r in runs {
        g.parts.push(g.xy.len() as u32);
        g.xy.extend(r);
    }
    g
}

/// Densify (`distance` in coordinate units): returns a copy with the new vertices.
pub fn densify(input: &dyn FeatureClass, distance: f64) -> Result<ToolOutput> {
    let kind = require_lines_or_polygons(input)?;
    if !(distance.is_finite() && distance > 0.0) {
        return Err(err("ERROR 000800: the distance must be greater than zero"));
    }
    let mut features = Vec::new();
    for f in input.features() {
        let mut f = f?;
        if let Some(g) = &f.geometry {
            if g.has_curves() {
                return Err(err("ERROR 000860: Densify does not handle curves yet"));
            }
            let runs = parts(g)
                .into_iter()
                .map(|run| {
                    let mut out = Vec::new();
                    for (i, a) in run.iter().enumerate() {
                        out.push(*a);
                        if let Some(b) = run.get(i + 1) {
                            let len = (b[0] - a[0]).hypot(b[1] - a[1]);
                            // A segment no longer than the distance stays whole; a longer one is cut into
                            // floor(length / distance) + 1 parts, so a multiple is split once more (observed).
                            let n = if len <= distance * (1.0 + 1e-12) {
                                1
                            } else {
                                (len / distance + 1e-9).floor() as usize + 1
                            };
                            for k in 1..n {
                                let t = k as f64 / n as f64;
                                out.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
                            }
                        }
                    }
                    out
                })
                .collect();
            let mut out = shape(kind, runs);
            out.z = None;
            out.m = None;
            f.geometry = Some(out);
        }
        features.push(f);
    }
    Ok(ToolOutput {
        kind,
        has_z: false,
        has_m: false,
        fields: input.fields().to_vec(),
        spatial_reference_wkt: input.spatial_reference_wkt().map(str::to_owned),
        features,
        messages: Vec::new(),
    })
}

/// How Generate Points Along Lines places its points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Placement {
    /// Every `d` units.
    Distance(f64),
    /// Every `p` percent of the length.
    Percentage(f64),
}

/// Generate Points Along Lines.
pub fn generate_points_along_lines(
    input: &dyn FeatureClass,
    placement: Placement,
    end_points: bool,
    chainage: bool,
) -> Result<ToolOutput> {
    require_lines_or_polygons(input)?;
    let step_ok = match placement {
        Placement::Distance(d) => d.is_finite() && d > 0.0,
        Placement::Percentage(p) => p.is_finite() && p > 0.0 && p < 100.0,
    };
    if !step_ok {
        return Err(err(
            "ERROR 000800: the distance must be greater than zero (a percentage between 0 and 100)",
        ));
    }
    let mut fields = vec![id_field(), Field::new("ORIG_FID", FieldType::Integer)];
    if chainage {
        // The distance of the point from the start of the line, and its number along the line.
        fields.push(Field::new("ORIG_LEN", FieldType::Double));
        fields.push(Field::new("ORIG_SEQ", FieldType::Integer));
    }
    for f in input.fields() {
        push_unique(&mut fields, f.clone());
    }
    let mut features = Vec::new();
    for f in input.features() {
        let f = f?;
        let Some(g) = &f.geometry else { continue };
        let runs = parts(g);
        let total: f64 = runs
            .iter()
            .map(|r| {
                r.windows(2)
                    .map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]))
                    .sum::<f64>()
            })
            .sum();
        let step = match placement {
            Placement::Distance(d) => d,
            Placement::Percentage(p) => total * p / 100.0,
        };
        let mut points: Vec<([f64; 2], f64)> = Vec::new();
        if end_points && let Some(p) = runs.iter().find_map(|r| r.first()) {
            points.push((*p, 0.0));
        }
        // Positions along the parts laid end to end.
        let mut next = step;
        let mut before = 0.0;
        let eps = 1e-9 * total.max(1.0);
        'walk: for r in &runs {
            for w in r.windows(2) {
                let len = (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]);
                while next < before + len + eps && next < total - eps {
                    let t = if len == 0.0 {
                        0.0
                    } else {
                        ((next - before) / len).clamp(0.0, 1.0)
                    };
                    points.push((
                        [
                            w[0][0] + (w[1][0] - w[0][0]) * t,
                            w[0][1] + (w[1][1] - w[0][1]) * t,
                        ],
                        next,
                    ));
                    next += step;
                    if step <= 0.0 {
                        break 'walk;
                    }
                }
                before += len;
            }
        }
        if end_points && let Some(p) = runs.iter().rev().find_map(|r| r.last()) {
            points.push((*p, total));
        }
        for (seq, (p, along)) in points.into_iter().enumerate() {
            let mut attributes = vec![Value::Integer(0), Value::Integer(f.oid as i32)];
            if chainage {
                attributes.push(Value::Double(along));
                attributes.push(Value::Integer(seq as i32 + 1));
            }
            attributes.extend(f.attributes.iter().cloned());
            features.push(Feature {
                oid: features.len() as i64,
                geometry: Some(Geometry::point(p[0], p[1])),
                attributes,
            });
        }
    }
    Ok(ToolOutput {
        kind: GeometryKind::Point,
        has_z: false,
        has_m: false,
        fields,
        spatial_reference_wkt: input.spatial_reference_wkt().map(str::to_owned),
        features,
        messages: Vec::new(),
    })
}

/// Generate Transects Along Lines: a `length` long line across the input every `interval` along
/// it (and, with `end_points`, at its start and end, listed last). The transect is perpendicular
/// to the segment at that place, drawn from its lower to its upper side: the left normal of the
/// travel direction, turned so that it does not point down (observed).
pub fn generate_transects_along_lines(
    input: &dyn FeatureClass,
    interval: f64,
    length: f64,
    end_points: bool,
) -> Result<ToolOutput> {
    if input.geometry_kind() != Some(GeometryKind::Polyline) {
        return Err(err("ERROR 000368: Invalid input data: lines are required"));
    }
    if !(interval.is_finite() && interval > 0.0 && length.is_finite() && length > 0.0) {
        return Err(err(
            "ERROR 000800: the interval and the length must be greater than zero",
        ));
    }
    let mut features = Vec::new();
    for f in input.features() {
        let f = f?;
        let Some(g) = &f.geometry else { continue };
        let runs = parts(g);
        // (start of the segment along the line, length, from, to)
        let mut segments = Vec::new();
        let mut total = 0.0;
        for r in &runs {
            for w in r.windows(2) {
                let len = (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]);
                if len > 0.0 {
                    segments.push((total, len, w[0], w[1]));
                    total += len;
                }
            }
        }
        if segments.is_empty() {
            continue;
        }
        let at = |d: f64| -> ([f64; 2], [f64; 2]) {
            let seg = segments
                .iter()
                .find(|s| d <= s.0 + s.1)
                .unwrap_or(&segments[segments.len() - 1]);
            let t = ((d - seg.0) / seg.1).clamp(0.0, 1.0);
            let dir = [(seg.3[0] - seg.2[0]) / seg.1, (seg.3[1] - seg.2[1]) / seg.1];
            (
                [
                    seg.2[0] + (seg.3[0] - seg.2[0]) * t,
                    seg.2[1] + (seg.3[1] - seg.2[1]) * t,
                ],
                dir,
            )
        };
        let mut distances = Vec::new();
        let mut d = interval;
        while d < total - 1e-9 * total.max(1.0) {
            distances.push(d);
            d += interval;
        }
        if end_points {
            distances.push(0.0);
            distances.push(total);
        }
        for d in distances {
            let (p, dir) = at(d);
            let mut n = [-dir[1], dir[0]];
            if n[1] < 0.0 || (n[1] == 0.0 && n[0] < 0.0) {
                n = [-n[0], -n[1]];
            }
            let h = length / 2.0;
            let line = unigis_geometry::paths_geometry(vec![vec![
                [p[0] - n[0] * h, p[1] - n[1] * h],
                [p[0] + n[0] * h, p[1] + n[1] * h],
            ]]);
            features.push(Feature {
                oid: features.len() as i64,
                geometry: line,
                attributes: vec![Value::Integer(0), Value::Integer(f.oid as i32)],
            });
        }
    }
    Ok(ToolOutput {
        kind: GeometryKind::Polyline,
        has_z: false,
        has_m: false,
        fields: vec![id_field(), Field::new("ORIG_FID", FieldType::Integer)],
        spatial_reference_wkt: input.spatial_reference_wkt().map(str::to_owned),
        features,
        messages: Vec::new(),
    })
}

/// A copy of the lines with each part's vertices rewritten by `edit` (Flip Line, Generalize).
fn edit_line_parts(
    input: &dyn FeatureClass,
    mut edit: impl FnMut(&[[f64; 2]]) -> Vec<[f64; 2]>,
) -> Result<ToolOutput> {
    if input.geometry_kind() != Some(GeometryKind::Polyline) {
        return Err(err("ERROR 000368: Invalid input data: lines are required"));
    }
    let mut features = Vec::new();
    for f in input.features() {
        let mut f = f?;
        if let Some(g) = &mut f.geometry {
            if g.z.is_some() || g.m.is_some() || g.has_curves() {
                return Err(err("this tool does not handle Z, M or curves yet"));
            }
            let mut xy = Vec::new();
            let mut starts = Vec::new();
            for r in g.part_ranges().collect::<Vec<_>>() {
                starts.push(xy.len() as u32);
                xy.extend(edit(&g.xy[r]));
            }
            g.xy = xy;
            g.parts = starts;
        }
        features.push(f);
    }
    Ok(ToolOutput {
        kind: GeometryKind::Polyline,
        has_z: false,
        has_m: false,
        fields: input.fields().to_vec(),
        spatial_reference_wkt: input.spatial_reference_wkt().map(str::to_owned),
        features,
        messages: Vec::new(),
    })
}

/// Flip Line: the direction of every line reversed.
pub fn flip_line(input: &dyn FeatureClass) -> Result<ToolOutput> {
    edit_line_parts(input, |p| p.iter().rev().copied().collect())
}

/// Generalize: vertices within `tolerance` of the line through the kept ones are removed
/// (Douglas-Peucker; same vertices as ArcGIS, observed).
pub fn generalize(input: &dyn FeatureClass, tolerance: f64) -> Result<ToolOutput> {
    edit_line_parts(input, |p| {
        let mut keep = vec![false; p.len()];
        if let Some(k) = keep.first_mut() {
            *k = true;
        }
        if let Some(k) = keep.last_mut() {
            *k = true;
        }
        if p.len() > 2 {
            crate::neighbors::douglas_peucker(p, tolerance, &mut keep, 0, p.len() - 1);
        }
        p.iter()
            .zip(&keep)
            .filter(|(_, k)| **k)
            .map(|(v, _)| *v)
            .collect()
    })
}

/// The cells Generate Tessellation makes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tessellation {
    Square,
    Diamond,
    Hexagon,
    TransverseHexagon,
    Triangle,
}

/// Generate Tessellation (square, diamond, both hexagon orientations, triangle): cells of `area` whose first one is centred on
/// the extent's lower-left corner; squares and diamonds/hexagons continue while their edges
/// start before the extent's far edges (diamonds and hexagons by the rule `(span + half cell) /
/// spacing`, observed on three extents). `GRID_ID` is `<column letters>-<row from the top>`; the
/// cells are listed from the bottom row up, left to right, each ring clockwise (squares from the
/// top right, diamonds from the top, hexagons from the east vertex).
pub fn generate_tessellation(
    shape: Tessellation,
    extent: [f64; 4],
    area: f64,
    wkt: Option<String>,
) -> Result<ToolOutput> {
    let (w, h) = (extent[2] - extent[0], extent[3] - extent[1]);
    if !(area.is_finite() && area > 0.0 && w.is_finite() && w > 0.0 && h.is_finite() && h > 0.0) {
        return Err(err(
            "ERROR 000800: the size and the extent must be finite and positive",
        ));
    }
    // (columns, rows, centre of the cell in column c, row r, ring around the centre)
    let (cols, rows): (usize, usize);
    let centre: Box<dyn Fn(usize, usize) -> [f64; 2]>;
    let ring: Box<dyn Fn([f64; 2]) -> Vec<[f64; 2]>>;
    match shape {
        Tessellation::Square => {
            let side = area.sqrt();
            let count = |span: f64| {
                (((span - side / 2.0) / side).ceil().max(0.0) as usize).saturating_add(1)
            };
            (cols, rows) = (count(w), count(h));
            let (x0, y0) = (extent[0], extent[1]);
            centre = Box::new(move |c, r| [x0 + c as f64 * side, y0 + r as f64 * side]);
            ring = Box::new(move |m| {
                let q = side / 2.0;
                vec![
                    [m[0] + q, m[1] + q],
                    [m[0] + q, m[1] - q],
                    [m[0] - q, m[1] - q],
                    [m[0] - q, m[1] + q],
                    [m[0] + q, m[1] + q],
                ]
            });
        }
        Tessellation::Diamond => {
            let d = (area / 2.0).sqrt();
            cols = ((w + d) / (2.0 * d)).ceil().max(1.0) as usize;
            rows = ((h + d) / d).ceil().max(1.0) as usize;
            let (x0, y0) = (extent[0], extent[1]);
            centre = Box::new(move |c, r| {
                [
                    x0 + 2.0 * d * c as f64 + if r % 2 == 1 { d } else { 0.0 },
                    y0 + d * r as f64,
                ]
            });
            ring = Box::new(move |m| {
                vec![
                    [m[0], m[1] + d],
                    [m[0] + d, m[1]],
                    [m[0], m[1] - d],
                    [m[0] - d, m[1]],
                    [m[0], m[1] + d],
                ]
            });
        }
        Tessellation::Hexagon => {
            let side = (area / (1.5 * 3f64.sqrt())).sqrt();
            let height = 3f64.sqrt() * side;
            cols = ((w + side) / (1.5 * side)).ceil().max(1.0) as usize;
            rows = ((h + height / 2.0) / height).ceil().max(1.0) as usize;
            let (x0, y0) = (extent[0], extent[1]);
            centre = Box::new(move |c, r| {
                [
                    x0 + 1.5 * side * c as f64,
                    y0 + height * r as f64 + if c % 2 == 1 { height / 2.0 } else { 0.0 },
                ]
            });
            ring = Box::new(move |m| {
                let q = height / 2.0;
                vec![
                    [m[0] + side, m[1]],
                    [m[0] + side / 2.0, m[1] - q],
                    [m[0] - side / 2.0, m[1] - q],
                    [m[0] - side, m[1]],
                    [m[0] - side / 2.0, m[1] + q],
                    [m[0] + side / 2.0, m[1] + q],
                    [m[0] + side, m[1]],
                ]
            });
        }
        Tessellation::TransverseHexagon => {
            let side = (area / (1.5 * 3f64.sqrt())).sqrt();
            let width = 3f64.sqrt() * side;
            cols = ((w + width / 2.0) / width).ceil().max(1.0) as usize;
            rows = ((h + side) / (1.5 * side)).ceil().max(1.0) as usize;
            let (x0, y0) = (extent[0], extent[1]);
            centre = Box::new(move |c, r| {
                [
                    x0 + width * c as f64 + if r % 2 == 1 { width / 2.0 } else { 0.0 },
                    y0 + 1.5 * side * r as f64,
                ]
            });
            ring = Box::new(move |m| {
                let q = width / 2.0;
                vec![
                    [m[0], m[1] + side],
                    [m[0] + q, m[1] + side / 2.0],
                    [m[0] + q, m[1] - side / 2.0],
                    [m[0], m[1] - side],
                    [m[0] - q, m[1] - side / 2.0],
                    [m[0] - q, m[1] + side / 2.0],
                    [m[0], m[1] + side],
                ]
            });
        }
        Tessellation::Triangle => {
            // Equilateral triangles whose first (pointing up) has its centroid on the extent's
            // lower-left corner; rows of alternating up / down triangles, odd rows shifted half
            // a triangle.
            let a = (area * 4.0 / 3f64.sqrt()).sqrt();
            let ht = 3f64.sqrt() / 2.0 * a;
            cols = ((w + a / 2.0) / (a / 2.0)).ceil().max(1.0) as usize;
            rows = ((h + ht / 3.0) / ht).ceil().max(1.0) as usize;
            let (x0, y0) = (extent[0], extent[1]);
            // `centre` gives a triangle's left vertex x and bottom y; `ring` recovers its column
            // and row from them to know whether it points up (column + row even) or down.
            centre = Box::new(move |c, r| {
                [
                    x0 - a / 2.0 + a / 2.0 * c as f64,
                    y0 - ht / 3.0 + ht * r as f64,
                ]
            });
            ring = Box::new(move |m| {
                let col = ((m[0] - (x0 - a / 2.0)) / (a / 2.0)).round() as i64;
                let row = ((m[1] - (y0 - ht / 3.0)) / ht).round() as i64;
                let up = (col + row).rem_euclid(2) == 0;
                let (xl, yb, yt) = (m[0], m[1], m[1] + ht);
                if up {
                    vec![
                        [xl + a / 2.0, yt],
                        [xl + a, yb],
                        [xl, yb],
                        [xl + a / 2.0, yt],
                    ]
                } else {
                    vec![[xl, yt], [xl + a, yt], [xl + a / 2.0, yb], [xl, yt]]
                }
            });
        }
    }
    if cols.saturating_mul(rows) > 5_000_000 {
        return Err(err("ERROR 000800: too many cells"));
    }
    let letters = |mut n: usize| {
        let mut s = Vec::new();
        loop {
            s.push(b'A' + (n % 26) as u8);
            if n < 26 {
                break;
            }
            n = n / 26 - 1;
        }
        s.reverse();
        String::from_utf8(s).unwrap_or_default()
    };
    let mut grid_id = Field::new("GRID_ID", FieldType::String);
    grid_id.length = 12;
    let mut features = Vec::with_capacity(cols * rows);
    for r in 0..rows {
        for c in 0..cols {
            let mut g = unigis_geometry::paths_geometry(vec![ring(centre(c, r))]);
            if let Some(g) = &mut g {
                g.kind = GeometryKind::Polygon;
            }
            features.push(Feature {
                oid: features.len() as i64,
                geometry: g,
                attributes: vec![
                    Value::Integer(0),
                    Value::String(format!("{}-{}", letters(c), rows - r)),
                ],
            });
        }
    }
    Ok(ToolOutput {
        kind: GeometryKind::Polygon,
        has_z: false,
        has_m: false,
        fields: vec![id_field(), grid_id],
        spatial_reference_wkt: wkt,
        features,
        messages: Vec::new(),
    })
}

/// What Create Fishnet needs: the origin, the second point of the y-axis and either the cell size
/// with the cell counts or an opposite corner with them.
#[derive(Debug, Clone, Copy)]
pub struct Fishnet {
    pub origin: [f64; 2],
    pub y_axis: [f64; 2],
    /// Cell width and height; zero when the `corner` gives them.
    pub cell: [f64; 2],
    pub rows: usize,
    pub columns: usize,
    pub corner: Option<[f64; 2]>,
    pub polylines: bool,
}

/// Create Fishnet. `spatial_reference_wkt` is the coordinate system of the output.
pub fn create_fishnet(f: &Fishnet, spatial_reference_wkt: Option<String>) -> Result<ToolOutput> {
    let (dx, dy) = (f.y_axis[0] - f.origin[0], f.y_axis[1] - f.origin[1]);
    let len = dx.hypot(dy);
    if len.is_nan() || len <= 0.0 || !len.is_finite() {
        return Err(err(
            "ERROR 000800: the y-axis point must differ from the origin",
        ));
    }
    let uy = [dx / len, dy / len];
    let ux = [uy[1], -uy[0]];
    if f.rows == 0 || f.columns == 0 {
        return Err(err("ERROR 000800: give the number of rows and columns"));
    }
    if f.rows.saturating_mul(f.columns) > 50_000_000 {
        return Err(err("ERROR 000800: too many cells"));
    }
    let (cw, ch) = match f.corner {
        Some(c) => {
            let (vx, vy) = (c[0] - f.origin[0], c[1] - f.origin[1]);
            (
                ((vx * ux[0] + vy * ux[1]) / f.columns as f64).abs(),
                ((vx * uy[0] + vy * uy[1]) / f.rows as f64).abs(),
            )
        }
        None => (f.cell[0], f.cell[1]),
    };
    if !(cw > 0.0 && ch > 0.0 && cw.is_finite() && ch.is_finite()) {
        return Err(err(
            "ERROR 000800: give the cell size or the opposite corner",
        ));
    }
    let at = |col: f64, row: f64| {
        [
            f.origin[0] + col * cw * ux[0] + row * ch * uy[0],
            f.origin[1] + col * cw * ux[1] + row * ch * uy[1],
        ]
    };
    let mut features = Vec::new();
    let mut push = |kind: GeometryKind, run: Vec<[f64; 2]>| {
        features.push(Feature {
            oid: features.len() as i64,
            geometry: Some(shape(kind, vec![run])),
            attributes: vec![Value::Integer(0)],
        });
    };
    if f.polylines {
        for row in 0..=f.rows {
            push(
                GeometryKind::Polyline,
                vec![at(0.0, row as f64), at(f.columns as f64, row as f64)],
            );
        }
        for col in 0..=f.columns {
            push(
                GeometryKind::Polyline,
                vec![at(col as f64, 0.0), at(col as f64, f.rows as f64)],
            );
        }
    } else {
        for row in 0..f.rows {
            for col in 0..f.columns {
                let (c, r) = (col as f64, row as f64);
                push(
                    GeometryKind::Polygon,
                    vec![
                        at(c, r),
                        at(c, r + 1.0),
                        at(c + 1.0, r + 1.0),
                        at(c + 1.0, r),
                        at(c, r),
                    ],
                );
            }
        }
    }
    Ok(ToolOutput {
        kind: if f.polylines {
            GeometryKind::Polyline
        } else {
            GeometryKind::Polygon
        },
        has_z: false,
        has_m: false,
        fields: vec![id_field()],
        spatial_reference_wkt,
        features,
        messages: Vec::new(),
    })
}

/// Polygon To Line with IGNORE_NEIGHBORS.
pub fn polygon_to_line(input: &dyn FeatureClass) -> Result<ToolOutput> {
    if input.geometry_kind() != Some(GeometryKind::Polygon) {
        return Err(err("ERROR 000368: the input must be polygons"));
    }
    let mut fields = input.fields().to_vec();
    fields.push(Field::new("ORIG_FID", FieldType::Integer));
    let mut features = Vec::new();
    for f in input.features() {
        let f = f?;
        let Some(g) = &f.geometry else { continue };
        if g.has_curves() {
            return Err(err(
                "ERROR 000860: Polygon To Line does not handle curves yet",
            ));
        }
        let runs = parts(g).into_iter().map(<[[f64; 2]]>::to_vec).collect();
        features.push(Feature {
            oid: features.len() as i64,
            geometry: Some(shape(GeometryKind::Polyline, runs)),
            attributes: f
                .attributes
                .iter()
                .cloned()
                .chain([Value::Integer(f.oid as i32)])
                .collect(),
        });
    }
    Ok(ToolOutput {
        kind: GeometryKind::Polyline,
        has_z: false,
        has_m: false,
        fields,
        spatial_reference_wkt: input.spatial_reference_wkt().map(str::to_owned),
        features,
        messages: Vec::new(),
    })
}

/// One axis of a coordinate as ArcGIS Convert Coordinate Notation writes it: degrees (two digits
/// for latitude, three for longitude), then minutes and seconds with eight decimals, and the
/// hemisphere letter last (`45 40 44.04360000N`).
fn notation_axis(value: f64, is_lat: bool, parts: u8) -> String {
    let width = if is_lat { 2 } else { 3 };
    let hemi = match (is_lat, value < 0.0) {
        (true, true) => 'S',
        (true, false) => 'N',
        (false, true) => 'W',
        (false, false) => 'E',
    };
    let a = value.abs();
    let mut deg = a.floor();
    let rem = a - deg;
    let body = match parts {
        1 => format!("{a:0w$.8}", w = width + 9),
        2 => {
            let mut min = format!("{:011.8}", rem * 60.0);
            if min.starts_with("60.") {
                deg += 1.0;
                min = format!("{:011.8}", 0.0);
            }
            format!("{deg:0width$.0} {min}")
        }
        _ => {
            let mut min = (rem * 60.0).floor();
            let mut sec = (rem - min / 60.0) * 3600.0;
            if sec < 0.0 {
                sec = 0.0;
            }
            let mut s = format!("{sec:011.8}");
            if s.starts_with("60.") {
                min += 1.0;
                s = format!("{:011.8}", 0.0);
            }
            if min >= 60.0 {
                min = 0.0;
                deg += 1.0;
            }
            format!("{deg:0width$.0} {min:02.0} {s}")
        }
    };
    format!("{body}{hemi}")
}

/// Convert Coordinate Notation from decimal-degree X/Y fields to `DD_1`, `DD_2`, `DDM_1`,
/// `DDM_2`, `DMS_1`, `DMS_2` or `DD_NUMERIC` (observed against ArcGIS Pro 3.7). Points are the
/// input coordinates; the input's fields come first, then the notation field(s).
pub fn convert_coordinate_notation(
    table: &dyn FeatureClass,
    x_field: &str,
    y_field: &str,
    output_format: &str,
    spatial_reference_wkt: Option<String>,
) -> Result<ToolOutput> {
    let pos = |n: &str| crate::field_positions(table, &[n.to_owned()]).map(|v| v[0]);
    let (xi, yi) = (pos(x_field)?, pos(y_field)?);
    let fmt = output_format.to_ascii_uppercase();
    let (prefix, parts, single) = match fmt.as_str() {
        "DD_1" => ("DD", 1, true),
        "DD_2" => ("DD", 1, false),
        "DDM_1" => ("DDM", 2, true),
        "DDM_2" => ("DDM", 2, false),
        "DMS_1" => ("DMS", 3, true),
        "DMS_2" => ("DMS", 3, false),
        "DD_NUMERIC" => ("DD", 0, false),
        other => return Err(err(format!("output format {other}"))),
    };
    let text = |len: u32, name: &str| {
        let mut f = Field::new(name, FieldType::String);
        f.length = len;
        f
    };
    let mut fields = table.fields().to_vec();
    if parts == 0 {
        fields.push(Field::new("DDLat", FieldType::Double));
        fields.push(Field::new("DDLon", FieldType::Double));
    } else if single {
        fields.push(text(255, &format!("{prefix}LatLon")));
    } else {
        fields.push(text(255, &format!("{prefix}Lat")));
        fields.push(text(255, &format!("{prefix}Lon")));
    }
    let mut features = Vec::new();
    for f in table.features() {
        let f = f?;
        let (x, y) = (
            crate::number(&f.attributes[xi]),
            crate::number(&f.attributes[yi]),
        );
        let mut attributes = f.attributes.clone();
        match (x, y) {
            (Some(x), Some(y)) if parts == 0 => {
                attributes.push(Value::Double(y));
                attributes.push(Value::Double(x));
            }
            (Some(x), Some(y)) => {
                let lat = notation_axis(y, true, parts);
                let lon = notation_axis(x, false, parts);
                if single {
                    attributes.push(Value::String(format!("{lat} {lon}")));
                } else {
                    attributes.push(Value::String(lat));
                    attributes.push(Value::String(lon));
                }
            }
            _ => {
                let n = if parts == 0 || !single { 2 } else { 1 };
                attributes.extend(std::iter::repeat_n(Value::Null, n));
            }
        }
        features.push(Feature {
            oid: features.len() as i64,
            geometry: x.zip(y).map(|(x, y)| Geometry::point(x, y)),
            attributes,
        });
    }
    Ok(ToolOutput {
        kind: GeometryKind::Point,
        has_z: false,
        has_m: false,
        fields,
        spatial_reference_wkt,
        features,
        messages: Vec::new(),
    })
}
