//! Spatial Analyst Raster Calculator: one map algebra expression in Python syntax.
//!
//! Rasters are quoted paths (`"dem.tif"`, `r"dem.tif"`, `Raster("dem.tif")`). Operators follow
//! Python precedence and map to the local tools as arcpy's Raster operators do: `+ - * /` Plus,
//! Minus, Times, Divide; `%` Mod; `**` Power; comparisons; `& | ^ ~` the Boolean tools; `<< >>`
//! the bitwise shifts; unary `-` Negate. Functions are the local tools by their arcpy names
//! (Con, SetNull, IsNull, Abs, SquareRoot, Int, Float, Power, …). Every step runs the matching tool,
//! so output types are those tools' types.

use unigis_core::{Error, Result};

use crate::Raster;
use crate::local::{self, BinaryOp, MathOp, Operand, UnaryOp};

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64, bool),
    Text(String),
    Name(String),
    Op(&'static str),
}

const OPS: [&str; 22] = [
    "**", "//", "<<", ">>", "<=", ">=", "==", "!=", "+", "-", "*", "/", "%", "<", ">", "&", "|",
    "^", "~", "(", ")", ",",
];

fn tokens(text: &str) -> Result<Vec<Token>> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c == '"' || c == '\'' {
            let end = chars[i + 1..]
                .iter()
                .position(|&d| d == c)
                .ok_or_else(|| bad("unterminated string"))?;
            out.push(Token::Text(chars[i + 1..i + 1 + end].iter().collect()));
            i += end + 2;
        } else if c.is_ascii_digit()
            || (c == '.' && chars.get(i + 1).is_some_and(char::is_ascii_digit))
        {
            let start = i;
            while i < chars.len()
                && (chars[i].is_ascii_digit()
                    || chars[i] == '.'
                    || matches!(chars[i], 'e' | 'E')
                    || (matches!(chars[i], '+' | '-') && matches!(chars[i - 1], 'e' | 'E')))
            {
                i += 1;
            }
            let s: String = chars[start..i].iter().collect();
            let v = s.parse().map_err(|_| bad(&format!("bad number {s}")))?;
            out.push(Token::Number(v, s.contains(['.', 'e', 'E'])));
        } else if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let name: String = chars[start..i].iter().collect();
            // A raw-string prefix: r"path".
            if name.eq_ignore_ascii_case("r") && matches!(chars.get(i), Some('"' | '\'')) {
                continue;
            }
            out.push(Token::Name(name));
        } else {
            let rest: String = chars[i..chars.len().min(i + 2)].iter().collect();
            let op = OPS
                .iter()
                .find(|o| rest.starts_with(**o))
                .ok_or_else(|| bad(&format!("unexpected character {c}")))?;
            out.push(Token::Op(op));
            i += op.len();
        }
    }
    Ok(out)
}

fn bad(msg: &str) -> Error {
    Error::Unsupported(format!("ERROR 000539: invalid expression: {msg}"))
}

/// A value during evaluation: a raster, or a constant (integer or float as written).
#[derive(Clone)]
enum Value {
    Raster(Raster),
    Constant(f64, bool),
}

impl Value {
    fn operand(&self) -> Operand {
        match self {
            Value::Raster(r) => Operand::Raster(r.clone()),
            Value::Constant(v, _) => Operand::Constant(*v),
        }
    }

    fn is_float(&self) -> bool {
        match self {
            Value::Raster(r) => r.data_type.is_float(),
            Value::Constant(_, f) => *f,
        }
    }

    fn into_float(self) -> Value {
        match self {
            Value::Raster(r) => Value::Raster(local::float(&r)),
            c => c,
        }
    }

    fn raster(self, what: &str) -> Result<Raster> {
        match self {
            Value::Raster(r) => Ok(r),
            Value::Constant(..) => Err(bad(&format!("{what} needs a raster"))),
        }
    }
}

struct Parser<'a> {
    tokens: Vec<Token>,
    at: usize,
    load: &'a dyn Fn(&str) -> Result<Raster>,
}

fn no_overlap() -> Error {
    Error::Unsupported("the rasters do not overlap".into())
}

fn binary(op: &str, a: Value, b: Value) -> Result<Value> {
    if let (Value::Constant(x, fx), Value::Constant(y, fy)) = (&a, &b) {
        let float = *fx || *fy;
        let v = match op {
            "+" => x + y,
            "-" => x - y,
            "*" => x * y,
            "/" | "divide" => x / y,
            "//" => (x / y).floor(),
            "%" => x.rem_euclid(*y),
            "**" => x.powf(*y),
            _ => return Err(bad(&format!("operator {op} on two constants"))),
        };
        return Ok(Value::Constant(v, float || op == "/" || v.fract() != 0.0));
    }
    // `/` is Python 3 true division: integer operands divide as float (the Divide function
    // keeps the tool's integer division).
    let (a, b) = if op == "/" && !a.is_float() && !b.is_float() {
        (a.into_float(), b.into_float())
    } else {
        (a, b)
    };
    let (oa, ob) = (a.operand(), b.operand());
    let math = |m| local::math(m, &oa, &ob);
    let bin = |o| local::binary(o, &oa, &ob);
    let out = match op {
        "+" => math(MathOp::Plus),
        "-" => math(MathOp::Minus),
        "*" => math(MathOp::Times),
        "/" | "divide" => math(MathOp::Divide),
        "%" => bin(BinaryOp::Mod),
        "**" => bin(BinaryOp::Power),
        "==" => bin(BinaryOp::EqualTo),
        "!=" => bin(BinaryOp::NotEqual),
        "<" => bin(BinaryOp::LessThan),
        "<=" => bin(BinaryOp::LessThanEqual),
        ">" => bin(BinaryOp::GreaterThan),
        ">=" => bin(BinaryOp::GreaterThanEqual),
        "&" => bin(BinaryOp::BooleanAnd),
        "|" => bin(BinaryOp::BooleanOr),
        "^" => bin(BinaryOp::BooleanXor),
        "<<" => bin(BinaryOp::BitwiseLeftShift),
        ">>" => bin(BinaryOp::BitwiseRightShift),
        _ => return Err(bad(&format!("operator {op} is not supported yet"))),
    };
    out.map(Value::Raster).ok_or_else(no_overlap)
}

fn unary_fn(name: &str) -> Option<UnaryOp> {
    Some(match name.to_ascii_lowercase().as_str() {
        "abs" => UnaryOp::Abs,
        "negate" => UnaryOp::Negate,
        "square" => UnaryOp::Square,
        "squareroot" => UnaryOp::Sqrt,
        "exp" => UnaryOp::Exp,
        "exp2" => UnaryOp::Exp2,
        "exp10" => UnaryOp::Exp10,
        "ln" => UnaryOp::Ln,
        "log2" => UnaryOp::Log2,
        "log10" => UnaryOp::Log10,
        "sin" => UnaryOp::Sin,
        "cos" => UnaryOp::Cos,
        "tan" => UnaryOp::Tan,
        "asin" => UnaryOp::ASin,
        "acos" => UnaryOp::ACos,
        "atan" => UnaryOp::ATan,
        "sinh" => UnaryOp::SinH,
        "cosh" => UnaryOp::CosH,
        "tanh" => UnaryOp::TanH,
        "asinh" => UnaryOp::ASinH,
        "acosh" => UnaryOp::ACosH,
        "atanh" => UnaryOp::ATanH,
        "rounddown" => UnaryOp::RoundDown,
        "roundup" => UnaryOp::RoundUp,
        "booleannot" => UnaryOp::BooleanNot,
        "bitwisenot" => UnaryOp::BitwiseNot,
        _ => return None,
    })
}

fn binary_fn(name: &str) -> Option<&'static str> {
    Some(match name.to_ascii_lowercase().as_str() {
        "plus" => "+",
        "minus" => "-",
        "times" => "*",
        "divide" => "divide",
        "mod" => "%",
        "power" => "**",
        "equalto" => "==",
        "notequal" => "!=",
        "lessthan" => "<",
        "lessthanequal" => "<=",
        "greaterthan" => ">",
        "greaterthanequal" => ">=",
        "booleanand" => "&",
        "booleanor" => "|",
        "booleanxor" => "^",
        "bitwiseleftshift" => "<<",
        "bitwiserightshift" => ">>",
        _ => return None,
    })
}

impl Parser<'_> {
    fn peek_op(&self) -> Option<&'static str> {
        match self.tokens.get(self.at) {
            Some(Token::Op(o)) => Some(o),
            _ => None,
        }
    }

    fn expect(&mut self, op: &str) -> Result<()> {
        if self.peek_op() == Some(op) {
            self.at += 1;
            Ok(())
        } else {
            Err(bad(&format!("expected {op}")))
        }
    }

    /// Python binary precedence levels, loosest first.
    fn level(&mut self, level: usize) -> Result<Value> {
        const LEVELS: [&[&str]; 7] = [
            &["<", "<=", ">", ">=", "==", "!="],
            &["|"],
            &["^"],
            &["&"],
            &["<<", ">>"],
            &["+", "-"],
            &["*", "/", "//", "%"],
        ];
        if level == LEVELS.len() {
            return self.unary();
        }
        let mut left = self.level(level + 1)?;
        let mut compared = false;
        while let Some(op) = self.peek_op().filter(|o| LEVELS[level].contains(o)) {
            if level == 0 && compared {
                return Err(bad(
                    "chained comparisons are not supported; use parentheses",
                ));
            }
            compared = true;
            self.at += 1;
            let right = self.level(level + 1)?;
            left = binary(op, left, right)?;
        }
        Ok(left)
    }

    fn unary(&mut self) -> Result<Value> {
        match self.peek_op() {
            Some("-") => {
                self.at += 1;
                match self.unary()? {
                    Value::Constant(v, f) => Ok(Value::Constant(-v, f)),
                    Value::Raster(r) => Ok(Value::Raster(local::unary(UnaryOp::Negate, &r))),
                }
            }
            Some("+") => {
                self.at += 1;
                self.unary()
            }
            Some("~") => {
                self.at += 1;
                let r = self.unary()?.raster("~")?;
                Ok(Value::Raster(local::unary(UnaryOp::BooleanNot, &r)))
            }
            _ => self.power(),
        }
    }

    fn power(&mut self) -> Result<Value> {
        let base = self.atom()?;
        if self.peek_op() == Some("**") {
            self.at += 1;
            let exponent = self.unary()?;
            return binary("**", base, exponent);
        }
        Ok(base)
    }

    fn arguments(&mut self) -> Result<Vec<Value>> {
        self.expect("(")?;
        let mut args = Vec::new();
        if self.peek_op() != Some(")") {
            loop {
                args.push(self.level(0)?);
                if self.peek_op() == Some(",") {
                    self.at += 1;
                } else {
                    break;
                }
            }
        }
        self.expect(")")?;
        Ok(args)
    }

    fn atom(&mut self) -> Result<Value> {
        let token = self
            .tokens
            .get(self.at)
            .cloned()
            .ok_or_else(|| bad("unexpected end"))?;
        self.at += 1;
        match token {
            Token::Number(v, f) => Ok(Value::Constant(v, f)),
            Token::Text(path) => Ok(Value::Raster((self.load)(&path)?)),
            Token::Op("(") => {
                let v = self.level(0)?;
                self.expect(")")?;
                Ok(v)
            }
            Token::Name(name) => {
                let args = self.arguments()?;
                call(&name, args)
            }
            Token::Op(o) => Err(bad(&format!("unexpected {o}"))),
        }
    }
}

fn call(name: &str, mut args: Vec<Value>) -> Result<Value> {
    let arity = |n: usize| -> Result<()> {
        if args.len() == n {
            Ok(())
        } else {
            Err(bad(&format!("{name} takes {n} arguments")))
        }
    };
    let lower = name.to_ascii_lowercase();
    if let Some(op) = unary_fn(name) {
        arity(1)?;
        let r = args.remove(0).raster(name)?;
        return Ok(Value::Raster(local::unary(op, &r)));
    }
    if let Some(op) = binary_fn(name) {
        arity(2)?;
        let b = args.remove(1);
        return binary(op, args.remove(0), b);
    }
    match lower.as_str() {
        "raster" => match args.as_slice() {
            [Value::Raster(_)] => Ok(args.remove(0)),
            _ => Err(bad("Raster takes a path")),
        },
        "int" => {
            arity(1)?;
            Ok(Value::Raster(local::int(&args.remove(0).raster(name)?)))
        }
        "float" => {
            arity(1)?;
            Ok(Value::Raster(local::float(&args.remove(0).raster(name)?)))
        }
        "isnull" => {
            arity(1)?;
            Ok(Value::Raster(local::is_null(&args.remove(0).raster(name)?)))
        }
        "atan2" => {
            arity(2)?;
            let (a, b) = (args[0].operand(), args[1].operand());
            local::binary(BinaryOp::ATan2, &a, &b)
                .map(Value::Raster)
                .ok_or_else(no_overlap)
        }
        "con" => {
            if !(2..=3).contains(&args.len()) {
                return Err(bad(
                    "Con takes 2 or 3 arguments (where clauses are not supported yet)",
                ));
            }
            let condition = args[0].clone().raster("Con's condition")?;
            let if_false = args.get(2).map(Value::operand);
            local::con(&condition, &args[1].operand(), if_false.as_ref(), None)
                .map(Value::Raster)
                .ok_or_else(no_overlap)
        }
        "setnull" => {
            if args.len() != 2 {
                return Err(bad(
                    "SetNull takes 2 arguments (where clauses are not supported yet)",
                ));
            }
            let condition = args[0].clone().raster("SetNull's condition")?;
            local::set_null(&condition, &args[1].operand(), None)
                .map(Value::Raster)
                .ok_or_else(no_overlap)
        }
        _ => Err(bad(&format!("function {name} is not supported yet"))),
    }
}

/// Evaluates `expression`, reading each quoted raster with `load`.
pub fn raster_calculator(
    expression: &str,
    load: &dyn Fn(&str) -> Result<Raster>,
) -> Result<Raster> {
    let mut parser = Parser {
        tokens: tokens(expression)?,
        at: 0,
        load,
    };
    let value = parser.level(0)?;
    if parser.at != parser.tokens.len() {
        return Err(bad("unexpected text after the expression"));
    }
    match value {
        Value::Raster(r) => Ok(r),
        Value::Constant(..) => Err(bad("the expression has no raster")),
    }
}
