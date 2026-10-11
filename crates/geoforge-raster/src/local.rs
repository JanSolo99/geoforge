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

/// Plus, Minus, Times, Divide: float output if either operand is float, else 32-bit
/// integer (Divide then truncates); NoData where either input is NoData or on
/// division by zero.
pub fn math(op: MathOp, a: &Operand, b: &Operand) -> Option<Raster> {
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
            MathOp::Divide => (x / y).trunc(),
        })
    });
    Some(if float {
        float_raster(g, values)
    } else {
        int_raster(g, values)
    })
}

/// Int: truncates toward zero into a 32-bit integer raster.
pub fn int(r: &Raster) -> Raster {
    int_raster(
        r,
        r.bands[0]
            .iter()
            .map(|&v| (!r.is_nodata(v)).then(|| v.trunc())),
    )
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

/// The type of a result made from `ops`: float if any is float; from constants
/// only, the smallest integer layout; otherwise 32-bit integer.
fn conditional_raster(g: &Raster, ops: &[&Operand], values: Vec<Option<f64>>) -> Raster {
    if ops.iter().any(|o| o.is_float()) {
        float_raster(g, values.into_iter())
    } else if ops.iter().all(|o| o.raster().is_none()) {
        let (min, max) = values
            .iter()
            .flatten()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &v| {
                (a.min(v), b.max(v))
            });
        let (min, max) = if min.is_finite() {
            (min, max)
        } else {
            (0.0, 0.0)
        };
        let (t, nodata) = crate::zonal::integer_output(min, max, true);
        let mut out = g.like(t);
        out.nodata = Some(nodata);
        out.bands[0] = values
            .into_iter()
            .map(|v| v.map_or(nodata, |v| t.store(v)))
            .collect();
        out
    } else {
        int_raster(g, values.into_iter())
    }
}

/// Con: where the condition raster holds (non-zero, or matching `clause`) take
/// `if_true`, else `if_false` (NoData when absent); NoData where the condition is.
pub fn con(
    condition: &Raster,
    if_true: &Operand,
    if_false: Option<&Operand>,
    clause: Option<&ValueClause>,
) -> Raster {
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
                if_false?.get(i)
            }
        })
        .collect();
    let mut ops = vec![if_true];
    ops.extend(if_false);
    conditional_raster(condition, &ops, values)
}

/// SetNull: NoData where the condition holds (non-zero, or matching `clause`),
/// else `if_false`.
pub fn set_null(condition: &Raster, if_false: &Operand, clause: Option<&ValueClause>) -> Raster {
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
    conditional_raster(condition, &[if_false], values)
}
