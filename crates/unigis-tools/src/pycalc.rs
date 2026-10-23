//! A small Python 3 subset for Calculate Field expressions: arithmetic, comparisons,
//! `and`/`or`/`not`, conditional expressions, calls, attributes, subscripts and slices,
//! lists/tuples, `%` and f-string formatting, plus a code block with `def`, `if`/`elif`/
//! `else`, `for`, `while`, assignments and `return`. `!FIELD!` tokens stand for the
//! current row's values. The standard `math` module and the common builtins are there;
//! nothing else (no imports, classes, comprehensions, dictionaries or exceptions).

use std::collections::HashMap;
use std::rc::Rc;

/// A Python value.
#[derive(Debug, Clone)]
pub enum Py {
    None,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    List(Vec<Py>),
    Tuple(Vec<Py>),
    Func(Rc<FuncDef>),
    Builtin(String),
    Module,
    /// A method bound to its receiver.
    Method(Box<Py>, String),
}

#[derive(Debug)]
pub struct FuncDef {
    name: String,
    params: Vec<(String, Option<Expr>)>,
    body: Vec<Stmt>,
}

type Res<T> = std::result::Result<T, String>;

// ------------------------------------------------------------------ lexer

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Int(i64),
    Float(f64),
    Str(String, bool),
    Name(String),
    Op(String),
    Newline,
    Indent,
    Dedent,
    End,
}

fn lex(src: &str) -> Res<Vec<Tok>> {
    let mut toks = Vec::new();
    let mut indents = vec![0usize];
    let mut depth = 0i32;
    for raw in src.replace("\r\n", "\n").split('\n') {
        let line = raw.trim_end();
        let chars: Vec<char> = line.chars().collect();
        if depth == 0 {
            let indent = chars
                .iter()
                .take_while(|c| **c == ' ' || **c == '\t')
                .count();
            let rest = line.trim_start();
            if rest.is_empty() || rest.starts_with('#') {
                continue;
            }
            if indent > *indents.last().unwrap() {
                indents.push(indent);
                toks.push(Tok::Indent);
            }
            while indent < *indents.last().unwrap() {
                indents.pop();
                toks.push(Tok::Dedent);
            }
            if indent != *indents.last().unwrap() {
                return Err("inconsistent indentation".into());
            }
        }
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            if c == ' ' || c == '\t' {
                i += 1;
            } else if c == '#' {
                break;
            } else if c.is_ascii_digit()
                || (c == '.' && chars.get(i + 1).is_some_and(char::is_ascii_digit))
            {
                let start = i;
                let mut is_float = false;
                while i < chars.len() {
                    let d = chars[i];
                    if d.is_ascii_digit() || d == '_' {
                        i += 1;
                    } else if d == '.' && !is_float {
                        is_float = true;
                        i += 1;
                    } else if (d == 'e' || d == 'E')
                        && chars
                            .get(i + 1)
                            .is_some_and(|n| n.is_ascii_digit() || *n == '-' || *n == '+')
                    {
                        is_float = true;
                        i += 2;
                    } else {
                        break;
                    }
                }
                let text: String = chars[start..i].iter().filter(|c| **c != '_').collect();
                if is_float {
                    toks.push(Tok::Float(
                        text.parse().map_err(|_| format!("bad number {text}"))?,
                    ));
                } else {
                    toks.push(match text.parse::<i64>() {
                        Ok(n) => Tok::Int(n),
                        Err(_) => {
                            Tok::Float(text.parse().map_err(|_| format!("bad number {text}"))?)
                        }
                    });
                }
            } else if c.is_alphabetic() || c == '_' {
                let start = i;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();
                // String prefixes.
                let lower = word.to_ascii_lowercase();
                if i < chars.len()
                    && (chars[i] == '\'' || chars[i] == '"')
                    && ["f", "r", "b", "u", "fr", "rf"].contains(&lower.as_str())
                {
                    let (s, next) = lex_string(&chars, i, lower.contains('r'))?;
                    toks.push(Tok::Str(s, lower.contains('f')));
                    i = next;
                } else {
                    toks.push(Tok::Name(word));
                }
            } else if c == '\'' || c == '"' {
                let (s, next) = lex_string(&chars, i, false)?;
                toks.push(Tok::Str(s, false));
                i = next;
            } else {
                let three: String = chars[i..chars.len().min(i + 3)].iter().collect();
                let two: String = chars[i..chars.len().min(i + 2)].iter().collect();
                let op = if ["**=", "//=", ">>=", "<<="].contains(&three.as_str()) {
                    three
                } else if [
                    "**", "//", "==", "!=", "<=", ">=", "+=", "-=", "*=", "/=", "%=", "->", "<<",
                    ">>",
                ]
                .contains(&two.as_str())
                {
                    two
                } else if "+-*/%()[]{},:.=<>&|^~@;".contains(c) {
                    c.to_string()
                } else {
                    return Err(format!("unexpected character {c:?}"));
                };
                match op.as_str() {
                    "(" | "[" | "{" => depth += 1,
                    ")" | "]" | "}" => depth -= 1,
                    _ => {}
                }
                i += op.chars().count();
                toks.push(Tok::Op(op));
            }
        }
        if depth == 0 {
            toks.push(Tok::Newline);
        }
    }
    while indents.len() > 1 {
        indents.pop();
        toks.push(Tok::Dedent);
    }
    toks.push(Tok::End);
    Ok(toks)
}

fn lex_string(chars: &[char], start: usize, raw: bool) -> Res<(String, usize)> {
    let quote = chars[start];
    let triple = chars.get(start + 1) == Some(&quote) && chars.get(start + 2) == Some(&quote);
    let mut i = start + if triple { 3 } else { 1 };
    let mut out = String::new();
    loop {
        let Some(&c) = chars.get(i) else {
            return Err("unterminated string".into());
        };
        if c == quote {
            if !triple {
                return Ok((out, i + 1));
            }
            if chars.get(i + 1) == Some(&quote) && chars.get(i + 2) == Some(&quote) {
                return Ok((out, i + 3));
            }
        }
        if c == '\\' && !raw {
            i += 1;
            match chars.get(i) {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some('\\') => out.push('\\'),
                Some('\'') => out.push('\''),
                Some('"') => out.push('"'),
                Some('0') => out.push('\0'),
                Some(other) => {
                    out.push('\\');
                    out.push(*other);
                }
                None => return Err("unterminated string".into()),
            }
        } else {
            out.push(c);
        }
        i += 1;
    }
}

// ----------------------------------------------------------------- parser

#[derive(Debug, Clone)]
pub enum Expr {
    Const(Py),
    Name(String),
    Bin(Box<Expr>, String, Box<Expr>),
    Unary(String, Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    Compare(Box<Expr>, Vec<(String, Expr)>),
    IfExp(Box<Expr>, Box<Expr>, Box<Expr>),
    Call(Box<Expr>, Vec<Expr>, Vec<(String, Expr)>),
    Attr(Box<Expr>, String),
    Index(Box<Expr>, Box<Expr>),
    Slice(Option<Box<Expr>>, Option<Box<Expr>>, Option<Box<Expr>>),
    List(Vec<Expr>),
    Tuple(Vec<Expr>),
    /// Literal text and `{expression:spec}` parts.
    FString(Vec<FPart>),
}

#[derive(Debug, Clone)]
pub enum FPart {
    Text(String),
    Value(Expr, String, Option<char>),
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Expr(Expr),
    Assign(Vec<String>, Expr),
    Aug(String, String, Expr),
    Return(Option<Expr>),
    If(Vec<(Expr, Vec<Stmt>)>, Vec<Stmt>),
    For(Vec<String>, Expr, Vec<Stmt>),
    While(Expr, Vec<Stmt>),
    Def(Rc<FuncDef>),
    Pass,
    Break,
    Continue,
}

impl Clone for FuncDef {
    fn clone(&self) -> Self {
        FuncDef {
            name: self.name.clone(),
            params: self.params.clone(),
            body: self.body.clone(),
        }
    }
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> &Tok {
        &self.toks[self.pos]
    }
    fn next(&mut self) -> Tok {
        let t = self.toks[self.pos].clone();
        if self.pos + 1 < self.toks.len() {
            self.pos += 1;
        }
        t
    }
    fn is_op(&self, op: &str) -> bool {
        matches!(self.peek(), Tok::Op(o) if o == op)
    }
    fn is_name(&self, kw: &str) -> bool {
        matches!(self.peek(), Tok::Name(n) if n == kw)
    }
    fn eat_op(&mut self, op: &str) -> bool {
        if self.is_op(op) {
            self.next();
            true
        } else {
            false
        }
    }
    fn eat_name(&mut self, kw: &str) -> bool {
        if self.is_name(kw) {
            self.next();
            true
        } else {
            false
        }
    }
    fn expect_op(&mut self, op: &str) -> Res<()> {
        if self.eat_op(op) {
            Ok(())
        } else {
            Err(format!("expected `{op}`, found {:?}", self.peek()))
        }
    }

    fn program(&mut self) -> Res<Vec<Stmt>> {
        let mut out = Vec::new();
        while *self.peek() != Tok::End {
            if *self.peek() == Tok::Newline {
                self.next();
                continue;
            }
            out.push(self.statement()?);
        }
        Ok(out)
    }

    fn block(&mut self) -> Res<Vec<Stmt>> {
        self.expect_op(":")?;
        if *self.peek() != Tok::Newline {
            // `if x: return 1` on one line.
            let s = self.simple_statement()?;
            return Ok(vec![s]);
        }
        self.next();
        if *self.peek() != Tok::Indent {
            return Err("expected an indented block".into());
        }
        self.next();
        let mut body = Vec::new();
        while *self.peek() != Tok::Dedent && *self.peek() != Tok::End {
            if *self.peek() == Tok::Newline {
                self.next();
                continue;
            }
            body.push(self.statement()?);
        }
        if *self.peek() == Tok::Dedent {
            self.next();
        }
        Ok(body)
    }

    fn statement(&mut self) -> Res<Stmt> {
        if self.eat_name("def") {
            let Tok::Name(name) = self.next() else {
                return Err("function name expected".into());
            };
            self.expect_op("(")?;
            let mut params = Vec::new();
            while !self.is_op(")") {
                let Tok::Name(p) = self.next() else {
                    return Err("parameter name expected".into());
                };
                let default = if self.eat_op("=") {
                    Some(self.expr()?)
                } else {
                    None
                };
                params.push((p, default));
                if !self.eat_op(",") {
                    break;
                }
            }
            self.expect_op(")")?;
            let body = self.block()?;
            return Ok(Stmt::Def(Rc::new(FuncDef { name, params, body })));
        }
        if self.eat_name("if") {
            let mut branches = Vec::new();
            let cond = self.expr()?;
            branches.push((cond, self.block()?));
            let mut otherwise = Vec::new();
            loop {
                if self.eat_name("elif") {
                    let cond = self.expr()?;
                    branches.push((cond, self.block()?));
                } else if self.eat_name("else") {
                    otherwise = self.block()?;
                    break;
                } else {
                    break;
                }
            }
            return Ok(Stmt::If(branches, otherwise));
        }
        if self.eat_name("for") {
            let mut names = Vec::new();
            loop {
                let Tok::Name(n) = self.next() else {
                    return Err("loop variable expected".into());
                };
                names.push(n);
                if !self.eat_op(",") {
                    break;
                }
            }
            if !self.eat_name("in") {
                return Err("`in` expected".into());
            }
            let iter = self.expr_list()?;
            return Ok(Stmt::For(names, iter, self.block()?));
        }
        if self.eat_name("while") {
            let cond = self.expr()?;
            return Ok(Stmt::While(cond, self.block()?));
        }
        let s = self.simple_statement()?;
        if *self.peek() == Tok::Newline {
            self.next();
        }
        Ok(s)
    }

    fn simple_statement(&mut self) -> Res<Stmt> {
        if self.eat_name("return") {
            if matches!(self.peek(), Tok::Newline | Tok::End | Tok::Dedent) {
                return Ok(Stmt::Return(None));
            }
            return Ok(Stmt::Return(Some(self.expr_list()?)));
        }
        if self.eat_name("pass") {
            return Ok(Stmt::Pass);
        }
        if self.eat_name("break") {
            return Ok(Stmt::Break);
        }
        if self.eat_name("continue") {
            return Ok(Stmt::Continue);
        }
        if self.eat_name("import") || self.is_name("from") {
            // Only `math` is known; imports are accepted and ignored.
            while !matches!(self.peek(), Tok::Newline | Tok::End) {
                self.next();
            }
            return Ok(Stmt::Pass);
        }
        let first = self.expr_list()?;
        // Assignment / augmented assignment to names.
        let target = |e: &Expr| -> Option<Vec<String>> {
            match e {
                Expr::Name(n) => Some(vec![n.clone()]),
                Expr::Tuple(items) => items
                    .iter()
                    .map(|i| {
                        if let Expr::Name(n) = i {
                            Some(n.clone())
                        } else {
                            None
                        }
                    })
                    .collect(),
                _ => None,
            }
        };
        if self.eat_op("=") {
            let names = target(&first).ok_or("cannot assign to this expression")?;
            let value = self.expr_list()?;
            return Ok(Stmt::Assign(names, value));
        }
        for op in ["+=", "-=", "*=", "/=", "//=", "%=", "**="] {
            if self.eat_op(op) {
                let names = target(&first).ok_or("cannot assign to this expression")?;
                let value = self.expr_list()?;
                return Ok(Stmt::Aug(
                    names[0].clone(),
                    op[..op.len() - 1].to_owned(),
                    value,
                ));
            }
        }
        Ok(Stmt::Expr(first))
    }

    /// `a, b` (a tuple) or a single expression.
    fn expr_list(&mut self) -> Res<Expr> {
        let first = self.expr()?;
        if !self.is_op(",") {
            return Ok(first);
        }
        let mut items = vec![first];
        while self.eat_op(",") {
            if matches!(self.peek(), Tok::Newline | Tok::End) || self.is_op("=") || self.is_op(")")
            {
                break;
            }
            items.push(self.expr()?);
        }
        Ok(Expr::Tuple(items))
    }

    fn expr(&mut self) -> Res<Expr> {
        let body = self.or_expr()?;
        if self.eat_name("if") {
            let cond = self.or_expr()?;
            if !self.eat_name("else") {
                return Err("`else` expected in conditional expression".into());
            }
            let other = self.expr()?;
            return Ok(Expr::IfExp(Box::new(cond), Box::new(body), Box::new(other)));
        }
        Ok(body)
    }

    fn or_expr(&mut self) -> Res<Expr> {
        let mut l = self.and_expr()?;
        while self.eat_name("or") {
            l = Expr::Or(Box::new(l), Box::new(self.and_expr()?));
        }
        Ok(l)
    }
    fn and_expr(&mut self) -> Res<Expr> {
        let mut l = self.not_expr()?;
        while self.eat_name("and") {
            l = Expr::And(Box::new(l), Box::new(self.not_expr()?));
        }
        Ok(l)
    }
    fn not_expr(&mut self) -> Res<Expr> {
        if self.eat_name("not") {
            return Ok(Expr::Not(Box::new(self.not_expr()?)));
        }
        self.comparison()
    }
    fn comparison(&mut self) -> Res<Expr> {
        let first = self.bit_or()?;
        let mut rest = Vec::new();
        loop {
            let op = match self.peek() {
                Tok::Op(o) if ["==", "!=", "<", ">", "<=", ">="].contains(&o.as_str()) => {
                    let o = o.clone();
                    self.next();
                    o
                }
                Tok::Name(n) if n == "in" => {
                    self.next();
                    "in".to_owned()
                }
                Tok::Name(n) if n == "is" => {
                    self.next();
                    if self.eat_name("not") {
                        "is not".to_owned()
                    } else {
                        "is".to_owned()
                    }
                }
                Tok::Name(n)
                    if n == "not"
                        && matches!(self.toks.get(self.pos + 1), Some(Tok::Name(m)) if m == "in") =>
                {
                    self.next();
                    self.next();
                    "not in".to_owned()
                }
                _ => break,
            };
            rest.push((op, self.bit_or()?));
        }
        Ok(if rest.is_empty() {
            first
        } else {
            Expr::Compare(Box::new(first), rest)
        })
    }
    fn binary(&mut self, ops: &[&str], next: fn(&mut Parser) -> Res<Expr>) -> Res<Expr> {
        let mut l = next(self)?;
        loop {
            let op = match self.peek() {
                Tok::Op(o) if ops.contains(&o.as_str()) => o.clone(),
                _ => break,
            };
            self.next();
            let r = next(self)?;
            l = Expr::Bin(Box::new(l), op, Box::new(r));
        }
        Ok(l)
    }
    fn bit_or(&mut self) -> Res<Expr> {
        self.binary(&["|"], Parser::bit_xor)
    }
    fn bit_xor(&mut self) -> Res<Expr> {
        self.binary(&["^"], Parser::bit_and)
    }
    fn bit_and(&mut self) -> Res<Expr> {
        self.binary(&["&"], Parser::shift)
    }
    fn shift(&mut self) -> Res<Expr> {
        self.binary(&["<<", ">>"], Parser::arith)
    }
    fn arith(&mut self) -> Res<Expr> {
        self.binary(&["+", "-"], Parser::term)
    }
    fn term(&mut self) -> Res<Expr> {
        self.binary(&["*", "/", "//", "%"], Parser::factor)
    }
    fn factor(&mut self) -> Res<Expr> {
        for op in ["-", "+", "~"] {
            if self.eat_op(op) {
                return Ok(Expr::Unary(op.to_owned(), Box::new(self.factor()?)));
            }
        }
        self.power()
    }
    fn power(&mut self) -> Res<Expr> {
        let base = self.postfix()?;
        if self.eat_op("**") {
            let exp = self.factor()?;
            return Ok(Expr::Bin(Box::new(base), "**".into(), Box::new(exp)));
        }
        Ok(base)
    }

    fn postfix(&mut self) -> Res<Expr> {
        let mut e = self.atom()?;
        loop {
            if self.eat_op("(") {
                let (mut args, mut kwargs) = (Vec::new(), Vec::new());
                while !self.is_op(")") {
                    if let (Tok::Name(n), Some(Tok::Op(eq))) =
                        (self.peek().clone(), self.toks.get(self.pos + 1))
                        && eq == "="
                    {
                        self.next();
                        self.next();
                        kwargs.push((n, self.expr()?));
                    } else {
                        args.push(self.expr()?);
                    }
                    if !self.eat_op(",") {
                        break;
                    }
                }
                self.expect_op(")")?;
                e = Expr::Call(Box::new(e), args, kwargs);
            } else if self.eat_op(".") {
                let Tok::Name(n) = self.next() else {
                    return Err("attribute name expected".into());
                };
                e = Expr::Attr(Box::new(e), n);
            } else if self.eat_op("[") {
                let idx = self.subscript()?;
                self.expect_op("]")?;
                e = Expr::Index(Box::new(e), Box::new(idx));
            } else {
                break;
            }
        }
        Ok(e)
    }

    fn subscript(&mut self) -> Res<Expr> {
        let part = |p: &mut Parser| -> Res<Option<Box<Expr>>> {
            if p.is_op(":") || p.is_op("]") {
                Ok(None)
            } else {
                Ok(Some(Box::new(p.expr()?)))
            }
        };
        let lo = part(self)?;
        if !self.eat_op(":") {
            return lo.map(|b| *b).ok_or_else(|| "empty subscript".to_owned());
        }
        let hi = part(self)?;
        let step = if self.eat_op(":") { part(self)? } else { None };
        Ok(Expr::Slice(lo, hi, step))
    }

    fn atom(&mut self) -> Res<Expr> {
        match self.next() {
            Tok::Int(n) => Ok(Expr::Const(Py::Int(n))),
            Tok::Float(f) => Ok(Expr::Const(Py::Float(f))),
            Tok::Str(s, is_f) => {
                let mut parts = if is_f {
                    parse_fstring(&s)?
                } else {
                    vec![FPart::Text(s)]
                };
                // Adjacent literals concatenate.
                while let Tok::Str(next, f2) = self.peek().clone() {
                    self.next();
                    if f2 {
                        parts.extend(parse_fstring(&next)?);
                    } else {
                        parts.push(FPart::Text(next));
                    }
                }
                if parts.iter().all(|p| matches!(p, FPart::Text(_))) {
                    let text: String = parts
                        .iter()
                        .map(|p| {
                            if let FPart::Text(t) = p {
                                t.as_str()
                            } else {
                                ""
                            }
                        })
                        .collect();
                    return Ok(Expr::Const(Py::Str(text)));
                }
                Ok(Expr::FString(parts))
            }
            Tok::Name(n) => match n.as_str() {
                "None" => Ok(Expr::Const(Py::None)),
                "True" => Ok(Expr::Const(Py::Bool(true))),
                "False" => Ok(Expr::Const(Py::Bool(false))),
                _ => Ok(Expr::Name(n)),
            },
            Tok::Op(o) if o == "(" => {
                if self.eat_op(")") {
                    return Ok(Expr::Tuple(Vec::new()));
                }
                let first = self.expr()?;
                if self.is_op(",") {
                    let mut items = vec![first];
                    while self.eat_op(",") {
                        if self.is_op(")") {
                            break;
                        }
                        items.push(self.expr()?);
                    }
                    self.expect_op(")")?;
                    return Ok(Expr::Tuple(items));
                }
                self.expect_op(")")?;
                Ok(first)
            }
            Tok::Op(o) if o == "[" => {
                let mut items = Vec::new();
                while !self.is_op("]") {
                    items.push(self.expr()?);
                    if !self.eat_op(",") {
                        break;
                    }
                }
                self.expect_op("]")?;
                Ok(Expr::List(items))
            }
            other => Err(format!("unexpected {other:?}")),
        }
    }
}

/// `!NAME!` becomes the identifier `__F__NAME` (upper case), in strings too, as ArcGIS
/// substitutes its field tokens before the expression is parsed.
pub fn substitute_fields(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '!'
            && chars
                .get(i + 1)
                .is_some_and(|c| c.is_alphabetic() || *c == '_')
        {
            let mut j = i + 1;
            while j < chars.len() && (chars[j].is_alphanumeric() || chars[j] == '_') {
                j += 1;
            }
            if chars.get(j) == Some(&'!') {
                let name: String = chars[i + 1..j].iter().collect();
                out.push_str("__F__");
                out.push_str(&name.to_uppercase());
                i = j + 1;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// The field names a calculation refers to, in order of appearance (upper case).
pub fn field_tokens(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    for word in substitute_fields(text).split(|c: char| !(c.is_alphanumeric() || c == '_')) {
        if let Some(n) = word.strip_prefix("__F__")
            && !names.iter().any(|x: &String| x == n)
        {
            names.push(n.to_owned());
        }
    }
    names
}

fn parse_fstring(s: &str) -> Res<Vec<FPart>> {
    let chars: Vec<char> = s.chars().collect();
    let mut parts = Vec::new();
    let mut text = String::new();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '{' if chars.get(i + 1) == Some(&'{') => {
                text.push('{');
                i += 2;
            }
            '}' if chars.get(i + 1) == Some(&'}') => {
                text.push('}');
                i += 2;
            }
            '{' => {
                if !text.is_empty() {
                    parts.push(FPart::Text(std::mem::take(&mut text)));
                }
                let mut depth = 1;
                let mut j = i + 1;
                while j < chars.len() && depth > 0 {
                    match chars[j] {
                        '{' => depth += 1,
                        '}' => depth -= 1,
                        _ => {}
                    }
                    j += 1;
                }
                if depth != 0 {
                    return Err("unbalanced braces in f-string".into());
                }
                let inner: String = chars[i + 1..j - 1].iter().collect();
                // expr[!r][:spec] — split at a top-level `:` (outside brackets and quotes).
                let (mut expr_end, mut spec) = (inner.len(), String::new());
                let (mut d, mut quote) = (0i32, None);
                for (k, c) in inner.char_indices() {
                    match (c, quote) {
                        ('\'' | '"', None) => quote = Some(c),
                        (q, Some(o)) if q == o => quote = None,
                        ('(' | '[' | '{', None) => d += 1,
                        (')' | ']' | '}', None) => d -= 1,
                        (':', None) if d == 0 => {
                            expr_end = k;
                            spec = inner[k + 1..].to_owned();
                            break;
                        }
                        _ => {}
                    }
                }
                let mut expr_text = inner[..expr_end].trim().to_owned();
                let mut conv = None;
                if let Some(p) = expr_text.rfind("!r").or_else(|| expr_text.rfind("!s")) {
                    conv = expr_text[p + 1..].chars().next();
                    expr_text.truncate(p);
                }
                let toks = lex(&expr_text)?;
                let mut p = Parser { toks, pos: 0 };
                parts.push(FPart::Value(p.expr()?, spec, conv));
                i = j;
            }
            c => {
                text.push(c);
                i += 1;
            }
        }
    }
    if !text.is_empty() {
        parts.push(FPart::Text(text));
    }
    Ok(parts)
}

// ------------------------------------------------------------------ values

impl Py {
    fn truthy(&self) -> bool {
        match self {
            Py::None => false,
            Py::Bool(b) => *b,
            Py::Int(n) => *n != 0,
            Py::Float(f) => *f != 0.0,
            Py::Str(s) => !s.is_empty(),
            Py::List(v) | Py::Tuple(v) => !v.is_empty(),
            _ => true,
        }
    }
    fn as_f64(&self) -> Option<f64> {
        match self {
            Py::Bool(b) => Some(f64::from(u8::from(*b))),
            Py::Int(n) => Some(*n as f64),
            Py::Float(f) => Some(*f),
            _ => None,
        }
    }
    fn as_int(&self) -> Option<i64> {
        match self {
            Py::Bool(b) => Some(i64::from(*b)),
            Py::Int(n) => Some(*n),
            _ => None,
        }
    }
    fn type_name(&self) -> &'static str {
        match self {
            Py::None => "NoneType",
            Py::Bool(_) => "bool",
            Py::Int(_) => "int",
            Py::Float(_) => "float",
            Py::Str(_) => "str",
            Py::List(_) => "list",
            Py::Tuple(_) => "tuple",
            _ => "function",
        }
    }
}

/// Python's `repr` of a float.
fn float_repr(f: f64) -> String {
    if f.is_nan() {
        return "nan".into();
    }
    if f.is_infinite() {
        return if f > 0.0 { "inf" } else { "-inf" }.into();
    }
    let abs = f.abs();
    if abs != 0.0 && !(1e-4..1e16).contains(&abs) {
        let s = format!("{f:e}");
        let (m, e) = s.split_once('e').unwrap();
        let exp: i32 = e.parse().unwrap();
        return format!("{m}e{}{:02}", if exp < 0 { '-' } else { '+' }, exp.abs());
    }
    let s = format!("{f}");
    if s.contains('.') { s } else { format!("{s}.0") }
}

/// `str(value)`.
pub fn to_text(v: &Py) -> String {
    str_of(v)
}

fn str_of(v: &Py) -> String {
    match v {
        Py::None => "None".into(),
        Py::Bool(true) => "True".into(),
        Py::Bool(false) => "False".into(),
        Py::Int(n) => n.to_string(),
        Py::Float(f) => float_repr(*f),
        Py::Str(s) => s.clone(),
        Py::List(items) => format!(
            "[{}]",
            items.iter().map(repr_of).collect::<Vec<_>>().join(", ")
        ),
        Py::Tuple(items) => {
            if items.len() == 1 {
                format!("({},)", repr_of(&items[0]))
            } else {
                format!(
                    "({})",
                    items.iter().map(repr_of).collect::<Vec<_>>().join(", ")
                )
            }
        }
        _ => "<function>".into(),
    }
}

fn repr_of(v: &Py) -> String {
    match v {
        Py::Str(s) => {
            if s.contains('\'') && !s.contains('"') {
                format!("\"{s}\"")
            } else {
                format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'"))
            }
        }
        other => str_of(other),
    }
}

fn py_eq(a: &Py, b: &Py) -> bool {
    match (a, b) {
        (Py::None, Py::None) => true,
        (Py::Str(x), Py::Str(y)) => x == y,
        (Py::List(x), Py::List(y)) | (Py::Tuple(x), Py::Tuple(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| py_eq(p, q))
        }
        _ => match (a.as_f64(), b.as_f64()) {
            (Some(x), Some(y)) => x == y,
            _ => false,
        },
    }
}

fn py_cmp(a: &Py, b: &Py) -> Res<std::cmp::Ordering> {
    use std::cmp::Ordering;
    match (a, b) {
        (Py::Str(x), Py::Str(y)) => Ok(x.cmp(y)),
        (Py::List(x), Py::List(y)) | (Py::Tuple(x), Py::Tuple(y)) => {
            for (p, q) in x.iter().zip(y) {
                let o = py_cmp(p, q)?;
                if o != Ordering::Equal {
                    return Ok(o);
                }
            }
            Ok(x.len().cmp(&y.len()))
        }
        _ => match (a.as_int(), b.as_int()) {
            (Some(x), Some(y)) => Ok(x.cmp(&y)),
            _ => match (a.as_f64(), b.as_f64()) {
                (Some(x), Some(y)) => x
                    .partial_cmp(&y)
                    .ok_or_else(|| "cannot compare NaN".to_owned()),
                _ => Err(format!(
                    "'<' not supported between instances of '{}' and '{}'",
                    a.type_name(),
                    b.type_name()
                )),
            },
        },
    }
}

// -------------------------------------------------------------- formatting

/// Python's format-spec mini-language for one value.
fn format_value(v: &Py, spec: &str) -> Res<String> {
    if spec.is_empty() {
        return Ok(str_of(v));
    }
    let chars: Vec<char> = spec.chars().collect();
    let mut i = 0;
    let (mut fill, mut align) = (' ', None);
    if chars.len() >= 2 && "<>^=".contains(chars[1]) {
        fill = chars[0];
        align = Some(chars[1]);
        i = 2;
    } else if !chars.is_empty() && "<>^=".contains(chars[0]) {
        align = Some(chars[0]);
        i = 1;
    }
    let mut sign = '-';
    if i < chars.len() && "+- ".contains(chars[i]) {
        sign = chars[i];
        i += 1;
    }
    if i < chars.len() && chars[i] == '#' {
        i += 1;
    }
    let mut zero = false;
    if i < chars.len() && chars[i] == '0' {
        zero = true;
        i += 1;
    }
    let mut width = 0usize;
    while i < chars.len() && chars[i].is_ascii_digit() {
        width = width * 10 + chars[i].to_digit(10).unwrap() as usize;
        i += 1;
    }
    let mut group = false;
    if i < chars.len() && (chars[i] == ',' || chars[i] == '_') {
        group = true;
        i += 1;
    }
    let mut precision = None;
    if i < chars.len() && chars[i] == '.' {
        i += 1;
        let mut p = 0usize;
        while i < chars.len() && chars[i].is_ascii_digit() {
            p = p * 10 + chars[i].to_digit(10).unwrap() as usize;
            i += 1;
        }
        precision = Some(p);
    }
    let kind = chars.get(i).copied();
    let numeric = |v: &Py| v.as_f64();
    let mut body = match (kind, v) {
        (Some('d'), _) => {
            let n = v.as_int().ok_or("format code 'd' needs an integer")?;
            n.abs().to_string()
        }
        (Some('f' | 'F'), _) => {
            let f = numeric(v).ok_or("format code 'f' needs a number")?;
            format!("{:.*}", precision.unwrap_or(6), f.abs())
        }
        (Some('e' | 'E'), _) => {
            let f = numeric(v).ok_or("format code 'e' needs a number")?;
            let s = format!("{:.*e}", precision.unwrap_or(6), f.abs());
            let (m, e) = s.split_once('e').unwrap();
            let exp: i32 = e.parse().unwrap();
            let out = format!("{m}e{}{:02}", if exp < 0 { '-' } else { '+' }, exp.abs());
            if kind == Some('E') {
                out.to_uppercase()
            } else {
                out
            }
        }
        (Some('g' | 'G'), _) => {
            let f = numeric(v).ok_or("format code 'g' needs a number")?;
            general(f.abs(), precision.unwrap_or(6))
        }
        (Some('%'), _) => {
            let f = numeric(v).ok_or("format code '%' needs a number")?;
            format!("{:.*}%", precision.unwrap_or(6), f.abs() * 100.0)
        }
        (Some('x'), _) => format!(
            "{:x}",
            v.as_int().ok_or("format code 'x' needs an integer")?.abs()
        ),
        (Some('X'), _) => format!(
            "{:X}",
            v.as_int().ok_or("format code 'X' needs an integer")?.abs()
        ),
        (Some('b'), _) => format!(
            "{:b}",
            v.as_int().ok_or("format code 'b' needs an integer")?.abs()
        ),
        (Some('o'), _) => format!(
            "{:o}",
            v.as_int().ok_or("format code 'o' needs an integer")?.abs()
        ),
        (Some('s'), _) | (None, Py::Str(_)) => {
            let mut s = str_of(v);
            if let Some(p) = precision {
                s = s.chars().take(p).collect();
            }
            s
        }
        (None, Py::Int(_) | Py::Bool(_)) => v.as_int().unwrap().abs().to_string(),
        (None, Py::Float(f)) => match precision {
            Some(p) => general(f.abs(), p.max(1)),
            None => float_repr(f.abs()),
        },
        (None, other) => str_of(other),
        (Some(c), _) => return Err(format!("unknown format code '{c}'")),
    };
    let negative = !matches!(kind, Some('s'))
        && numeric(v).is_some_and(|f| f < 0.0 || (f == 0.0 && f.is_sign_negative()))
        && !matches!(v, Py::Str(_));
    if group {
        let (int_part, rest) = match body.find(|c: char| !c.is_ascii_digit()) {
            Some(p) => (body[..p].to_owned(), body[p..].to_owned()),
            None => (body.clone(), String::new()),
        };
        let mut grouped = String::new();
        for (k, c) in int_part.chars().enumerate() {
            if k > 0 && (int_part.len() - k) % 3 == 0 {
                grouped.push(',');
            }
            grouped.push(c);
        }
        body = grouped + &rest;
    }
    let sign_text = if negative {
        "-"
    } else if sign == '+' && numeric(v).is_some() {
        "+"
    } else if sign == ' ' && numeric(v).is_some() {
        " "
    } else {
        ""
    };
    let len = sign_text.chars().count() + body.chars().count();
    if width <= len {
        return Ok(format!("{sign_text}{body}"));
    }
    let pad = width - len;
    let default_align = if matches!(v, Py::Str(_)) { '<' } else { '>' };
    let (fill, align) = if zero && align.is_none() {
        ('0', '=')
    } else {
        (fill, align.unwrap_or(default_align))
    };
    let f = |n: usize| fill.to_string().repeat(n);
    Ok(match align {
        '<' => format!("{sign_text}{body}{}", f(pad)),
        '^' => format!("{}{sign_text}{body}{}", f(pad / 2), f(pad - pad / 2)),
        '=' => format!("{sign_text}{}{body}", f(pad)),
        _ => format!("{}{sign_text}{body}", f(pad)),
    })
}

/// C's `%g` / Python's `g`.
fn general(f: f64, precision: usize) -> String {
    if f == 0.0 {
        return "0".into();
    }
    let p = precision.max(1);
    let exp = f.abs().log10().floor() as i32;
    if exp < -4 || exp >= p as i32 {
        let s = format!("{:.*e}", p - 1, f);
        let (m, e) = s.split_once('e').unwrap();
        let m = if m.contains('.') {
            m.trim_end_matches('0').trim_end_matches('.')
        } else {
            m
        };
        let exp: i32 = e.parse().unwrap();
        format!("{m}e{}{:02}", if exp < 0 { '-' } else { '+' }, exp.abs())
    } else {
        let decimals = (p as i32 - 1 - exp).max(0) as usize;
        let s = format!("{f:.decimals$}");
        if s.contains('.') {
            s.trim_end_matches('0').trim_end_matches('.').to_owned()
        } else {
            s
        }
    }
}

/// `template % args`.
fn percent_format(template: &str, args: &Py) -> Res<String> {
    let items: Vec<Py> = match args {
        Py::Tuple(v) => v.clone(),
        other => vec![other.clone()],
    };
    let chars: Vec<char> = template.chars().collect();
    let mut out = String::new();
    let (mut i, mut next_arg) = (0, 0);
    while i < chars.len() {
        if chars[i] != '%' {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        i += 1;
        if chars.get(i) == Some(&'%') {
            out.push('%');
            i += 1;
            continue;
        }
        let mut spec = String::new();
        while i < chars.len() && !chars[i].is_alphabetic() {
            spec.push(chars[i]);
            i += 1;
        }
        let Some(&code) = chars.get(i) else {
            return Err("incomplete format".into());
        };
        i += 1;
        let arg = items
            .get(next_arg)
            .ok_or("not enough arguments for format string")?;
        next_arg += 1;
        // Flags: '-' means left-align, '+' sign, '0' zero pad.
        let mut s = spec.clone();
        let left = s.starts_with('-');
        if left {
            s.remove(0);
        }
        let spec_py = match code {
            's' | 'r' => format!("{}{}", if left { "<" } else { ">" }, s),
            'd' | 'i' | 'f' | 'e' | 'g' | 'E' | 'G' | 'x' | 'X' | 'o' => {
                let kind = if code == 'i' { 'd' } else { code };
                let zero = s.starts_with('0');
                format!("{}{}{}", if left { "<" } else { "" }, s, kind)
                    .replace("0.", if zero { "0." } else { "." })
            }
            other => return Err(format!("unsupported format character '{other}'")),
        };
        let value = match (code, arg) {
            ('d' | 'i', Py::Float(f)) => Py::Int(f.trunc() as i64),
            ('r', v) => Py::Str(repr_of(v)),
            ('s', v) => Py::Str(str_of(v)),
            (_, v) => v.clone(),
        };
        out.push_str(&format_value(&value, &spec_py)?);
    }
    Ok(out)
}

// -------------------------------------------------------------- evaluator

enum Flow {
    Next,
    Return(Py),
    Break,
    Continue,
}

pub struct Interp {
    globals: HashMap<String, Py>,
}

const BUILTINS: &[&str] = &[
    "abs", "round", "int", "float", "str", "len", "max", "min", "sum", "bool", "range", "sorted",
    "list", "tuple", "pow", "divmod", "repr",
];

impl Interp {
    /// An interpreter with the code block run (definitions and statements).
    pub fn new(code_block: &str) -> Res<Self> {
        let mut globals: HashMap<String, Py> = HashMap::new();
        for b in BUILTINS {
            globals.insert((*b).to_owned(), Py::Builtin((*b).to_owned()));
        }
        globals.insert("math".into(), Py::Module);
        let mut it = Interp { globals };
        if !code_block.trim().is_empty() {
            let toks = lex(code_block)?;
            let prog = Parser { toks, pos: 0 }.program()?;
            let mut scope = HashMap::new();
            for s in &prog {
                it.exec(s, &mut scope, true)?;
            }
            it.globals.extend(scope);
        }
        Ok(it)
    }

    /// Evaluates `expression` with the given `!NAME!` values (a map from upper-case field
    /// name to value).
    pub fn eval(&self, expression: &str, fields: &HashMap<String, Py>) -> Res<Py> {
        let toks = lex(substitute_fields(expression.trim()).as_str())?;
        let mut p = Parser { toks, pos: 0 };
        let e = p.expr_list()?;
        if !matches!(p.peek(), Tok::Newline | Tok::End) {
            return Err(format!("unexpected {:?}", p.peek()));
        }
        let mut scope: HashMap<String, Py> = fields
            .iter()
            .map(|(k, v)| (format!("__F__{k}"), v.clone()))
            .collect();
        self.expr(&e, &mut scope)
    }

    fn lookup(&self, name: &str, scope: &HashMap<String, Py>) -> Res<Py> {
        scope
            .get(name)
            .or_else(|| self.globals.get(name))
            .cloned()
            .ok_or_else(|| format!("name '{name}' is not defined"))
    }

    fn exec(&self, s: &Stmt, scope: &mut HashMap<String, Py>, top: bool) -> Res<Flow> {
        match s {
            Stmt::Expr(e) => {
                self.expr(e, scope)?;
            }
            Stmt::Pass => {}
            Stmt::Assign(names, e) => {
                let v = self.expr(e, scope)?;
                if names.len() == 1 {
                    scope.insert(names[0].clone(), v);
                } else {
                    let items = self.iterate(&v)?;
                    if items.len() != names.len() {
                        return Err("cannot unpack: wrong number of values".into());
                    }
                    for (n, v) in names.iter().zip(items) {
                        scope.insert(n.clone(), v);
                    }
                }
            }
            Stmt::Aug(name, op, e) => {
                let cur = self.lookup(name, scope)?;
                let rhs = self.expr(e, scope)?;
                scope.insert(name.clone(), self.binop(&cur, op, &rhs)?);
            }
            Stmt::Return(e) => {
                let v = match e {
                    Some(e) => self.expr(e, scope)?,
                    None => Py::None,
                };
                return Ok(Flow::Return(v));
            }
            Stmt::Def(f) => {
                scope.insert(f.name.clone(), Py::Func(f.clone()));
            }
            Stmt::If(branches, otherwise) => {
                for (cond, body) in branches {
                    if self.expr(cond, scope)?.truthy() {
                        return self.block(body, scope, top);
                    }
                }
                return self.block(otherwise, scope, top);
            }
            Stmt::For(names, iter, body) => {
                let seq = self.expr(iter, scope)?;
                for item in self.iterate(&seq)? {
                    if names.len() == 1 {
                        scope.insert(names[0].clone(), item);
                    } else {
                        let parts = self.iterate(&item)?;
                        for (n, v) in names.iter().zip(parts) {
                            scope.insert(n.clone(), v);
                        }
                    }
                    match self.block(body, scope, top)? {
                        Flow::Return(v) => return Ok(Flow::Return(v)),
                        Flow::Break => break,
                        _ => {}
                    }
                }
            }
            Stmt::While(cond, body) => {
                let mut guard = 0u32;
                while self.expr(cond, scope)?.truthy() {
                    guard += 1;
                    if guard > 1_000_000 {
                        return Err("loop does not end".into());
                    }
                    match self.block(body, scope, top)? {
                        Flow::Return(v) => return Ok(Flow::Return(v)),
                        Flow::Break => break,
                        _ => {}
                    }
                }
            }
            Stmt::Break => return Ok(Flow::Break),
            Stmt::Continue => return Ok(Flow::Continue),
        }
        Ok(Flow::Next)
    }

    fn block(&self, body: &[Stmt], scope: &mut HashMap<String, Py>, top: bool) -> Res<Flow> {
        for s in body {
            match self.exec(s, scope, top)? {
                Flow::Next => {}
                other => return Ok(other),
            }
        }
        Ok(Flow::Next)
    }

    fn iterate(&self, v: &Py) -> Res<Vec<Py>> {
        match v {
            Py::List(x) | Py::Tuple(x) => Ok(x.clone()),
            Py::Str(s) => Ok(s.chars().map(|c| Py::Str(c.to_string())).collect()),
            other => Err(format!("'{}' object is not iterable", other.type_name())),
        }
    }

    fn call_func(&self, f: &FuncDef, args: Vec<Py>, kwargs: Vec<(String, Py)>) -> Res<Py> {
        let mut local: HashMap<String, Py> = HashMap::new();
        if args.len() > f.params.len() {
            return Err(format!("{}() takes {} arguments", f.name, f.params.len()));
        }
        for (i, (name, default)) in f.params.iter().enumerate() {
            let v = if let Some(a) = args.get(i) {
                a.clone()
            } else if let Some((_, v)) = kwargs.iter().find(|(k, _)| k == name) {
                v.clone()
            } else if let Some(d) = default {
                self.expr(d, &mut HashMap::new())?
            } else {
                return Err(format!("{}() missing argument '{name}'", f.name));
            };
            local.insert(name.clone(), v);
        }
        match self.block(&f.body, &mut local, false)? {
            Flow::Return(v) => Ok(v),
            _ => Ok(Py::None),
        }
    }

    fn expr(&self, e: &Expr, scope: &mut HashMap<String, Py>) -> Res<Py> {
        match e {
            Expr::Const(v) => Ok(v.clone()),
            Expr::Name(n) => self.lookup(n, scope),
            Expr::List(items) => Ok(Py::List(
                items
                    .iter()
                    .map(|i| self.expr(i, scope))
                    .collect::<Res<_>>()?,
            )),
            Expr::Tuple(items) => Ok(Py::Tuple(
                items
                    .iter()
                    .map(|i| self.expr(i, scope))
                    .collect::<Res<_>>()?,
            )),
            Expr::Unary(op, x) => {
                let v = self.expr(x, scope)?;
                match (op.as_str(), &v) {
                    ("-", Py::Int(n)) => Ok(Py::Int(-n)),
                    ("-", Py::Float(f)) => Ok(Py::Float(-f)),
                    ("-", Py::Bool(b)) => Ok(Py::Int(-i64::from(*b))),
                    ("+", Py::Int(_) | Py::Float(_)) => Ok(v),
                    ("~", Py::Int(n)) => Ok(Py::Int(!n)),
                    _ => Err(format!(
                        "bad operand type for unary {op}: '{}'",
                        v.type_name()
                    )),
                }
            }
            Expr::Not(x) => Ok(Py::Bool(!self.expr(x, scope)?.truthy())),
            Expr::And(a, b) => {
                let l = self.expr(a, scope)?;
                if l.truthy() {
                    self.expr(b, scope)
                } else {
                    Ok(l)
                }
            }
            Expr::Or(a, b) => {
                let l = self.expr(a, scope)?;
                if l.truthy() {
                    Ok(l)
                } else {
                    self.expr(b, scope)
                }
            }
            Expr::IfExp(c, a, b) => {
                if self.expr(c, scope)?.truthy() {
                    self.expr(a, scope)
                } else {
                    self.expr(b, scope)
                }
            }
            Expr::Bin(a, op, b) => {
                let (l, r) = (self.expr(a, scope)?, self.expr(b, scope)?);
                self.binop(&l, op, &r)
            }
            Expr::Compare(first, rest) => {
                let mut left = self.expr(first, scope)?;
                for (op, rhs) in rest {
                    let right = self.expr(rhs, scope)?;
                    let ok = match op.as_str() {
                        "==" => py_eq(&left, &right),
                        "!=" => !py_eq(&left, &right),
                        "<" => py_cmp(&left, &right)?.is_lt(),
                        ">" => py_cmp(&left, &right)?.is_gt(),
                        "<=" => py_cmp(&left, &right)?.is_le(),
                        ">=" => py_cmp(&left, &right)?.is_ge(),
                        "is" => match (&left, &right) {
                            (Py::None, Py::None) => true,
                            (Py::Bool(a), Py::Bool(b)) => a == b,
                            _ => false,
                        },
                        "is not" => !match (&left, &right) {
                            (Py::None, Py::None) => true,
                            (Py::Bool(a), Py::Bool(b)) => a == b,
                            _ => false,
                        },
                        "in" | "not in" => {
                            let found = match &right {
                                Py::Str(s) => match &left {
                                    Py::Str(p) => s.contains(p.as_str()),
                                    _ => return Err("'in <string>' needs a string".into()),
                                },
                                Py::List(v) | Py::Tuple(v) => v.iter().any(|x| py_eq(x, &left)),
                                _ => return Err("argument is not iterable".into()),
                            };
                            found == (op == "in")
                        }
                        _ => unreachable!(),
                    };
                    if !ok {
                        return Ok(Py::Bool(false));
                    }
                    left = right;
                }
                Ok(Py::Bool(true))
            }
            Expr::Slice(..) => Err("slice outside a subscript".into()),
            Expr::Index(base, idx) => {
                let b = self.expr(base, scope)?;
                if let Expr::Slice(lo, hi, step) = &**idx {
                    let get =
                        |x: &Option<Box<Expr>>, s: &mut HashMap<String, Py>| -> Res<Option<i64>> {
                            match x {
                                None => Ok(None),
                                Some(e) => Ok(Some(
                                    self.expr(e, s)?
                                        .as_int()
                                        .ok_or("slice indices must be integers")?,
                                )),
                            }
                        };
                    let (lo, hi, step) = (get(lo, scope)?, get(hi, scope)?, get(step, scope)?);
                    return slice(&b, lo, hi, step.unwrap_or(1));
                }
                let i = self
                    .expr(idx, scope)?
                    .as_int()
                    .ok_or("indices must be integers")?;
                let item = |len: usize| -> Res<usize> {
                    let k = if i < 0 { i + len as i64 } else { i };
                    if k < 0 || k >= len as i64 {
                        Err("index out of range".into())
                    } else {
                        Ok(k as usize)
                    }
                };
                match &b {
                    Py::Str(s) => {
                        let chars: Vec<char> = s.chars().collect();
                        Ok(Py::Str(chars[item(chars.len())?].to_string()))
                    }
                    Py::List(v) | Py::Tuple(v) => Ok(v[item(v.len())?].clone()),
                    other => Err(format!(
                        "'{}' object is not subscriptable",
                        other.type_name()
                    )),
                }
            }
            Expr::Attr(base, name) => {
                let b = self.expr(base, scope)?;
                if matches!(b, Py::Module) {
                    return math_attr(name);
                }
                Ok(Py::Method(Box::new(b), name.clone()))
            }
            Expr::Call(f, args, kwargs) => {
                let callee = self.expr(f, scope)?;
                let argv: Vec<Py> = args
                    .iter()
                    .map(|a| self.expr(a, scope))
                    .collect::<Res<_>>()?;
                let kw: Vec<(String, Py)> = kwargs
                    .iter()
                    .map(|(k, v)| Ok((k.clone(), self.expr(v, scope)?)))
                    .collect::<Res<_>>()?;
                match callee {
                    Py::Func(def) => self.call_func(&def, argv, kw),
                    Py::Builtin(name) => call_builtin(&name, argv, &kw),
                    Py::Method(recv, name) => call_method(&recv, &name, argv),
                    other => Err(format!("'{}' object is not callable", other.type_name())),
                }
            }
            Expr::FString(parts) => {
                let mut out = String::new();
                for p in parts {
                    match p {
                        FPart::Text(t) => out.push_str(t),
                        FPart::Value(e, spec, conv) => {
                            let v = self.expr(e, scope)?;
                            let v = match conv {
                                Some('r') => Py::Str(repr_of(&v)),
                                Some('s') => Py::Str(str_of(&v)),
                                _ => v,
                            };
                            out.push_str(&format_value(&v, spec)?);
                        }
                    }
                }
                Ok(Py::Str(out))
            }
        }
    }

    fn binop(&self, l: &Py, op: &str, r: &Py) -> Res<Py> {
        use Py::*;
        // Strings and sequences.
        match (op, l, r) {
            ("+", Str(a), Str(b)) => return Ok(Str(format!("{a}{b}"))),
            ("+", List(a), List(b)) => return Ok(List(a.iter().chain(b).cloned().collect())),
            ("+", Tuple(a), Tuple(b)) => return Ok(Tuple(a.iter().chain(b).cloned().collect())),
            ("*", Str(s), n) | ("*", n, Str(s)) if n.as_int().is_some() => {
                return Ok(Str(s.repeat(n.as_int().unwrap().max(0) as usize)));
            }
            ("%", Str(t), args) => return Ok(Str(percent_format(t, args)?)),
            _ => {}
        }
        let bad = || {
            format!(
                "unsupported operand type(s) for {op}: '{}' and '{}'",
                l.type_name(),
                r.type_name()
            )
        };
        if let (Some(a), Some(b)) = (l.as_int(), r.as_int())
            && !matches!(l, Float(_))
            && !matches!(r, Float(_))
        {
            return Ok(match op {
                "+" => a.checked_add(b).map_or(Float(a as f64 + b as f64), Int),
                "-" => a.checked_sub(b).map_or(Float(a as f64 - b as f64), Int),
                "*" => a.checked_mul(b).map_or(Float(a as f64 * b as f64), Int),
                "/" => {
                    if b == 0 {
                        return Err("division by zero".into());
                    }
                    Float(a as f64 / b as f64)
                }
                "//" => {
                    if b == 0 {
                        return Err("integer division or modulo by zero".into());
                    }
                    Int(a.div_euclid(b) - i64::from(b < 0 && a.rem_euclid(b) != 0))
                }
                "%" => {
                    if b == 0 {
                        return Err("integer division or modulo by zero".into());
                    }
                    let m = a % b;
                    Int(if m != 0 && ((m < 0) != (b < 0)) {
                        m + b
                    } else {
                        m
                    })
                }
                "**" => {
                    if b >= 0 {
                        a.checked_pow(b.min(u32::MAX as i64) as u32)
                            .map_or(Float((a as f64).powf(b as f64)), Int)
                    } else {
                        Float((a as f64).powf(b as f64))
                    }
                }
                "&" => Int(a & b),
                "|" => Int(a | b),
                "^" => Int(a ^ b),
                "<<" => Int(a << b.clamp(0, 62)),
                ">>" => Int(a >> b.clamp(0, 62)),
                _ => return Err(bad()),
            });
        }
        let (Some(a), Some(b)) = (l.as_f64(), r.as_f64()) else {
            return Err(bad());
        };
        Ok(Float(match op {
            "+" => a + b,
            "-" => a - b,
            "*" => a * b,
            "/" => {
                if b == 0.0 {
                    return Err("float division by zero".into());
                }
                a / b
            }
            "//" => {
                if b == 0.0 {
                    return Err("float floor division by zero".into());
                }
                (a / b).floor()
            }
            "%" => {
                if b == 0.0 {
                    return Err("float modulo by zero".into());
                }
                let m = a % b;
                if m != 0.0 && ((m < 0.0) != (b < 0.0)) {
                    m + b
                } else {
                    m
                }
            }
            "**" => a.powf(b),
            _ => return Err(bad()),
        }))
    }
}

fn slice(b: &Py, lo: Option<i64>, hi: Option<i64>, step: i64) -> Res<Py> {
    if step == 0 {
        return Err("slice step cannot be zero".into());
    }
    let items: Vec<Py> = match b {
        Py::Str(s) => s.chars().map(|c| Py::Str(c.to_string())).collect(),
        Py::List(v) | Py::Tuple(v) => v.clone(),
        other => {
            return Err(format!(
                "'{}' object is not subscriptable",
                other.type_name()
            ));
        }
    };
    let n = items.len() as i64;
    let norm = |i: i64, default: i64| -> i64 {
        let i = if i < 0 { i + n } else { i };
        i.clamp(
            if step > 0 { 0 } else { -1 },
            if step > 0 { n } else { n - 1 },
        )
        .max(default.min(i))
    };
    let _ = norm;
    let (start, stop) = if step > 0 {
        let s = lo.map_or(0, |i| (if i < 0 { i + n } else { i }).clamp(0, n));
        let e = hi.map_or(n, |i| (if i < 0 { i + n } else { i }).clamp(0, n));
        (s, e)
    } else {
        let s = lo.map_or(n - 1, |i| (if i < 0 { i + n } else { i }).clamp(-1, n - 1));
        let e = hi.map_or(-1, |i| (if i < 0 { i + n } else { i }).clamp(-1, n - 1));
        (s, e)
    };
    let mut out = Vec::new();
    let mut i = start;
    while (step > 0 && i < stop) || (step < 0 && i > stop) {
        out.push(items[i as usize].clone());
        i += step;
    }
    Ok(match b {
        Py::Str(_) => Py::Str(out.iter().map(str_of).collect()),
        Py::Tuple(_) => Py::Tuple(out),
        _ => Py::List(out),
    })
}

fn math_attr(name: &str) -> Res<Py> {
    Ok(match name {
        "pi" => Py::Float(std::f64::consts::PI),
        "e" => Py::Float(std::f64::consts::E),
        "tau" => Py::Float(std::f64::consts::TAU),
        "inf" => Py::Float(f64::INFINITY),
        "sqrt" | "floor" | "ceil" | "pow" | "log" | "log10" | "log2" | "exp" | "sin" | "cos"
        | "tan" | "asin" | "acos" | "atan" | "atan2" | "fabs" | "radians" | "degrees" | "hypot"
        | "trunc" | "fmod" | "isnan" | "isinf" | "copysign" | "sinh" | "cosh" | "tanh" => {
            Py::Builtin(format!("math.{name}"))
        }
        other => return Err(format!("module 'math' has no attribute '{other}'")),
    })
}

fn num_arg(args: &[Py], i: usize, who: &str) -> Res<f64> {
    args.get(i)
        .and_then(Py::as_f64)
        .ok_or_else(|| format!("{who}() needs a number"))
}

fn round_half_even(x: f64) -> f64 {
    x.round_ties_even()
}

fn call_builtin(name: &str, args: Vec<Py>, kwargs: &[(String, Py)]) -> Res<Py> {
    if let Some(m) = name.strip_prefix("math.") {
        let x = num_arg(&args, 0, m)?;
        let float = |v: f64| Ok(Py::Float(v));
        return match m {
            "sqrt" => {
                if x < 0.0 {
                    Err("math domain error".into())
                } else {
                    float(x.sqrt())
                }
            }
            "floor" => Ok(Py::Int(x.floor() as i64)),
            "ceil" => Ok(Py::Int(x.ceil() as i64)),
            "trunc" => Ok(Py::Int(x.trunc() as i64)),
            "pow" => float(x.powf(num_arg(&args, 1, m)?)),
            "log" => {
                if x <= 0.0 {
                    return Err("math domain error".into());
                }
                match args.get(1) {
                    Some(b) => float(x.ln() / b.as_f64().ok_or("log() base needs a number")?.ln()),
                    None => float(x.ln()),
                }
            }
            "log10" => {
                if x <= 0.0 {
                    Err("math domain error".into())
                } else {
                    float(x.log10())
                }
            }
            "log2" => {
                if x <= 0.0 {
                    Err("math domain error".into())
                } else {
                    float(x.log2())
                }
            }
            "exp" => float(x.exp()),
            "sin" => float(x.sin()),
            "cos" => float(x.cos()),
            "tan" => float(x.tan()),
            "asin" => float(x.asin()),
            "acos" => float(x.acos()),
            "atan" => float(x.atan()),
            "sinh" => float(x.sinh()),
            "cosh" => float(x.cosh()),
            "tanh" => float(x.tanh()),
            "atan2" => float(x.atan2(num_arg(&args, 1, m)?)),
            "fabs" => float(x.abs()),
            "radians" => float(x.to_radians()),
            "degrees" => float(x.to_degrees()),
            "hypot" => float(x.hypot(num_arg(&args, 1, m)?)),
            "fmod" => float(x % num_arg(&args, 1, m)?),
            "copysign" => float(x.copysign(num_arg(&args, 1, m)?)),
            "isnan" => Ok(Py::Bool(x.is_nan())),
            "isinf" => Ok(Py::Bool(x.is_infinite())),
            _ => Err(format!("math.{m} is not supported")),
        };
    }
    let one = |who: &str| {
        args.first()
            .ok_or_else(|| format!("{who}() needs an argument"))
    };
    match name {
        "abs" => match one(name)? {
            Py::Int(n) => Ok(Py::Int(n.abs())),
            Py::Float(f) => Ok(Py::Float(f.abs())),
            Py::Bool(b) => Ok(Py::Int(i64::from(*b))),
            other => Err(format!(
                "bad operand type for abs(): '{}'",
                other.type_name()
            )),
        },
        "float" => match one(name)? {
            Py::Str(s) => s
                .trim()
                .parse::<f64>()
                .map(Py::Float)
                .map_err(|_| format!("could not convert string to float: {s:?}")),
            v => v
                .as_f64()
                .map(Py::Float)
                .ok_or_else(|| "float() argument must be a number".into()),
        },
        "int" => match one(name)? {
            Py::Str(s) => {
                let base = args.get(1).and_then(Py::as_int).unwrap_or(10) as u32;
                i64::from_str_radix(s.trim(), base)
                    .map(Py::Int)
                    .map_err(|_| format!("invalid literal for int(): {s:?}"))
            }
            Py::Float(f) => {
                if f.is_finite() {
                    Ok(Py::Int(f.trunc() as i64))
                } else {
                    Err("cannot convert float to integer".into())
                }
            }
            v => v
                .as_int()
                .map(Py::Int)
                .ok_or_else(|| "int() argument must be a number".into()),
        },
        "str" => Ok(Py::Str(args.first().map_or(String::new(), str_of))),
        "repr" => Ok(Py::Str(repr_of(one(name)?))),
        "bool" => Ok(Py::Bool(args.first().is_some_and(Py::truthy))),
        "len" => match one(name)? {
            Py::Str(s) => Ok(Py::Int(s.chars().count() as i64)),
            Py::List(v) | Py::Tuple(v) => Ok(Py::Int(v.len() as i64)),
            other => Err(format!(
                "object of type '{}' has no len()",
                other.type_name()
            )),
        },
        "round" => {
            let ndigits = args
                .get(1)
                .or_else(|| kwargs.iter().find(|(k, _)| k == "ndigits").map(|(_, v)| v));
            match (one(name)?, ndigits) {
                (Py::Int(n), None) => Ok(Py::Int(*n)),
                (v, None) => {
                    let f = v.as_f64().ok_or("round() needs a number")?;
                    Ok(Py::Int(round_half_even(f) as i64))
                }
                (v, Some(d)) => {
                    let f = v.as_f64().ok_or("round() needs a number")?;
                    let d = d.as_int().ok_or("ndigits must be an integer")?;
                    if d >= 0 {
                        // Correctly rounded decimal (ties to even on the exact value).
                        let s = format!("{:.*}", d as usize, f);
                        let r = s.parse::<f64>().map_err(|_| "round failed")?;
                        Ok(if matches!(v, Py::Int(_)) {
                            Py::Int(f as i64)
                        } else {
                            Py::Float(r)
                        })
                    } else {
                        let p = 10f64.powi((-d) as i32);
                        let r = round_half_even(f / p) * p;
                        Ok(if matches!(v, Py::Int(_)) {
                            Py::Int(r as i64)
                        } else {
                            Py::Float(r)
                        })
                    }
                }
            }
        }
        "max" | "min" => {
            let items = if args.len() == 1 {
                match &args[0] {
                    Py::List(v) | Py::Tuple(v) => v.clone(),
                    other => return Err(format!("'{}' object is not iterable", other.type_name())),
                }
            } else {
                args.clone()
            };
            let mut best = items
                .first()
                .cloned()
                .ok_or(format!("{name}() of an empty sequence"))?;
            for x in &items[1..] {
                let o = py_cmp(x, &best)?;
                if (name == "max" && o.is_gt()) || (name == "min" && o.is_lt()) {
                    best = x.clone();
                }
            }
            Ok(best)
        }
        "sum" => {
            let items = match one(name)? {
                Py::List(v) | Py::Tuple(v) => v.clone(),
                _ => return Err("sum() needs a list".into()),
            };
            let interp = Interp {
                globals: HashMap::new(),
            };
            let mut acc = args.get(1).cloned().unwrap_or(Py::Int(0));
            for x in &items {
                acc = interp.binop(&acc, "+", x)?;
            }
            Ok(acc)
        }
        "pow" => {
            let interp = Interp {
                globals: HashMap::new(),
            };
            interp.binop(
                one(name)?,
                "**",
                args.get(1).ok_or("pow() needs two arguments")?,
            )
        }
        "divmod" => {
            let interp = Interp {
                globals: HashMap::new(),
            };
            let (a, b) = (
                one(name)?,
                args.get(1).ok_or("divmod() needs two arguments")?,
            );
            Ok(Py::Tuple(vec![
                interp.binop(a, "//", b)?,
                interp.binop(a, "%", b)?,
            ]))
        }
        "range" => {
            let ints: Vec<i64> = args.iter().filter_map(Py::as_int).collect();
            let (start, stop, step) = match ints.as_slice() {
                [n] => (0, *n, 1),
                [a, b] => (*a, *b, 1),
                [a, b, c] => (*a, *b, *c),
                _ => return Err("range expected 1 to 3 integer arguments".into()),
            };
            if step == 0 {
                return Err("range() step must not be zero".into());
            }
            let mut out = Vec::new();
            let mut i = start;
            while (step > 0 && i < stop) || (step < 0 && i > stop) {
                out.push(Py::Int(i));
                i += step;
            }
            Ok(Py::List(out))
        }
        "list" | "tuple" => {
            let items = match args.first() {
                None => Vec::new(),
                Some(Py::Str(s)) => s.chars().map(|c| Py::Str(c.to_string())).collect(),
                Some(Py::List(v)) | Some(Py::Tuple(v)) => v.clone(),
                Some(other) => {
                    return Err(format!("'{}' object is not iterable", other.type_name()));
                }
            };
            Ok(if name == "list" {
                Py::List(items)
            } else {
                Py::Tuple(items)
            })
        }
        "sorted" => {
            let mut items = match one(name)? {
                Py::List(v) | Py::Tuple(v) => v.clone(),
                Py::Str(s) => s.chars().map(|c| Py::Str(c.to_string())).collect(),
                other => return Err(format!("'{}' object is not iterable", other.type_name())),
            };
            let mut err = None;
            items.sort_by(|a, b| {
                py_cmp(a, b).unwrap_or_else(|e| {
                    err = Some(e);
                    std::cmp::Ordering::Equal
                })
            });
            if kwargs.iter().any(|(k, v)| k == "reverse" && v.truthy()) {
                items.reverse();
            }
            err.map_or(Ok(Py::List(items)), Err)
        }
        other => Err(format!("{other}() is not supported")),
    }
}

fn call_method(recv: &Py, name: &str, args: Vec<Py>) -> Res<Py> {
    let s = match recv {
        Py::Str(s) => s,
        Py::List(items) => {
            return match name {
                "index" => items
                    .iter()
                    .position(|x| py_eq(x, args.first().unwrap_or(&Py::None)))
                    .map(|i| Py::Int(i as i64))
                    .ok_or_else(|| "value is not in list".into()),
                "count" => Ok(Py::Int(
                    items
                        .iter()
                        .filter(|x| py_eq(x, args.first().unwrap_or(&Py::None)))
                        .count() as i64,
                )),
                other => Err(format!("'list' object has no attribute '{other}'")),
            };
        }
        other => {
            return Err(format!(
                "'{}' object has no attribute '{name}'",
                other.type_name()
            ));
        }
    };
    let arg_str = |i: usize| -> Res<String> {
        match args.get(i) {
            Some(Py::Str(a)) => Ok(a.clone()),
            _ => Err(format!("{name}() argument must be a string")),
        }
    };
    let chars_opt = |i: usize| -> Option<String> {
        match args.get(i) {
            Some(Py::Str(a)) => Some(a.clone()),
            _ => None,
        }
    };
    Ok(match name {
        "upper" => Py::Str(s.to_uppercase()),
        "lower" => Py::Str(s.to_lowercase()),
        "title" => Py::Str(
            s.split(' ')
                .map(|w| {
                    let mut c = w.chars();
                    c.next().map_or(String::new(), |f| {
                        f.to_uppercase().collect::<String>() + &c.as_str().to_lowercase()
                    })
                })
                .collect::<Vec<_>>()
                .join(" "),
        ),
        "capitalize" => {
            let mut c = s.chars();
            Py::Str(c.next().map_or(String::new(), |f| {
                f.to_uppercase().collect::<String>() + &c.as_str().to_lowercase()
            }))
        }
        "swapcase" => Py::Str(
            s.chars()
                .map(|c| {
                    if c.is_uppercase() {
                        c.to_lowercase().collect::<String>()
                    } else {
                        c.to_uppercase().collect::<String>()
                    }
                })
                .collect(),
        ),
        "strip" | "lstrip" | "rstrip" => {
            let set = chars_opt(0);
            let f = |c: char| match &set {
                Some(t) => t.contains(c),
                None => c.is_whitespace(),
            };
            Py::Str(match name {
                "strip" => s.trim_matches(f).to_owned(),
                "lstrip" => s.trim_start_matches(f).to_owned(),
                _ => s.trim_end_matches(f).to_owned(),
            })
        }
        "replace" => {
            let (old, new) = (arg_str(0)?, arg_str(1)?);
            match args.get(2).and_then(Py::as_int) {
                Some(n) if n >= 0 => Py::Str(s.replacen(&old, &new, n as usize)),
                _ => Py::Str(s.replace(&old, &new)),
            }
        }
        "startswith" => Py::Bool(s.starts_with(&arg_str(0)?)),
        "endswith" => Py::Bool(s.ends_with(&arg_str(0)?)),
        "find" => {
            let sub = arg_str(0)?;
            Py::Int(s.find(&sub).map_or(-1, |b| s[..b].chars().count() as i64))
        }
        "index" => {
            let sub = arg_str(0)?;
            s.find(&sub)
                .map(|b| Py::Int(s[..b].chars().count() as i64))
                .ok_or("substring not found")?
        }
        "rfind" => {
            let sub = arg_str(0)?;
            Py::Int(s.rfind(&sub).map_or(-1, |b| s[..b].chars().count() as i64))
        }
        "count" => {
            let sub = arg_str(0)?;
            Py::Int(if sub.is_empty() {
                s.chars().count() as i64 + 1
            } else {
                s.matches(&sub).count() as i64
            })
        }
        "split" => {
            let parts: Vec<Py> = match chars_opt(0) {
                Some(sep) => s
                    .split(sep.as_str())
                    .map(|p| Py::Str(p.to_owned()))
                    .collect(),
                None => s
                    .split_whitespace()
                    .map(|p| Py::Str(p.to_owned()))
                    .collect(),
            };
            Py::List(parts)
        }
        "join" => match args.first() {
            Some(Py::List(v)) | Some(Py::Tuple(v)) => Py::Str(
                v.iter()
                    .map(|x| match x {
                        Py::Str(t) => Ok(t.clone()),
                        _ => Err("join() needs strings".to_owned()),
                    })
                    .collect::<Res<Vec<_>>>()?
                    .join(s),
            ),
            _ => return Err("join() needs a list".into()),
        },
        "zfill" => {
            let w = args.first().and_then(Py::as_int).unwrap_or(0).max(0) as usize;
            let (sign, digits) = match s.strip_prefix(['-', '+']) {
                Some(rest) => (&s[..1], rest),
                None => ("", s.as_str()),
            };
            Py::Str(format!(
                "{sign}{}{digits}",
                "0".repeat(w.saturating_sub(s.chars().count()))
            ))
        }
        "ljust" | "rjust" | "center" => {
            let w = args.first().and_then(Py::as_int).unwrap_or(0).max(0) as usize;
            let fill = chars_opt(1).and_then(|f| f.chars().next()).unwrap_or(' ');
            let pad = w.saturating_sub(s.chars().count());
            let rep = |n: usize| fill.to_string().repeat(n);
            Py::Str(match name {
                "ljust" => format!("{s}{}", rep(pad)),
                "rjust" => format!("{}{s}", rep(pad)),
                _ => format!("{}{s}{}", rep(pad / 2), rep(pad - pad / 2)),
            })
        }
        "isdigit" => Py::Bool(!s.is_empty() && s.chars().all(|c| c.is_ascii_digit())),
        "isalpha" => Py::Bool(!s.is_empty() && s.chars().all(char::is_alphabetic)),
        "isalnum" => Py::Bool(!s.is_empty() && s.chars().all(char::is_alphanumeric)),
        "isspace" => Py::Bool(!s.is_empty() && s.chars().all(char::is_whitespace)),
        "isupper" => {
            Py::Bool(s.chars().any(char::is_alphabetic) && !s.chars().any(char::is_lowercase))
        }
        "islower" => {
            Py::Bool(s.chars().any(char::is_alphabetic) && !s.chars().any(char::is_uppercase))
        }
        "format" => {
            // `{}` / `{0}` placeholders with optional `:spec`.
            let mut out = String::new();
            let chars: Vec<char> = s.chars().collect();
            let (mut i, mut auto) = (0, 0);
            while i < chars.len() {
                match chars[i] {
                    '{' if chars.get(i + 1) == Some(&'{') => {
                        out.push('{');
                        i += 2;
                    }
                    '}' if chars.get(i + 1) == Some(&'}') => {
                        out.push('}');
                        i += 2;
                    }
                    '{' => {
                        let end = chars[i..]
                            .iter()
                            .position(|c| *c == '}')
                            .ok_or("unbalanced braces")?
                            + i;
                        let inner: String = chars[i + 1..end].iter().collect();
                        let (field, spec) = inner.split_once(':').unwrap_or((inner.as_str(), ""));
                        let idx = if field.is_empty() {
                            auto += 1;
                            auto - 1
                        } else {
                            field
                                .parse::<usize>()
                                .map_err(|_| "named format fields are not supported")?
                        };
                        let v = args.get(idx).ok_or("not enough arguments for format()")?;
                        out.push_str(&format_value(v, spec)?);
                        i = end + 1;
                    }
                    c => {
                        out.push(c);
                        i += 1;
                    }
                }
            }
            Py::Str(out)
        }
        other => return Err(format!("'str' object has no attribute '{other}'")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(expr: &str) -> String {
        let it = Interp::new("").unwrap();
        str_of(&it.eval(expr, &HashMap::new()).unwrap())
    }

    #[test]
    fn arithmetic_follows_python() {
        assert_eq!(run("7 / 2"), "3.5");
        assert_eq!(run("7 // 2"), "3");
        assert_eq!(run("-7 // 2"), "-4");
        assert_eq!(run("-7 % 3"), "2");
        assert_eq!(run("2 ** 10"), "1024");
        assert_eq!(run("2 ** -1"), "0.5");
        assert_eq!(run("round(2.5)"), "2");
        assert_eq!(run("round(3.5)"), "4");
        assert_eq!(run("round(1.2345, 2)"), "1.23");
        assert_eq!(run("1 if 3 > 2 else 0"), "1");
        assert_eq!(run("10 / 4 * 2"), "5.0");
        assert_eq!(run("1e3"), "1000.0");
    }

    #[test]
    fn strings_and_formatting() {
        assert_eq!(run("'Lot 1'.upper()"), "LOT 1");
        assert_eq!(run("'Lot 12'[4:]"), "12");
        assert_eq!(run("'abc'[::-1]"), "cba");
        assert_eq!(run("'%s-%d' % ('Lot', 3)"), "Lot-3");
        assert_eq!(run("'%.2f' % 3.14159"), "3.14");
        assert_eq!(run("'%05d' % 42"), "00042");
        assert_eq!(run("f'{3.14159:.2f}|{42:>5}|{7:03d}'"), "3.14|   42|007");
        assert_eq!(run("'{} and {:.1f}'.format('a', 2.25)"), "a and 2.2");
        assert_eq!(run("len('hello') + 1"), "6");
        assert_eq!(run("'a,b,c'.split(',')"), "['a', 'b', 'c']");
        assert_eq!(run("str(8597.94) + 'x'"), "8597.94x");
        assert_eq!(run("f'{1234567:,}'"), "1,234,567");
    }

    #[test]
    fn math_and_builtins() {
        assert_eq!(run("math.sqrt(16)"), "4.0");
        assert_eq!(run("math.floor(-2.5)"), "-3");
        assert_eq!(run("max(3, 9, 2)"), "9");
        assert_eq!(run("int(3.9) + int('4')"), "7");
        assert_eq!(run("abs(-2)"), "2");
        assert_eq!(run("sum([1, 2, 3])"), "6");
    }

    #[test]
    fn code_blocks() {
        let it = Interp::new(
            "def f(z):\n    if z > 2:\n        return 10\n    elif z == 2:\n        return 5\n    r = 0\n    for i in range(z):\n        r += i\n    return r\n",
        )
        .unwrap();
        let v = |n: i64| {
            let mut m = HashMap::new();
            m.insert("Z".to_owned(), Py::Int(n));
            str_of(&it.eval("f(!Z!)", &m).unwrap())
        };
        assert_eq!(v(3), "10");
        assert_eq!(v(2), "5");
        assert_eq!(v(4 - 3), "0");
    }

    #[test]
    fn errors_are_reported() {
        let it = Interp::new("").unwrap();
        let e = |s: &str| it.eval(s, &HashMap::new()).unwrap_err();
        assert!(e("1 / 0").contains("division"));
        assert!(e("'a' + 1").contains("unsupported"));
        assert!(e("nosuch(1)").contains("not defined"));
    }
}
