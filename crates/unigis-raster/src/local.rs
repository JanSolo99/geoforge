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

    pub(crate) fn is_float(&self) -> bool {
        match self {
            Operand::Raster(r) => r.data_type.is_float(),
            Operand::Constant(c) => c.fract() != 0.0,
        }
    }

    pub(crate) fn get(&self, i: usize) -> Option<f64> {
        match self {
            Operand::Raster(r) => {
                let v = r.bands[0][i];
                (!r.is_nodata(v)).then_some(v)
            }
            Operand::Constant(c) => Some(*c),
        }
    }

    pub(crate) fn raster(&self) -> Option<&Raster> {
        match self {
            Operand::Raster(r) => Some(r),
            Operand::Constant(_) => None,
        }
    }
}

/// The first raster among the operands gives the output grid.
pub(crate) fn grid<'a>(ops: &[&'a Operand]) -> Option<&'a Raster> {
    ops.iter().find_map(|o| o.raster())
}

pub(crate) fn float_raster(g: &Raster, values: impl Iterator<Item = Option<f64>>) -> Raster {
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

pub(crate) fn typed_raster(
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
    pub(crate) fn known_range(&self) -> Option<(f64, f64)> {
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

/// Pick: each cell takes the value of the `position`-th (counting from 1) of `items`; a position
/// of 0, past the last item or NoData gives NoData. The grid is the [`local_grid`] of the position
/// raster and the item rasters; the type is the one Con gives, but fractional constants make
/// 64-bit float.
pub fn pick(position: &Raster, items: &[Operand]) -> Option<Raster> {
    let mut on_grid = vec![position];
    on_grid.extend(items.iter().filter_map(|o| o.raster()));
    let grid = local_grid(&on_grid)?;
    let position = position.onto(grid.0, grid.1, grid.2);
    let items: Vec<Operand> = items.iter().map(|o| o.onto(grid)).collect();
    let n = position.width * position.height;
    let values: Vec<Option<f64>> = (0..n)
        .map(|i| {
            let p = position.bands[0][i];
            if position.is_nodata(p) || p < 1.0 {
                return None;
            }
            items.get(p as usize - 1)?.get(i)
        })
        .collect();
    if items
        .iter()
        .any(|o| matches!(o, Operand::Constant(c) if c.fract() != 0.0))
    {
        let nodata = f64::MIN;
        let mut out = position.like(DataType::F64);
        out.nodata = Some(nodata);
        out.bands[0] = values.iter().map(|v| v.unwrap_or(nodata)).collect();
        return Some(out);
    }
    let refs: Vec<&Operand> = items.iter().collect();
    Some(conditional_raster(&position, &refs, values))
}

/// Single-raster math tools (Spatial Analyst Math toolset).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Abs,
    Negate,
    Square,
    Sqrt,
    Exp,
    Exp2,
    Exp10,
    Ln,
    Log2,
    Log10,
    Sin,
    Cos,
    Tan,
    ASin,
    ACos,
    ATan,
    SinH,
    CosH,
    TanH,
    ASinH,
    ACosH,
    ATanH,
    RoundDown,
    RoundUp,
    BooleanNot,
    BitwiseNot,
}

/// The unsigned type of the width of `t`, and its NoData (the maximum).
fn unsigned_of(t: DataType) -> (DataType, f64) {
    match t {
        DataType::U8 | DataType::I8 => (DataType::U8, 255.0),
        DataType::U16 | DataType::I16 => (DataType::U16, f64::from(u16::MAX)),
        _ => (DataType::U32, f64::from(u32::MAX)),
    }
}

/// A signed type that holds the result, and its NoData (the minimum).
fn signed_for(t: DataType) -> (DataType, f64) {
    match t {
        DataType::U8 | DataType::I8 | DataType::I16 => (DataType::I16, f64::from(i16::MIN)),
        _ => (DataType::I32, f64::from(i32::MIN)),
    }
}

/// Cached integer metadata follows ArcGIS range inference, including its
/// signed32 fallback with an out-of-range Double NoData marker.
fn cached_integer_unary(op: UnaryOp, r: &Raster, (lo, hi): (f64, f64)) -> Raster {
    let (out_lo, out_hi) = match op {
        UnaryOp::Negate => (-hi, -lo),
        UnaryOp::Abs => (0., lo.abs().max(hi.abs())),
        _ => (0., (lo * lo).max(hi * hi)),
    };
    let (small_type, small_nodata) = crate::zonal::integer_output(out_lo, out_hi, true);
    let (t, nodata) = if matches!(
        small_type,
        DataType::U8 | DataType::I8 | DataType::U16 | DataType::I16
    ) {
        (small_type, small_nodata)
    } else if out_lo >= -(i32::MAX as f64)
        && out_hi <= i32::MAX as f64
        && (out_lo < 0. || out_hi < i32::MAX as f64)
    {
        (
            DataType::I32,
            if out_lo > -(i32::MAX as f64) {
                -(i32::MAX as f64)
            } else if out_hi < i32::MAX as f64 {
                i32::MAX as f64
            } else {
                f64::MIN
            },
        )
    } else if out_lo >= 0. && out_hi < u32::MAX as f64 {
        (DataType::U32, u32::MAX as f64)
    } else {
        (DataType::I32, f64::MIN)
    };
    let (minimum, maximum) = type_range(t);
    let values = r.bands[0].iter().map(|&value| {
        if r.is_nodata(value) {
            return None;
        }
        let result = match op {
            UnaryOp::Abs => value.abs(),
            UnaryOp::Negate if r.data_type == DataType::U32 => {
                f64::from((value as u32).wrapping_neg() as i32)
            }
            UnaryOp::Negate => -value,
            _ => value * value,
        };
        (result >= minimum && result <= maximum).then_some(result)
    });
    typed_raster(r, values, t, nodata)
}

/// Applies a one-raster math tool. Abs/Negate/Square preserve uncached F64 input precision.
/// Other float results and the trigonometric, logarithmic and exponential functions give 32-bit float with NoData where the result is not a finite number
/// (as 32-bit float); integer Abs keeps the width but is unsigned, Negate gives a signed type and
/// Square a wider one; Round Down / Up give float; Boolean Not gives 8-bit with NoData 2
/// (observed).
pub fn unary(op: UnaryOp, r: &Raster) -> Raster {
    if r.bands.len() > 1
        && (r.data_type.is_float() || r.statistics.is_none())
        && matches!(op, UnaryOp::Abs | UnaryOp::Negate | UnaryOp::Square)
    {
        // Use the dataset's cache status for every band, as ArcGIS does for
        // F64 output planning. Uncached integer planning depends only on storage
        // type, so every band has the same output type and NoData marker.
        // Clone only the current band, not the dataset.
        let apply = |band: &[f64]| {
            let single = Raster {
                width: r.width,
                height: r.height,
                data_type: r.data_type,
                transform: r.transform,
                nodata: r.nodata,
                epsg: r.epsg,
                statistics: r.statistics,
                bands: vec![band.to_vec()],
            };
            unary(op, &single)
        };
        let mut output = apply(&r.bands[0]);
        for band in r.bands.iter().skip(1) {
            output.bands.extend(apply(band).bands);
        }
        return output;
    }
    let values = || r.bands[0].iter().map(|&v| (!r.is_nodata(v)).then_some(v));
    let float_fn = |f: fn(f64) -> f64| {
        float_raster(
            r,
            values().map(|v| {
                v.map(f)
                    .filter(|x| x.is_finite() && (*x as f32).is_finite())
            }),
        )
    };
    let integer = !r.data_type.is_float();
    if integer
        && matches!(op, UnaryOp::Abs | UnaryOp::Negate | UnaryOp::Square)
        && let Some(range) = r
            .statistics
            .filter(|(lo, hi)| lo.is_finite() && hi.is_finite() && lo <= hi)
    {
        return cached_integer_unary(op, r, range);
    }
    match op {
        UnaryOp::Abs | UnaryOp::Negate | UnaryOp::Square
            if r.data_type == DataType::F64 && r.statistics.is_none() =>
        {
            let operation: fn(f64) -> f64 = match op {
                UnaryOp::Abs => f64::abs,
                UnaryOp::Negate => |x| -x,
                _ => |x| x * x,
            };
            typed_raster(
                r,
                values().map(|v| v.map(operation).filter(|x| x.is_finite())),
                DataType::F64,
                f64::MIN,
            )
        }
        UnaryOp::Abs if integer => {
            let (t, nodata) = unsigned_of(r.data_type);
            typed_raster(r, values().map(|v| v.map(f64::abs)), t, nodata)
        }
        UnaryOp::Negate if integer && r.statistics.is_none() => {
            let (t, nodata) = match r.data_type {
                DataType::U8 | DataType::I8 => (DataType::I16, i16::MIN as f64),
                _ => (DataType::I32, i32::MIN as f64),
            };
            let wide = matches!(r.data_type, DataType::U32 | DataType::I32);
            typed_raster(
                r,
                values().map(|v| {
                    v.map(|x| {
                        if wide {
                            f64::from((x as i64 as i32).wrapping_neg())
                        } else {
                            -x
                        }
                    })
                }),
                t,
                nodata,
            )
        }
        UnaryOp::Negate if integer => {
            let (t, nodata) = signed_for(r.data_type);
            typed_raster(r, values().map(|v| v.map(|x| -x)), t, nodata)
        }
        UnaryOp::Square if integer && r.statistics.is_none() => {
            let (t, nodata) = match r.data_type {
                DataType::U8 => (DataType::U16, u16::MAX as f64),
                DataType::I8 => (DataType::I16, i16::MIN as f64),
                DataType::U16 | DataType::U32 => (DataType::U32, u32::MAX as f64),
                _ => (DataType::I32, i32::MIN as f64),
            };
            let (lo, hi) = type_range(t);
            typed_raster(
                r,
                values().map(|v| v.map(|x| x * x).filter(|x| *x >= lo && *x <= hi)),
                t,
                nodata,
            )
        }
        UnaryOp::Square if integer => {
            let (t, nodata) = match r.data_type {
                DataType::U8 | DataType::I8 => (DataType::U16, f64::from(u16::MAX)),
                DataType::U16 | DataType::I16 => (DataType::I32, -2147483647.0), // not i32::MIN (observed)
                _ => (DataType::F32, f32::MIN as f64),
            };
            typed_raster(r, values().map(|v| v.map(|x| x * x)), t, nodata)
        }
        UnaryOp::BooleanNot => typed_raster(
            r,
            values().map(|v| v.map(|x| f64::from(x == 0.0))),
            DataType::U8,
            2.0,
        ),
        UnaryOp::BitwiseNot => typed_raster(
            r,
            values().map(|v| v.map(|x| f64::from(!(x.trunc() as i64 as i32)))),
            DataType::I32,
            f64::from(i32::MIN),
        ),
        UnaryOp::Abs => float_fn(f64::abs),
        UnaryOp::Negate => float_fn(|x| -x),
        UnaryOp::Square => float_fn(|x| x * x),
        UnaryOp::Sqrt => float_fn(f64::sqrt),
        UnaryOp::Exp => float_fn(f64::exp),
        UnaryOp::Exp2 => float_fn(f64::exp2),
        UnaryOp::Exp10 => float_fn(|x| 10f64.powf(x)),
        UnaryOp::Ln => float_fn(f64::ln),
        UnaryOp::Log2 => float_fn(f64::log2),
        UnaryOp::Log10 => float_fn(f64::log10),
        UnaryOp::Sin => float_fn(f64::sin),
        UnaryOp::Cos => float_fn(f64::cos),
        UnaryOp::Tan => float_fn(f64::tan),
        UnaryOp::ASin => float_fn(f64::asin),
        UnaryOp::ACos => float_fn(f64::acos),
        UnaryOp::ATan => float_fn(f64::atan),
        UnaryOp::SinH => float_fn(f64::sinh),
        UnaryOp::CosH => float_fn(f64::cosh),
        UnaryOp::TanH => float_fn(f64::tanh),
        UnaryOp::ASinH => float_fn(f64::asinh),
        UnaryOp::ACosH => float_fn(f64::acosh),
        UnaryOp::ATanH => float_fn(f64::atanh),
        UnaryOp::RoundDown => float_fn(f64::floor),
        UnaryOp::RoundUp => float_fn(f64::ceil),
    }
}

/// Two-operand math tools (Spatial Analyst Math toolset, besides Plus, Minus, Times, Divide).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    ATan2,
    Power,
    Mod,
    BooleanAnd,
    BooleanOr,
    BooleanXor,
    EqualTo,
    GreaterThan,
    GreaterThanEqual,
    LessThan,
    LessThanEqual,
    NotEqual,
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    BitwiseLeftShift,
    BitwiseRightShift,
}

/// The type of an integer Mod: from the operands' value ranges when ArcGIS knows both (stored
/// statistics, constants), else the wider of their types (signed if either is) (observed).
fn mod_type(a: &Operand, b: &Operand) -> (DataType, f64) {
    if let (Some(ra), Some(rb)) = (a.known_range(), b.known_range()) {
        let reach = rb.0.abs().max(rb.1.abs()) - 1.0;
        let lo = if ra.0 < 0.0 { -(-ra.0).min(reach) } else { 0.0 };
        // A constant dividend does not narrow the range: the divisor alone sets it (observed).
        let hi = if a.raster().is_none() {
            reach
        } else if ra.1 > 0.0 {
            ra.1.min(reach)
        } else {
            0.0
        };
        return crate::zonal::integer_output(lo, hi, true);
    }
    let (ta, tb) = (a.integer_type(), b.integer_type());
    let width = |t: DataType| match t {
        DataType::U8 | DataType::I8 => 8,
        DataType::U16 | DataType::I16 => 16,
        _ => 32,
    };
    let signed = |t: DataType| matches!(t, DataType::I8 | DataType::I16 | DataType::I32);
    let w = width(ta).max(width(tb));
    // Which operand is the dividend decides the sign: a constant divisor does not make it
    // signed, a signed raster does.
    let signed_result = signed(ta) || (signed(tb) && b.raster().is_some());
    match (signed_result, w) {
        (false, 8) => (DataType::U8, 255.0),
        (false, 16) => (DataType::U16, f64::from(u16::MAX)),
        (false, _) => (DataType::U32, f64::from(u32::MAX)),
        (true, 8 | 16) => (DataType::I16, f64::from(i16::MIN)),
        (true, _) => (DataType::I32, f64::from(i32::MIN)),
    }
}

/// Applies a two-operand math tool: Power gives 32-bit float; comparisons and Boolean tools
/// give 8-bit 0/1 (nonzero is true) with NoData 255; Bitwise tools truncate to integers and give
/// 32-bit integers (shifts by the count modulo 32); Mod is the C remainder (sign of the
/// dividend) and is float only if an operand is. NoData where an operand is, on division by zero,
/// and where a float result is not finite.
pub fn binary(op: BinaryOp, a: &Operand, b: &Operand) -> Option<Raster> {
    let (a, b) = match (a, b) {
        (Operand::Raster(ra), Operand::Raster(rb)) => {
            let (ra, rb) = common_grid(ra, rb)?;
            (Operand::Raster(ra), Operand::Raster(rb))
        }
        _ => (a.clone(), b.clone()),
    };
    let (a, b) = (&a, &b);
    let g = grid(&[a, b])?;
    let n = g.width * g.height;
    let pair = |i: usize| Some((a.get(i)?, b.get(i)?));
    let truth = |v: bool| Some(f64::from(v));
    let int = |v: f64| v.trunc() as i64 as i32;
    let finite = |v: f64| Some(v).filter(|v| v.is_finite() && (*v as f32).is_finite());
    let bool_raster = |f: &dyn Fn(f64, f64) -> Option<f64>| {
        typed_raster(
            g,
            (0..n).map(|i| pair(i).and_then(|(x, y)| f(x, y))),
            DataType::U8,
            255.0,
        )
    };
    let bits = |f: &dyn Fn(i32, i32) -> i32| {
        typed_raster(
            g,
            (0..n).map(|i| pair(i).map(|(x, y)| f64::from(f(int(x), int(y))))),
            DataType::I32,
            f64::from(i32::MIN),
        )
    };
    Some(match op {
        BinaryOp::ATan2 => float_raster(
            g,
            (0..n).map(|i| {
                pair(i)
                    .filter(|&(y, x)| y != 0.0 || x != 0.0)
                    .map(|(y, x)| y.atan2(x))
            }),
        ),
        BinaryOp::Power => float_raster(
            g,
            (0..n).map(|i| pair(i).and_then(|(x, y)| finite(x.powf(y)))),
        ),
        BinaryOp::Mod => {
            let float = a.is_float() || b.is_float();
            let values = (0..n).map(|i| {
                let (x, y) = pair(i)?;
                (y != 0.0).then(|| x % y)
            });
            if float {
                float_raster(g, values)
            } else {
                let (t, nodata) = mod_type(a, b);
                typed_raster(g, values, t, nodata)
            }
        }
        BinaryOp::BooleanAnd => bool_raster(&|x, y| truth(x != 0.0 && y != 0.0)),
        BinaryOp::BooleanOr => bool_raster(&|x, y| truth(x != 0.0 || y != 0.0)),
        BinaryOp::BooleanXor => bool_raster(&|x, y| truth((x != 0.0) != (y != 0.0))),
        BinaryOp::EqualTo => bool_raster(&|x, y| truth(x == y)),
        BinaryOp::GreaterThan => bool_raster(&|x, y| truth(x > y)),
        BinaryOp::GreaterThanEqual => bool_raster(&|x, y| truth(x >= y)),
        BinaryOp::LessThan => bool_raster(&|x, y| truth(x < y)),
        BinaryOp::LessThanEqual => bool_raster(&|x, y| truth(x <= y)),
        BinaryOp::NotEqual => bool_raster(&|x, y| truth(x != y)),
        BinaryOp::BitwiseAnd => bits(&|x, y| x & y),
        BinaryOp::BitwiseOr => bits(&|x, y| x | y),
        BinaryOp::BitwiseXor => bits(&|x, y| x ^ y),
        BinaryOp::BitwiseLeftShift => bits(&|x, y| x.wrapping_shl(y as u32)),
        BinaryOp::BitwiseRightShift => bits(&|x, y| x.wrapping_shr(y as u32)),
    })
}

/// Combinatorial And / Or / XOr.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CombinatorialOp {
    And,
    Or,
    Xor,
}

/// Combinatorial And / Or / XOr: each distinct pair of input values gets a number, in the order
/// the pairs first occur (row by row). The pairs the operation calls false (And: either value 0;
/// Or: both 0; XOr: both or neither 0) get 0 and use no number. NoData where either input is.
/// The type is the smallest that holds the numbers (observed).
pub fn combinatorial(op: CombinatorialOp, a: &Raster, b: &Raster) -> Option<Raster> {
    let (a, b) = common_grid(a, b)?;
    let mut ids: std::collections::HashMap<(u64, u64), f64> = std::collections::HashMap::new();
    let mut next = 1.0;
    let values: Vec<Option<f64>> = a.bands[0]
        .iter()
        .zip(&b.bands[0])
        .map(|(&x, &y)| {
            if a.is_nodata(x) || b.is_nodata(y) {
                return None;
            }
            let numbered = match op {
                CombinatorialOp::And => x != 0.0 && y != 0.0,
                CombinatorialOp::Or => x != 0.0 || y != 0.0,
                CombinatorialOp::Xor => (x != 0.0) != (y != 0.0),
            };
            if !numbered {
                return Some(0.0);
            }
            Some(*ids.entry((x.to_bits(), y.to_bits())).or_insert_with(|| {
                next += 1.0;
                next - 1.0
            }))
        })
        .collect();
    let max = values.iter().flatten().fold(0.0_f64, |m, &v| m.max(v));
    let (t, nodata) = crate::zonal::integer_output(0.0, max, true);
    Some(typed_raster(&a, values.into_iter(), t, nodata))
}

/// The type ArcGIS gives a raster holding values of all of `types`: float if any is, else signed
/// if any is signed, else unsigned; the widest width (U16 with S16 gives S16, observed).
fn promote(rasters: &[Raster]) -> (DataType, f64) {
    let types: Vec<DataType> = rasters.iter().map(|r| r.data_type).collect();
    // A signed raster whose stored statistics are all non-negative counts as unsigned (observed).
    let signed_needed = rasters.iter().any(|r| {
        matches!(r.data_type, DataType::I8 | DataType::I16 | DataType::I32)
            && !r.statistics.is_some_and(|(lo, _)| lo >= 0.0)
    });
    let types: &[DataType] = &types;
    if types.iter().any(|t| t.is_float()) {
        return (DataType::F32, f32::MIN as f64);
    }
    let width = |t: &DataType| match t {
        DataType::U8 | DataType::I8 => 8,
        DataType::U16 | DataType::I16 => 16,
        _ => 32,
    };
    let signed = signed_needed;
    let w = types.iter().map(width).max().unwrap_or(8);
    match (signed, w) {
        (false, 8) => (DataType::U8, 255.0),
        (false, 16) => (DataType::U16, f64::from(u16::MAX)),
        (false, _) => (DataType::U32, f64::from(u32::MAX)),
        (true, 8) => (DataType::I8, -128.0),
        (true, 16) => (DataType::I16, f64::from(i16::MIN)),
        (true, _) => (DataType::I32, f64::from(i32::MIN)),
    }
}

/// Rank (`popularity` false): the `k`-th smallest of the rasters' values in each cell. Popularity:
/// the `k`-th most frequent value; NoData when it is tied with another or there are fewer
/// distinct values. NoData where any raster is NoData.
pub fn rank_or_popularity(k: usize, rasters: &[Raster], popularity: bool) -> Option<Raster> {
    let first = rasters.first()?;
    if k == 0 || k > rasters.len() {
        return None;
    }
    let (t, nodata) = promote(rasters);
    let mut out = first.like(t);
    out.nodata = Some(nodata);
    for i in 0..first.width * first.height {
        let values: Option<Vec<f64>> = rasters
            .iter()
            .map(|r| {
                let v = r.bands[0][i];
                (!r.is_nodata(v)).then_some(v)
            })
            .collect();
        let v = values.and_then(|mut vals| {
            if !popularity {
                vals.sort_by(f64::total_cmp);
                return Some(vals[k - 1]);
            }
            let mut counts: Vec<(f64, usize)> = Vec::new();
            for v in vals {
                match counts.iter_mut().find(|(x, _)| *x == v) {
                    Some(c) => c.1 += 1,
                    None => counts.push((v, 1)),
                }
            }
            // When every raster agrees, any rank gives that value (observed).
            if let [(value, _)] = counts[..] {
                return Some(value);
            }
            counts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.total_cmp(&b.0)));
            let (value, n) = *counts.get(k - 1)?;
            let tied = counts.iter().filter(|c| c.1 == n).count() > 1;
            (!tied).then_some(value)
        });
        out.bands[0][i] = v.map_or(nodata, |v| t.store(v));
    }
    Some(out)
}

/// In List: the cell's value where it equals a constant of `list` or the value of a raster of
/// `list` at that cell, else NoData; the first raster's type stays and NoData is the type's
/// default (observed: -32768 for a 16-bit signed raster that had -9999).
pub fn in_list(input: &Raster, list: &[Operand]) -> Raster {
    let nodata = if input.data_type.is_float() {
        f32::MIN as f64
    } else {
        match input.data_type {
            DataType::U8 | DataType::U16 | DataType::U32 => type_range(input.data_type).1,
            _ => type_range(input.data_type).0,
        }
    };
    let mut out = input.like(input.data_type);
    out.nodata = Some(nodata);
    for i in 0..input.width * input.height {
        let v = input.bands[0][i];
        let keep = !input.is_nodata(v)
            && list.iter().any(|item| match item {
                Operand::Constant(c) => *c == v,
                Operand::Raster(r) => {
                    i < r.bands[0].len() && !r.is_nodata(r.bands[0][i]) && r.bands[0][i] == v
                }
            });
        out.bands[0][i] = if keep { v } else { nodata };
    }
    out
}

/// Diff and Over (Image Analyst). Diff: the first value where it differs from the second, 0 where
/// they are equal. Over: the first value, or the second where the first is 0 (the background).
/// The type is the widest of the operands' (floating if any is, signed only if a signed raster's
/// statistics have negatives); NoData where either operand is.
pub fn diff_or_over(over: bool, a: &Operand, b: &Operand) -> Option<Raster> {
    let (a, b) = match (a, b) {
        (Operand::Raster(ra), Operand::Raster(rb)) => {
            let (ra, rb) = common_grid(ra, rb)?;
            (Operand::Raster(ra), Operand::Raster(rb))
        }
        _ => (a.clone(), b.clone()),
    };
    let g = grid(&[&a, &b])?;
    let n = g.width * g.height;
    // Diff only returns values of the first operand, so only its type counts.
    let float = a.is_float() || (over && b.is_float());
    let signed = |o: &Operand| match o {
        Operand::Raster(r) => {
            matches!(r.data_type, DataType::I8 | DataType::I16 | DataType::I32)
                && !r.statistics.is_some_and(|(lo, _)| lo >= 0.0)
        }
        Operand::Constant(c) => *c < 0.0,
    };
    let width = |o: &Operand| match o.integer_type() {
        DataType::U8 | DataType::I8 => 8,
        DataType::U16 | DataType::I16 => 16,
        _ => 32,
    };
    let values = (0..n).map(|i| {
        let (x, y) = (a.get(i)?, b.get(i)?);
        Some(if over {
            if x != 0.0 { x } else { y }
        } else if x != y {
            x
        } else {
            0.0
        })
    });
    Some(if float {
        float_raster(g, values)
    } else {
        let w = if over {
            width(&a).max(width(&b))
        } else {
            width(&a)
        };
        let (t, nodata) = match (signed(&a) || (over && signed(&b)), w) {
            (false, 8) => (DataType::U8, 255.0),
            (false, 16) => (DataType::U16, f64::from(u16::MAX)),
            (false, _) => (DataType::U32, f64::from(u32::MAX)),
            (true, 8) => (DataType::I8, -128.0),
            (true, 16) => (DataType::I16, f64::from(i16::MIN)),
            (true, _) => (DataType::I32, f64::from(i32::MIN)),
        };
        typed_raster(g, values, t, nodata)
    })
}

/// Compute Change Raster (single band, DIFFERENCE or RELATIVE_DIFFERENCE): `to - from`, or that
/// divided by the larger absolute value of the two (0 where both are 0). 32-bit float, NoData
/// 3.4e38; NoData where either raster is.
pub fn compute_change(from: &Raster, to: &Raster, relative: bool) -> Option<Raster> {
    let (f, t) = common_grid(from, to)?;
    let nodata = f64::from(3.4e38_f32);
    let mut out = f.like(DataType::F32);
    out.nodata = Some(nodata);
    out.bands[0] = f.bands[0]
        .iter()
        .zip(&t.bands[0])
        .map(|(&a, &b)| {
            if f.is_nodata(a) || t.is_nodata(b) {
                return nodata;
            }
            let d = b - a;
            let v = if relative {
                let m = a.abs().max(b.abs());
                if m == 0.0 { 0.0 } else { d / m }
            } else {
                d
            };
            DataType::F32.store(v)
        })
        .collect();
    Some(out)
}

/// Create Binary Mask (no flood fill, expansion or minimum region size): 1 where the cell is not
/// the background value (default 0), else 0; 8-bit with NoData 255 kept on NoData cells.
pub fn binary_mask(input: &Raster, background: f64) -> Raster {
    let mut out = input.like(DataType::U8);
    out.nodata = Some(255.0);
    out.bands[0] = input.bands[0]
        .iter()
        .map(|&v| {
            if input.is_nodata(v) {
                255.0
            } else {
                f64::from(v != background)
            }
        })
        .collect();
    out
}
