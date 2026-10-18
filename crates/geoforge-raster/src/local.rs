//! Spatial Analyst local (cell-by-cell) tools: math operators, Int, Float,
//! IsNull, Con and SetNull. Raster operands must share one grid.

use crate::{DataType, Raster};

/// A raster or a constant operand.
#[derive(Debug, Clone)]
pub enum Operand {
    Raster(Raster),
    Constant(f64),
}

impl Operand {
    /// Parses a constant ("5", "0.5"); anything else is not a constant.
    pub fn constant(text: &str) -> Option<Operand> {
        text.trim().parse().ok().map(Operand::Constant)
    }

    fn is_float(&self) -> bool {
        match self {
            Operand::Raster(r) => r.data_type.is_float(),
            Operand::Constant(c) => c.fract() != 0.0,
        }
    }

    fn get(&self, i: usize) -> Option<f64> {
        match self {
            Operand::Raster(r) => {
                let v = r.bands[0][i];
                (!r.is_nodata(v)).then_some(v)
            }
            Operand::Constant(c) => Some(*c),
        }
    }

    fn raster(&self) -> Option<&Raster> {
        match self {
            Operand::Raster(r) => Some(r),
            Operand::Constant(_) => None,
        }
    }
}

/// The first raster among the operands gives the output grid.
fn grid<'a>(ops: &[&'a Operand]) -> Option<&'a Raster> {
    ops.iter().find_map(|o| o.raster())
}

fn float_raster(g: &Raster, values: impl Iterator<Item = Option<f64>>) -> Raster {
    let nodata = f32::MIN as f64;
    let mut out = g.like(DataType::F32);
    out.nodata = Some(nodata);
    out.bands[0] = values
        .map(|v| {
            v.filter(|v| v.is_finite())
                .map_or(nodata, |v| DataType::F32.store(v))
        })
        .collect();
    out
}

fn int_raster(g: &Raster, values: impl Iterator<Item = Option<f64>>) -> Raster {
    let nodata = i32::MIN as f64;
    let mut out = g.like(DataType::I32);
    out.nodata = Some(nodata);
    out.bands[0] = values
        .map(|v| v.map_or(nodata, |v| DataType::I32.store(v)))
        .collect();
    out
}

/// Binary math operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathOp {
    Plus,
    Minus,
    Times,
    Divide,
}

/// Plus, Minus, Times, Divide: float output if either operand is float; integer
/// output takes ArcGIS's type for the operands (see [`math_type`]), Divide then
/// rounding down; NoData where either input is NoData or on division by zero.
/// Rasters on different grids are combined on [`common_grid`].
pub fn math(op: MathOp, a: &Operand, b: &Operand) -> Option<Raster> {
    let (a, b) = match (a, b) {
        (Operand::Raster(ra), Operand::Raster(rb)) => {
            let (ra, rb) = common_grid(ra, rb)?;
            (Operand::Raster(ra), Operand::Raster(rb))
        }
        _ => (a.clone(), b.clone()),
    };
    let (a, b) = (&a, &b);
    let g = grid(&[a, b])?;
    let float = a.is_float() || b.is_float();
    let n = g.width * g.height;
    let values = (0..n).map(|i| {
        let (x, y) = (a.get(i)?, b.get(i)?);
        Some(match op {
            MathOp::Plus => x + y,
            MathOp::Minus => x - y,
            MathOp::Times => x * y,
            MathOp::Divide if y == 0.0 => return None,
            MathOp::Divide if float => x / y,
            MathOp::Divide => (x / y).floor(),
        })
    });
    Some(if float {
        float_raster(g, values)
    } else {
        let (t, nodata) = math_type(op, a, b);
        typed_raster(g, values, t, nodata)
    })
}

fn typed_raster(
    g: &Raster,
    values: impl Iterator<Item = Option<f64>>,
    t: DataType,
    nodata: f64,
) -> Raster {
    let mut out = g.like(t);
    out.nodata = Some(nodata);
    out.bands[0] = values.map(|v| v.map_or(nodata, |v| t.store(v))).collect();
    out
}

/// The grid ArcGIS's local tools put their rasters on by default: the
/// intersection of their extents in cells of the largest size, anchored at its
/// lower-left corner with round(extent / cell) columns and rows. `None` when
/// the extents do not overlap; the shared grid when all rasters are on one.
pub fn local_grid(rasters: &[&Raster]) -> Option<([f64; 6], usize, usize)> {
    let first = rasters.first()?;
    if rasters
        .iter()
        .all(|r| (r.width, r.height, r.transform) == (first.width, first.height, first.transform))
    {
        return Some((first.transform, first.width, first.height));
    }
    let (mut x0, mut y0, mut x1, mut y1) = first.extent();
    let mut cell = first.cell_width();
    for r in &rasters[1..] {
        let (a, b, c, d) = r.extent();
        (x0, y0, x1, y1) = (x0.max(a), y0.max(b), x1.min(c), y1.min(d));
        cell = cell.max(r.cell_width());
    }
    if x0 >= x1 || y0 >= y1 {
        return None;
    }
    let cols = (((x1 - x0) / cell).round() as usize).max(1);
    let rows = (((y1 - y0) / cell).round() as usize).max(1);
    Some((
        [x0, cell, 0.0, y0 + rows as f64 * cell, 0.0, -cell],
        cols,
        rows,
    ))
}

/// Two rasters on their [`local_grid`], sampled at its cell centers.
pub fn common_grid(a: &Raster, b: &Raster) -> Option<(Raster, Raster)> {
    let (t, cols, rows) = local_grid(&[a, b])?;
    Some((a.onto(t, cols, rows), b.onto(t, cols, rows)))
}

impl Raster {
    /// This raster sampled onto another grid (a copy when it is the same grid).
    pub(crate) fn onto(&self, transform: [f64; 6], cols: usize, rows: usize) -> Raster {
        if (self.transform, self.width, self.height) == (transform, cols, rows) {
            self.clone()
        } else {
            crate::resample::onto_grid(self, transform, cols, rows)
        }
    }
}

impl Operand {
    fn onto(&self, grid: ([f64; 6], usize, usize)) -> Operand {
        match self {
            Operand::Raster(r) => Operand::Raster(r.onto(grid.0, grid.1, grid.2)),
            Operand::Constant(c) => Operand::Constant(*c),
        }
    }
}

/// The value range of an integer type.
pub(crate) fn type_range_of(t: DataType) -> (f64, f64) {
    type_range(t)
}

/// The value range of an integer type.
fn type_range(t: DataType) -> (f64, f64) {
    match t {
        DataType::U8 => (0.0, u8::MAX as f64),
        DataType::I8 => (i8::MIN as f64, i8::MAX as f64),
        DataType::U16 => (0.0, u16::MAX as f64),
        DataType::I16 => (i16::MIN as f64, i16::MAX as f64),
        DataType::U32 => (0.0, u32::MAX as f64),
        _ => (i32::MIN as f64, i32::MAX as f64),
    }
}

/// The smallest integer type holding `lo..=hi`, unsigned when `lo` ≥ 0, with
/// NoData at the end of its range.
fn smallest_type(lo: f64, hi: f64) -> (DataType, f64) {
    let order = if lo >= 0.0 {
        [DataType::U8, DataType::U16, DataType::U32, DataType::I32]
    } else {
        [DataType::I8, DataType::I16, DataType::I32, DataType::I32]
    };
    let t = order
        .into_iter()
        .find(|&t| {
            let (a, b) = type_range(t);
            a <= lo && hi <= b
        })
        .unwrap_or(DataType::I32);
    let nodata = match t {
        DataType::U8 | DataType::U16 | DataType::U32 => type_range(t).1,
        _ => type_range(t).0,
    };
    (t, nodata)
}

impl Operand {
    /// The values the operand can take as ArcGIS knows them: a constant's value
    /// or a raster's stored statistics.
    fn known_range(&self) -> Option<(f64, f64)> {
        match self {
            Operand::Raster(r) => r.statistics,
            Operand::Constant(c) => Some((*c, *c)),
        }
    }

    /// The values its type allows: a raster's pixel type, or the smallest type
    /// holding a constant.
    fn type_range(&self) -> (f64, f64) {
        match self {
            Operand::Raster(r) => type_range(r.data_type),
            Operand::Constant(c) => type_range(smallest_type(*c, *c).0),
        }
    }

    fn integer_type(&self) -> DataType {
        match self {
            Operand::Raster(r) => r.data_type,
            Operand::Constant(c) => smallest_type(*c, *c).0,
        }
    }
}

/// The range of `op` over operands in `a` and `b` (Divide rounding down, never by 0).
fn op_range(op: MathOp, a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    let mut bs = vec![b.0, b.1];
    if b.0 <= 0.0 && 0.0 <= b.1 {
        bs.retain(|&v| v != 0.0);
        if b.1 >= 1.0 {
            bs.push(1.0);
        }
        if b.0 <= -1.0 {
            bs.push(-1.0);
        }
    }
    let mut out = (f64::INFINITY, f64::NEG_INFINITY);
    for x in [a.0, a.1] {
        for &y in &bs {
            let v = match op {
                MathOp::Plus => x + y,
                MathOp::Minus => x - y,
                MathOp::Times => x * y,
                MathOp::Divide => (x / y).floor(),
            };
            out = (out.0.min(v), out.1.max(v));
        }
    }
    if out.0 > out.1 { (0.0, 0.0) } else { out }
}

/// ArcGIS's integer output type for `a op b` (observed). When every raster
/// operand has stored statistics, the type fits the range the statistics and
/// constants allow, like other Spatial Analyst output (1/2/4-bit layouts, then
/// 8-bit, ...). Otherwise it follows the operand types: Plus, Minus and Times
/// take the smallest type holding the result over the types' full ranges (U8 +
/// U8 → U16, U8 − U8 → S16, S16 + U8 → S32); Divide keeps the wider type, or
/// the next wider signed type when an operand is signed.
pub fn math_type(op: MathOp, a: &Operand, b: &Operand) -> (DataType, f64) {
    if let (Some(ra), Some(rb)) = (a.known_range(), b.known_range()) {
        let (lo, hi) = op_range(op, ra, rb);
        return crate::zonal::integer_output(lo, hi, true);
    }
    if op == MathOp::Divide {
        let (ta, tb) = (a.integer_type(), b.integer_type());
        let width = |t: DataType| match t {
            DataType::U8 | DataType::I8 => 8,
            DataType::U16 | DataType::I16 => 16,
            _ => 32,
        };
        let signed = |t: DataType| matches!(t, DataType::I8 | DataType::I16 | DataType::I32);
        let w = width(ta).max(width(tb));
        let t = match (signed(ta) || signed(tb), w) {
            (false, 8) => DataType::U8,
            (false, 16) => DataType::U16,
            (false, _) => DataType::U32,
            (true, 8) => DataType::I16,
            (true, _) => DataType::I32,
        };
        let (lo, hi) = type_range(t);
        return (t, if lo < 0.0 { lo } else { hi });
    }
    let (lo, hi) = op_range(op, a.type_range(), b.type_range());
    smallest_type(lo, hi)
}

/// Int: truncates toward zero. Float input gives a 32-bit integer raster;
/// integer input with stored statistics takes the type fitting them, else keeps
/// its type (observed).
pub fn int(r: &Raster) -> Raster {
    let values = r.bands[0]
        .iter()
        .map(|&v| (!r.is_nodata(v)).then(|| v.trunc()));
    if r.data_type.is_float() {
        return int_raster(r, values);
    }
    let (t, nodata) = match r.statistics {
        Some((lo, hi)) => crate::zonal::integer_output(lo, hi, true),
        None => smallest_type(type_range(r.data_type).0, type_range(r.data_type).1),
    };
    typed_raster(r, values, t, nodata)
}

/// Float: the values as 32-bit float.
pub fn float(r: &Raster) -> Raster {
    float_raster(
        r,
        r.bands[0].iter().map(|&v| (!r.is_nodata(v)).then_some(v)),
    )
}

/// IsNull: 1 where the input is NoData, else 0 (8-bit unsigned).
pub fn is_null(r: &Raster) -> Raster {
    let mut out = r.like(DataType::U8);
    out.nodata = Some(255.0);
    out.bands[0] = r.bands[0]
        .iter()
        .map(|&v| if r.is_nodata(v) { 1.0 } else { 0.0 })
        .collect();
    out
}

/// A where clause on cell values: comparisons of `Value` with numbers joined by
/// AND / OR (AND binds tighter).
#[derive(Debug, Clone)]
pub struct ValueClause {
    any_of: Vec<Vec<(String, f64)>>,
}

impl ValueClause {
    pub fn parse(text: &str) -> Option<ValueClause> {
        let upper = text.to_ascii_uppercase();
        let any_of = upper
            .split(" OR ")
            .map(|conj| {
                conj.split(" AND ")
                    .map(|term| {
                        let t = term.trim().trim_start_matches('(').trim_end_matches(')');
                        let rest = t.strip_prefix("VALUE")?.trim_start();
                        let op_len = rest
                            .chars()
                            .take_while(|c| matches!(c, '<' | '>' | '=' | '!'))
                            .count();
                        let (op, num) = rest.split_at(op_len);
                        if !matches!(op, "<" | "<=" | ">" | ">=" | "=" | "<>" | "!=") {
                            return None;
                        }
                        Some((op.to_string(), num.trim().parse().ok()?))
                    })
                    .collect::<Option<Vec<_>>>()
            })
            .collect::<Option<Vec<_>>>()?;
        Some(ValueClause { any_of })
    }

    pub fn matches(&self, v: f64) -> bool {
        self.any_of.iter().any(|all| {
            all.iter().all(|(op, n)| match op.as_str() {
                "<" => v < *n,
                "<=" => v <= *n,
                ">" => v > *n,
                ">=" => v >= *n,
                "=" => v == *n,
                _ => v != *n,
            })
        })
    }
}

/// The output of Con / Set Null from the values of `ops`: float if any is
/// float; else, as for the math tools, the type fitting the known ranges of the
/// operands (constants and rasters with stored statistics) when all are known,
/// or the smallest type holding all their types' values (observed).
fn conditional_raster(g: &Raster, ops: &[&Operand], values: Vec<Option<f64>>) -> Raster {
    if ops.iter().any(|o| o.is_float()) {
        return float_raster(g, values.into_iter());
    }
    let union = |ranges: &mut dyn Iterator<Item = (f64, f64)>| {
        ranges.fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), (lo, hi)| {
            (a.min(lo), b.max(hi))
        })
    };
    let known: Option<Vec<(f64, f64)>> = ops.iter().map(|o| o.known_range()).collect();
    let (t, nodata) = match known {
        Some(k) => {
            let (lo, hi) = union(&mut k.into_iter());
            crate::zonal::integer_output(lo, hi, true)
        }
        None => {
            let (lo, hi) = union(&mut ops.iter().map(|o| o.type_range()));
            smallest_type(lo, hi)
        }
    };
    typed_raster(g, values.into_iter(), t, nodata)
}

/// Con: where the condition raster holds (non-zero, or matching `clause`) take
/// `if_true`, else `if_false` (NoData when absent); NoData where the condition is.
/// The output is on the [`local_grid`] of the condition and true rasters; `None`
/// when they do not overlap.
pub fn con(
    condition: &Raster,
    if_true: &Operand,
    if_false: Option<&Operand>,
    clause: Option<&ValueClause>,
) -> Option<Raster> {
    // The false raster takes no part in the grid (observed).
    let mut on_grid = vec![condition];
    on_grid.extend(if_true.raster());
    let grid = local_grid(&on_grid)?;
    let condition = condition.onto(grid.0, grid.1, grid.2);
    let if_true = if_true.onto(grid);
    let if_false = if_false.map(|o| o.onto(grid));
    let n = condition.width * condition.height;
    let values = (0..n)
        .map(|i| {
            let c = condition.bands[0][i];
            if condition.is_nodata(c) {
                return None;
            }
            let hit = clause.map_or(c != 0.0, |w| w.matches(c));
            if hit {
                if_true.get(i)
            } else {
                if_false.as_ref()?.get(i)
            }
        })
        .collect();
    let mut ops = vec![&if_true];
    ops.extend(if_false.as_ref());
    Some(conditional_raster(&condition, &ops, values))
}

/// SetNull: NoData where the condition holds (non-zero, or matching `clause`),
/// else `if_false`, on the [`local_grid`] of both rasters.
pub fn set_null(
    condition: &Raster,
    if_false: &Operand,
    clause: Option<&ValueClause>,
) -> Option<Raster> {
    let mut on_grid = vec![condition];
    on_grid.extend(if_false.raster());
    let grid = local_grid(&on_grid)?;
    let condition = condition.onto(grid.0, grid.1, grid.2);
    let if_false = if_false.onto(grid);
    let n = condition.width * condition.height;
    let values = (0..n)
        .map(|i| {
            let c = condition.bands[0][i];
            if condition.is_nodata(c) {
                return None;
            }
            if clause.map_or(c != 0.0, |w| w.matches(c)) {
                None
            } else {
                if_false.get(i)
            }
        })
        .collect();
    Some(conditional_raster(&condition, &[&if_false], values))
}
