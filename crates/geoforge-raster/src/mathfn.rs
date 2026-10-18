//! Spatial Analyst math functions: Abs, Exp, Ln, Sin, ... (one raster), Power, Mod, ATan2, the
//! relational and Boolean operators, Diff, Over and the bitwise operators (two operands).
//!
//! Matched to ArcGIS Pro 3.7 (`testdata/arcgis-reference/mathfn.json`, runs on 6 x 8 rasters with
//! NoData, floats, integers and constants):
//! - Float functions give 32-bit float; a result that is not a number or does not fit a 32-bit
//!   float (`Ln` of 0, `ASin(2)`, `Exp(100)`) is NoData. RoundDown and RoundUp give floats too.
//! - Abs, Square and Negate keep integers: the type fits the range the input's statistics allow
//!   (as `local::math`), a result beyond 16 bits is a signed 32-bit integer with NoData -2147483647.
//! - BooleanNot is 1 where the input is 0 and 0 elsewhere, an unsigned byte with NoData 2.
//! - Power and ATan2 are floats; Mod is `fmod` (the dividend's sign; zero divisors are NoData):
//!   float with a float operand, else an integer fitting `-(|divisor| - 1) ..= |divisor| - 1`.
//! - Relational and Boolean operators give 0/1 as an unsigned byte with NoData 255; a number is true
//!   when it is not 0.
//! - Diff is the first value where the two differ, else 0; Over is the first value where it is not 0,
//!   else the second; integer results fit the union of both ranges (with 0 for Diff).
//! - Bitwise operators take integers and give 32-bit signed integers (NoData -2147483648); shifts are
//!   32-bit (the count counts modulo 32). ATan2 of 0 and 0 is NoData.
//! - NoData in any operand is NoData; rasters on different grids meet on the local grid.

use crate::local::{Operand, common_grid, float_raster, grid, typed_raster};
use crate::zonal::integer_output;
use crate::{DataType, Raster};

/// A function of one raster.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unary {
    Abs,
    Exp,
    Exp2,
    Exp10,
    Ln,
    Log2,
    Log10,
    SquareRoot,
    Square,
    Negate,
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

impl Unary {
    pub fn parse(s: &str) -> Option<Unary> {
        Some(match s.to_ascii_uppercase().as_str() {
            "ABS" => Unary::Abs,
            "EXP" => Unary::Exp,
            "EXP2" => Unary::Exp2,
            "EXP10" => Unary::Exp10,
            "LN" => Unary::Ln,
            "LOG2" => Unary::Log2,
            "LOG10" => Unary::Log10,
            "SQUAREROOT" | "SQRT" => Unary::SquareRoot,
            "SQUARE" => Unary::Square,
            "NEGATE" => Unary::Negate,
            "SIN" => Unary::Sin,
            "COS" => Unary::Cos,
            "TAN" => Unary::Tan,
            "ASIN" => Unary::ASin,
            "ACOS" => Unary::ACos,
            "ATAN" => Unary::ATan,
            "SINH" => Unary::SinH,
            "COSH" => Unary::CosH,
            "TANH" => Unary::TanH,
            "ASINH" => Unary::ASinH,
            "ACOSH" => Unary::ACosH,
            "ATANH" => Unary::ATanH,
            "ROUNDDOWN" => Unary::RoundDown,
            "ROUNDUP" => Unary::RoundUp,
            "BOOLEANNOT" => Unary::BooleanNot,
            "BITWISENOT" => Unary::BitwiseNot,
            _ => return None,
        })
    }

    fn apply(self, x: f64) -> f64 {
        match self {
            Unary::Abs => x.abs(),
            Unary::Exp => x.exp(),
            Unary::Exp2 => x.exp2(),
            Unary::Exp10 => 10f64.powf(x),
            Unary::Ln => x.ln(),
            Unary::Log2 => x.log2(),
            Unary::Log10 => x.log10(),
            Unary::SquareRoot => x.sqrt(),
            Unary::Square => x * x,
            Unary::Negate => -x,
            Unary::Sin => x.sin(),
            Unary::Cos => x.cos(),
            Unary::Tan => x.tan(),
            Unary::ASin => x.asin(),
            Unary::ACos => x.acos(),
            Unary::ATan => x.atan(),
            Unary::SinH => x.sinh(),
            Unary::CosH => x.cosh(),
            Unary::TanH => x.tanh(),
            Unary::ASinH => x.asinh(),
            Unary::ACosH => x.acosh(),
            Unary::ATanH => x.atanh(),
            Unary::RoundDown => x.floor(),
            Unary::RoundUp => x.ceil(),
            Unary::BooleanNot => f64::from(x == 0.0),
            Unary::BitwiseNot => !(x as i64) as f64,
        }
    }
}

/// The range an integer raster's values can take: its statistics, else its type's.
fn range_of(r: &Raster) -> (f64, f64) {
    r.statistics
        .unwrap_or_else(|| crate::local::type_range_of(r.data_type))
}

/// The integer type for results in `lo..=hi`; 32-bit results from wide input are signed.
fn fit(lo: f64, hi: f64) -> (DataType, f64) {
    let (t, nd) = integer_output(lo, hi, true);
    if matches!(t, DataType::U32 | DataType::I32) {
        (DataType::I32, -2_147_483_647.0)
    } else {
        (t, nd)
    }
}

fn float_values(r: &Raster, f: impl Fn(f64) -> f64) -> Raster {
    float_raster(
        r,
        r.bands[0].iter().map(|&v| {
            if r.is_nodata(v) {
                return None;
            }
            let y = f(v);
            // Not a number, infinite or beyond a 32-bit float: NoData.
            (y.is_finite() && y.abs() <= f32::MAX as f64).then_some(y)
        }),
    )
}

/// A function of one raster.
pub fn unary(op: Unary, r: &Raster) -> Raster {
    let float_in = r.data_type.is_float();
    let keeps_integers = matches!(op, Unary::Abs | Unary::Square | Unary::Negate) && !float_in;
    if op == Unary::BooleanNot {
        let mut out = r.like(DataType::U8);
        out.nodata = Some(2.0);
        out.bands[0] = r.bands[0]
            .iter()
            .map(|&v| if r.is_nodata(v) { 2.0 } else { op.apply(v) })
            .collect();
        return out;
    }
    if op == Unary::BitwiseNot {
        let mut out = r.like(DataType::I32);
        out.nodata = Some(i32::MIN as f64);
        out.bands[0] = r.bands[0]
            .iter()
            .map(|&v| {
                if r.is_nodata(v) {
                    i32::MIN as f64
                } else {
                    op.apply(v)
                }
            })
            .collect();
        return out;
    }
    if !keeps_integers {
        return float_values(r, |x| op.apply(x));
    }
    let (lo, hi) = range_of(r);
    let (a, b) = (op.apply(lo), op.apply(hi));
    let (min, max) = match op {
        Unary::Abs | Unary::Square => (
            if lo <= 0.0 && hi >= 0.0 {
                0.0
            } else {
                a.min(b)
            },
            a.max(b),
        ),
        _ => (a.min(b), a.max(b)),
    };
    let (t, nodata) = fit(min, max);
    typed_raster(
        r,
        r.bands[0]
            .iter()
            .map(|&v| (!r.is_nodata(v)).then(|| op.apply(v))),
        t,
        nodata,
    )
}

/// A function of two operands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Binary {
    Power,
    Mod,
    ATan2,
    EqualTo,
    NotEqual,
    GreaterThan,
    GreaterThanEqual,
    LessThan,
    LessThanEqual,
    BooleanAnd,
    BooleanOr,
    BooleanXor,
    Diff,
    Over,
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    BitwiseLeftShift,
    BitwiseRightShift,
}

impl Binary {
    pub fn parse(s: &str) -> Option<Binary> {
        Some(match s.to_ascii_uppercase().as_str() {
            "POWER" => Binary::Power,
            "MOD" => Binary::Mod,
            "ATAN2" => Binary::ATan2,
            "EQUALTO" => Binary::EqualTo,
            "NOTEQUAL" => Binary::NotEqual,
            "GREATERTHAN" => Binary::GreaterThan,
            "GREATERTHANEQUAL" => Binary::GreaterThanEqual,
            "LESSTHAN" => Binary::LessThan,
            "LESSTHANEQUAL" => Binary::LessThanEqual,
            "BOOLEANAND" => Binary::BooleanAnd,
            "BOOLEANOR" => Binary::BooleanOr,
            "BOOLEANXOR" => Binary::BooleanXor,
            "DIFF" => Binary::Diff,
            "OVER" => Binary::Over,
            "BITWISEAND" => Binary::BitwiseAnd,
            "BITWISEOR" => Binary::BitwiseOr,
            "BITWISEXOR" => Binary::BitwiseXor,
            "BITWISELEFTSHIFT" => Binary::BitwiseLeftShift,
            "BITWISERIGHTSHIFT" => Binary::BitwiseRightShift,
            _ => return None,
        })
    }

    fn is_bitwise(self) -> bool {
        matches!(
            self,
            Binary::BitwiseAnd
                | Binary::BitwiseOr
                | Binary::BitwiseXor
                | Binary::BitwiseLeftShift
                | Binary::BitwiseRightShift
        )
    }

    fn is_logical(self) -> bool {
        matches!(
            self,
            Binary::EqualTo
                | Binary::NotEqual
                | Binary::GreaterThan
                | Binary::GreaterThanEqual
                | Binary::LessThan
                | Binary::LessThanEqual
                | Binary::BooleanAnd
                | Binary::BooleanOr
                | Binary::BooleanXor
        )
    }

    /// The value for `x` and `y`; `None` is NoData.
    fn apply(self, x: f64, y: f64) -> Option<f64> {
        Some(match self {
            Binary::Power => x.powf(y),
            Binary::Mod => {
                if y == 0.0 {
                    return None;
                }
                x % y
            }
            Binary::ATan2 => {
                if x == 0.0 && y == 0.0 {
                    return None;
                }
                x.atan2(y)
            }
            Binary::EqualTo => f64::from(x == y),
            Binary::NotEqual => f64::from(x != y),
            Binary::GreaterThan => f64::from(x > y),
            Binary::GreaterThanEqual => f64::from(x >= y),
            Binary::LessThan => f64::from(x < y),
            Binary::LessThanEqual => f64::from(x <= y),
            Binary::BooleanAnd => f64::from(x != 0.0 && y != 0.0),
            Binary::BooleanOr => f64::from(x != 0.0 || y != 0.0),
            Binary::BooleanXor => f64::from((x != 0.0) != (y != 0.0)),
            Binary::Diff => {
                if x != y {
                    x
                } else {
                    0.0
                }
            }
            Binary::Over => {
                if x != 0.0 {
                    x
                } else {
                    y
                }
            }
            Binary::BitwiseAnd => ((x as i64) & (y as i64)) as f64,
            Binary::BitwiseOr => ((x as i64) | (y as i64)) as f64,
            Binary::BitwiseXor => ((x as i64) ^ (y as i64)) as f64,
            // 32-bit shifts: the count counts modulo 32 and the result wraps (observed).
            Binary::BitwiseLeftShift => f64::from((x as i32).wrapping_shl(y as i64 as u32)),
            Binary::BitwiseRightShift => f64::from((x as i32).wrapping_shr(y as i64 as u32)),
        })
    }
}

fn operand_range(o: &Operand) -> (f64, f64) {
    match o {
        Operand::Raster(r) => range_of(r),
        Operand::Constant(c) => (*c, *c),
    }
}

/// A function of two operands (rasters or constants); `None` when the rasters do not overlap or a
/// bitwise operator meets a float.
pub fn binary(op: Binary, a: &Operand, b: &Operand) -> Option<Raster> {
    let (a, b) = match (a, b) {
        (Operand::Raster(ra), Operand::Raster(rb)) => {
            let (ra, rb) = common_grid(ra, rb)?;
            (Operand::Raster(ra), Operand::Raster(rb))
        }
        _ => (a.clone(), b.clone()),
    };
    let g = grid(&[&a, &b])?;
    let float = a.is_float() || b.is_float();
    if op.is_bitwise() && float {
        return None;
    }
    let n = g.width * g.height;
    let values: Vec<Option<f64>> = (0..n).map(|i| op.apply(a.get(i)?, b.get(i)?)).collect();
    if op.is_logical() {
        let mut out = g.like(DataType::U8);
        out.nodata = Some(255.0);
        out.bands[0] = values.iter().map(|v| v.unwrap_or(255.0)).collect();
        return Some(out);
    }
    if op.is_bitwise() {
        let mut out = g.like(DataType::I32);
        out.nodata = Some(i32::MIN as f64);
        out.bands[0] = values
            .iter()
            .map(|v| v.unwrap_or(i32::MIN as f64))
            .collect();
        return Some(out);
    }
    let floats = matches!(op, Binary::Power | Binary::ATan2) || float;
    if floats {
        let nd = f32::MAX as f64;
        return Some(float_raster(
            g,
            values
                .into_iter()
                .map(|v| v.filter(|y| y.is_finite() && y.abs() <= nd)),
        ));
    }
    let (ra, rb) = (operand_range(&a), operand_range(&b));
    let (lo, hi) = match op {
        Binary::Mod => {
            let m = rb.0.abs().max(rb.1.abs()) - 1.0;
            (-m.max(0.0), m.max(0.0))
        }
        Binary::Diff => (ra.0.min(0.0), ra.1.max(0.0)),
        _ => (ra.0.min(rb.0), ra.1.max(rb.1)),
    };
    let (t, nodata) = fit(lo, hi);
    Some(typed_raster(g, values.into_iter(), t, nodata))
}
