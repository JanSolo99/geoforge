//! `where_clause` expressions as ArcGIS evaluates them on shapefiles (the dBASE
//! SQL subset): comparisons, `AND`/`OR`/`NOT`, `IN`, `LIKE` (`%`, `_`),
//! `IS [NOT] NULL`, parentheses, arithmetic, bare or double-quoted field names,
//! single-quoted strings. `BETWEEN` is not part of it (ArcGIS: ERROR 000358).

use unigis_core::{Error, Field, Result, Value};

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Ident(String),
    Number(f64),
    Str(String),
    Op(String),
    LParen,
    RParen,
    Comma,
}

fn tokenize(s: &str) -> Option<Vec<Token>> {
    let c: Vec<char> = s.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < c.len() {
        let ch = c[i];
        match ch {
            ' ' | '\t' | '\n' | '\r' => i += 1,
            '(' => {
                out.push(Token::LParen);
                i += 1;
            }
            ')' => {
                out.push(Token::RParen);
                i += 1;
            }
            ',' => {
                out.push(Token::Comma);
                i += 1;
            }
            '\'' => {
                let mut v = String::new();
                i += 1;
                loop {
                    match c.get(i) {
                        None => return None,
                        Some('\'') if c.get(i + 1) == Some(&'\'') => {
                            v.push('\'');
                            i += 2;
                        }
                        Some('\'') => {
                            i += 1;
                            break;
                        }
                        Some(&x) => {
                            v.push(x);
                            i += 1;
                        }
                    }
                }
                out.push(Token::Str(v));
            }
            '"' => {
                let end = c[i + 1..].iter().position(|&x| x == '"')? + i + 1;
                out.push(Token::Ident(c[i + 1..end].iter().collect()));
                i = end + 1;
            }
            '<' | '>' | '=' | '!' => {
                let two: String = c[i..(i + 2).min(c.len())].iter().collect();
                if ["<=", ">=", "<>", "!="].contains(&two.as_str()) {
                    out.push(Token::Op(two));
                    i += 2;
                } else if ch == '!' {
                    return None;
                } else {
                    out.push(Token::Op(ch.to_string()));
                    i += 1;
                }
            }
            '+' | '-' | '*' | '/' => {
                out.push(Token::Op(ch.to_string()));
                i += 1;
            }
            _ if ch.is_ascii_digit() || ch == '.' => {
                let start = i;
                while i < c.len()
                    && (c[i].is_ascii_digit() || c[i] == '.' || c[i] == 'e' || c[i] == 'E')
                {
                    i += 1;
                }
                out.push(Token::Number(
                    c[start..i].iter().collect::<String>().parse().ok()?,
                ));
            }
            _ if ch.is_alphabetic() || ch == '_' => {
                let start = i;
                while i < c.len() && (c[i].is_alphanumeric() || c[i] == '_') {
                    i += 1;
                }
                out.push(Token::Ident(c[start..i].iter().collect()));
            }
            _ => return None,
        }
    }
    Some(out)
}

#[derive(Debug, Clone)]
enum Operand {
    Field(usize),
    Value(Value),
    Arith(Box<Operand>, char, Box<Operand>),
    Neg(Box<Operand>),
}

#[derive(Debug, Clone)]
enum Expr {
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    Compare(Operand, String, Operand),
    In(Operand, Vec<Operand>, bool),
    Like(Operand, String, bool),
    IsNull(Operand, bool),
}

struct Parser<'a> {
    t: Vec<Token>,
    i: usize,
    fields: &'a [Field],
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Token> {
        self.t.get(self.i)
    }
    fn keyword(&self, k: &str) -> bool {
        matches!(self.peek(), Some(Token::Ident(w)) if w.eq_ignore_ascii_case(k))
    }
    fn eat_keyword(&mut self, k: &str) -> bool {
        let yes = self.keyword(k);
        if yes {
            self.i += 1;
        }
        yes
    }
    fn next(&mut self) -> Option<Token> {
        let t = self.t.get(self.i).cloned();
        self.i += 1;
        t
    }

    fn or(&mut self) -> Option<Expr> {
        let mut e = self.and()?;
        while self.eat_keyword("OR") {
            e = Expr::Or(Box::new(e), Box::new(self.and()?));
        }
        Some(e)
    }
    fn and(&mut self) -> Option<Expr> {
        let mut e = self.not()?;
        while self.eat_keyword("AND") {
            e = Expr::And(Box::new(e), Box::new(self.not()?));
        }
        Some(e)
    }
    fn not(&mut self) -> Option<Expr> {
        if self.eat_keyword("NOT") {
            return Some(Expr::Not(Box::new(self.not()?)));
        }
        self.predicate()
    }
    fn predicate(&mut self) -> Option<Expr> {
        // A parenthesized condition (rather than a parenthesized operand).
        if self.peek() == Some(&Token::LParen) {
            let save = self.i;
            self.i += 1;
            if let Some(e) = self.or()
                && self.peek() == Some(&Token::RParen)
            {
                self.i += 1;
                return Some(e);
            }
            self.i = save;
        }
        let left = self.sum()?;
        let negated = self.eat_keyword("NOT");
        if self.eat_keyword("IN") {
            if self.next()? != Token::LParen {
                return None;
            }
            let mut list = vec![self.sum()?];
            while self.peek() == Some(&Token::Comma) {
                self.i += 1;
                list.push(self.sum()?);
            }
            if self.next()? != Token::RParen {
                return None;
            }
            return Some(Expr::In(left, list, negated));
        }
        if self.eat_keyword("LIKE") {
            let Token::Str(pattern) = self.next()? else {
                return None;
            };
            return Some(Expr::Like(left, pattern, negated));
        }
        if negated {
            return None;
        }
        if self.eat_keyword("IS") {
            let not = self.eat_keyword("NOT");
            return self.eat_keyword("NULL").then_some(Expr::IsNull(left, not));
        }
        match self.next()? {
            Token::Op(op) if ["=", "<>", "!=", "<", ">", "<=", ">="].contains(&op.as_str()) => {
                Some(Expr::Compare(left, op, self.sum()?))
            }
            _ => None,
        }
    }
    fn sum(&mut self) -> Option<Operand> {
        let mut a = self.product()?;
        while let Some(Token::Op(op)) = self.peek().cloned() {
            if op != "+" && op != "-" {
                break;
            }
            self.i += 1;
            a = Operand::Arith(Box::new(a), op.chars().next()?, Box::new(self.product()?));
        }
        Some(a)
    }
    fn product(&mut self) -> Option<Operand> {
        let mut a = self.atom()?;
        while let Some(Token::Op(op)) = self.peek().cloned() {
            if op != "*" && op != "/" {
                break;
            }
            self.i += 1;
            a = Operand::Arith(Box::new(a), op.chars().next()?, Box::new(self.atom()?));
        }
        Some(a)
    }
    fn atom(&mut self) -> Option<Operand> {
        match self.next()? {
            Token::Number(n) => Some(Operand::Value(Value::Double(n))),
            Token::Str(s) => Some(Operand::Value(Value::String(s))),
            Token::Op(op) if op == "-" => Some(Operand::Neg(Box::new(self.atom()?))),
            Token::LParen => {
                let o = self.sum()?;
                (self.next()? == Token::RParen).then_some(o)
            }
            Token::Ident(name) => {
                let keywords = ["AND", "OR", "NOT", "IN", "LIKE", "IS", "NULL", "BETWEEN"];
                if keywords.iter().any(|k| name.eq_ignore_ascii_case(k)) {
                    return None;
                }
                self.fields
                    .iter()
                    .position(|f| f.name.eq_ignore_ascii_case(&name))
                    .map(Operand::Field)
            }
            _ => None,
        }
    }
}

/// A parsed `where_clause`, evaluated against features of a table with `fields`.
#[derive(Debug, Clone)]
pub struct WhereClause {
    expr: Expr,
}

impl WhereClause {
    /// Parses `text`; an invalid expression is ArcGIS's ERROR 000358.
    pub fn parse(text: &str, fields: &[Field]) -> Result<Self> {
        let invalid = || Error::Unsupported(format!("ERROR 000358: Invalid expression {text}"));
        let tokens = tokenize(text).ok_or_else(invalid)?;
        let mut p = Parser {
            t: tokens,
            i: 0,
            fields,
        };
        let expr = p.or().ok_or_else(invalid)?;
        if p.i != p.t.len() {
            return Err(invalid());
        }
        Ok(Self { expr })
    }

    pub fn matches(&self, attributes: &[Value]) -> bool {
        eval(&self.expr, attributes) == Some(true)
    }
}

fn value(o: &Operand, a: &[Value]) -> Value {
    let num = |v: &Value| crate::number(v);
    match o {
        Operand::Field(i) => match &a[*i] {
            // dBASE text is blank-padded.
            Value::String(s) => Value::String(s.trim_end().to_owned()),
            v => v.clone(),
        },
        Operand::Value(v) => v.clone(),
        Operand::Neg(x) => num(&value(x, a)).map_or(Value::Null, |n| Value::Double(-n)),
        Operand::Arith(l, op, r) => match (num(&value(l, a)), num(&value(r, a))) {
            (Some(x), Some(y)) => Value::Double(match op {
                '+' => x + y,
                '-' => x - y,
                '*' => x * y,
                _ => x / y,
            }),
            _ => Value::Null,
        },
    }
}

/// Three-valued comparison: `None` when either side is null.
fn order(a: &Value, b: &Value) -> Option<std::cmp::Ordering> {
    if a.is_null() || b.is_null() {
        return None;
    }
    match (crate::number(a), crate::number(b)) {
        (Some(x), Some(y)) => x.partial_cmp(&y),
        _ => match (a, b) {
            (Value::String(x), Value::String(y)) => Some(x.cmp(y)),
            _ => None,
        },
    }
}

fn like(text: &str, pattern: &str) -> bool {
    let (t, p): (Vec<char>, Vec<char>) = (text.chars().collect(), pattern.chars().collect());
    // Classic wildcard match: % any run, _ one character.
    let (mut ti, mut pi, mut star, mut mark) = (0, 0, None, 0);
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '_' || p[pi] == t[ti]) {
            ti += 1;
            pi += 1;
        } else if pi < p.len() && p[pi] == '%' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '%' {
        pi += 1;
    }
    pi == p.len()
}

fn eval(e: &Expr, a: &[Value]) -> Option<bool> {
    use std::cmp::Ordering::*;
    match e {
        Expr::And(l, r) => match (eval(l, a), eval(r, a)) {
            (Some(false), _) | (_, Some(false)) => Some(false),
            (Some(true), Some(true)) => Some(true),
            _ => None,
        },
        Expr::Or(l, r) => match (eval(l, a), eval(r, a)) {
            (Some(true), _) | (_, Some(true)) => Some(true),
            (Some(false), Some(false)) => Some(false),
            _ => None,
        },
        Expr::Not(x) => eval(x, a).map(|b| !b),
        Expr::Compare(l, op, r) => {
            let o = order(&value(l, a), &value(r, a))?;
            Some(match op.as_str() {
                "=" => o == Equal,
                "<>" | "!=" => o != Equal,
                "<" => o == Less,
                ">" => o == Greater,
                "<=" => o != Greater,
                _ => o != Less,
            })
        }
        Expr::In(l, list, negated) => {
            let v = value(l, a);
            if v.is_null() {
                return None;
            }
            let hit = list.iter().any(|x| order(&v, &value(x, a)) == Some(Equal));
            Some(hit != *negated)
        }
        Expr::Like(l, pattern, negated) => match value(l, a) {
            Value::String(s) => Some(like(&s, pattern) != *negated),
            Value::Null => None,
            v => Some(like(&format!("{}", crate::number(&v)?), pattern) != *negated),
        },
        Expr::IsNull(l, not) => Some(value(l, a).is_null() != *not),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use unigis_core::FieldType;

    #[test]
    fn evaluates_where_clauses() {
        let fields = vec![
            Field::new("NAME", FieldType::String),
            Field::new("VALUE", FieldType::Double),
            Field::new("ZONE", FieldType::Integer),
        ];
        let row = vec![
            Value::String("Lot 12    ".into()),
            Value::Double(4200.5),
            Value::Integer(3),
        ];
        let t = |w: &str| WhereClause::parse(w, &fields).unwrap().matches(&row);
        assert!(t("VALUE > 4000"));
        assert!(t("ZONE IN (1, 3) AND VALUE < 8000"));
        assert!(t("NAME LIKE 'Lot 1%'"));
        assert!(!t("NAME LIKE 'Lot _'"));
        assert!(t("\"NAME\" = 'Lot 12'"));
        assert!(t("NOT ZONE = 2"));
        assert!(t(
            "(ZONE = 1 OR ZONE = 3) AND NOT (VALUE >= 3000 AND VALUE <= 4000)"
        ));
        assert!(t("ZONE NOT IN (0, 1)"));
        assert!(t("VALUE * 2 > 8000"));
        assert!(WhereClause::parse("ZONE BETWEEN 1 AND 3", &fields).is_err());
        assert!(WhereClause::parse("NOPE = 1", &fields).is_err());
    }
}
