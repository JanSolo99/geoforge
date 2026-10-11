//! A read-only reader for SQLite database files, written from the public file
//! format description (<https://www.sqlite.org/fileformat2.html>): the header,
//! table b-trees with overflow pages, the record format and the schema table.
//! Indexes, the write-ahead log and writing are not supported.

use std::path::{Path, PathBuf};

use geoforge_core::{Error, Result};

fn bad(message: impl Into<String>) -> Error {
    Error::format("SQLite", message)
}

/// One stored value.
#[derive(Debug, Clone, PartialEq)]
pub enum SqlValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

impl SqlValue {
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            SqlValue::Integer(v) => Some(*v),
            SqlValue::Real(v) => Some(*v as i64),
            SqlValue::Text(t) => t.trim().parse().ok(),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            SqlValue::Integer(v) => Some(*v as f64),
            SqlValue::Real(v) => Some(*v),
            SqlValue::Text(t) => t.trim().parse().ok(),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            SqlValue::Text(t) => Some(t),
            _ => None,
        }
    }
}

/// A row: its rowid and values.
pub type Row = (i64, Vec<SqlValue>);

/// A column of a table as declared in its CREATE TABLE statement.
#[derive(Debug, Clone, PartialEq)]
pub struct Column {
    pub name: String,
    /// The declared type as written (upper-cased), e.g. `TEXT(20)`; empty when
    /// none is given.
    pub declared_type: String,
    pub not_null: bool,
    /// An INTEGER PRIMARY KEY column, which aliases the rowid.
    pub rowid_alias: bool,
}

impl Column {
    /// REAL type affinity (the declared type mentions REAL, FLOA or DOUB and not
    /// INT, CHAR, CLOB, TEXT or BLOB).
    pub fn real_affinity(&self) -> bool {
        let t = &self.declared_type;
        !["INT", "CHAR", "CLOB", "TEXT", "BLOB"]
            .iter()
            .any(|k| t.contains(k))
            && ["REAL", "FLOA", "DOUB"].iter().any(|k| t.contains(k))
    }
}

/// A table from the schema.
#[derive(Debug, Clone)]
pub struct Table {
    pub name: String,
    pub root_page: u32,
    pub columns: Vec<Column>,
    pub sql: String,
}

/// An open database file, held in memory.
pub struct Database {
    path: PathBuf,
    data: Vec<u8>,
    page_size: usize,
    usable: usize,
    tables: Vec<Table>,
}

impl Database {
    pub fn open(path: &Path) -> Result<Database> {
        let data = std::fs::read(path).map_err(|e| Error::io(path, e))?;
        if data.len() < 100 || &data[..16] != b"SQLite format 3\0" {
            return Err(bad(format!("{}: not an SQLite 3 database", path.display())));
        }
        let wal = PathBuf::from(format!("{}-wal", path.display()));
        if std::fs::metadata(&wal).is_ok_and(|m| m.len() > 0) {
            return Err(Error::Unsupported(format!(
                "{}: uncommitted write-ahead log (checkpoint it first)",
                path.display()
            )));
        }
        let page_size = match u16::from_be_bytes([data[16], data[17]]) {
            1 => 65536,
            n => n as usize,
        };
        if page_size < 512 || !page_size.is_power_of_two() {
            return Err(bad(format!("page size {page_size}")));
        }
        let usable = page_size - data[20] as usize;
        let encoding = u32::from_be_bytes([data[56], data[57], data[58], data[59]]);
        if encoding > 1 {
            return Err(Error::Unsupported(format!(
                "{}: UTF-16 text encoding",
                path.display()
            )));
        }
        let mut db = Database {
            path: path.to_path_buf(),
            data,
            page_size,
            usable,
            tables: Vec::new(),
        };
        let mut tables = Vec::new();
        for (_, values) in db.rows(1)? {
            let text = |i: usize| values.get(i).and_then(|v| v.as_str()).unwrap_or("");
            if text(0) != "table" {
                continue;
            }
            let sql = text(4).to_string();
            tables.push(Table {
                name: text(1).to_string(),
                root_page: values.get(3).and_then(SqlValue::as_i64).unwrap_or(0) as u32,
                columns: parse_columns(&sql),
                sql,
            });
        }
        db.tables = tables;
        Ok(db)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn tables(&self) -> &[Table] {
        &self.tables
    }

    /// A table by name, ignoring ASCII case as SQL does.
    pub fn table(&self, name: &str) -> Option<&Table> {
        self.tables
            .iter()
            .find(|t| t.name.eq_ignore_ascii_case(name))
    }

    /// All rows of a table in rowid order, one value per declared column (the
    /// rowid alias filled in, missing trailing columns as NULL).
    pub fn select(&self, table: &Table) -> Result<Vec<Row>> {
        let mut out = Vec::new();
        for (rowid, mut values) in self.rows(table.root_page)? {
            values.resize(table.columns.len().max(values.len()), SqlValue::Null);
            for (i, c) in table.columns.iter().enumerate() {
                if c.rowid_alias {
                    values[i] = SqlValue::Integer(rowid);
                } else if let SqlValue::Integer(v) = values[i]
                    && c.real_affinity()
                {
                    // SQLite stores whole REAL values as integers.
                    values[i] = SqlValue::Real(v as f64);
                }
            }
            values.truncate(table.columns.len());
            out.push((rowid, values));
        }
        Ok(out)
    }

    /// Rows of the table b-tree rooted at `root`, in key order.
    fn rows(&self, root: u32) -> Result<Vec<Row>> {
        let mut out = Vec::new();
        let mut stack = vec![root];
        let mut visited = 0usize;
        while let Some(page) = stack.pop() {
            visited += 1;
            if visited > self.data.len() / self.page_size + 1 {
                return Err(bad("b-tree loop"));
            }
            let (bytes, header_at) = self.page(page)?;
            let kind = bytes[header_at];
            let cells = u16::from_be_bytes([bytes[header_at + 3], bytes[header_at + 4]]) as usize;
            match kind {
                0x05 => {
                    // Interior table page: children in key order, right-most last.
                    let right = be32(bytes, header_at + 8)?;
                    let mut children = Vec::with_capacity(cells + 1);
                    for k in 0..cells {
                        let ptr = be16(bytes, header_at + 12 + 2 * k)?;
                        children.push(be32(bytes, ptr)?);
                    }
                    children.push(right);
                    // Popped from the end, so push in reverse.
                    stack.extend(children.into_iter().rev());
                }
                0x0d => {
                    for k in 0..cells {
                        let ptr = be16(bytes, header_at + 8 + 2 * k)?;
                        out.push(self.leaf_cell(bytes, ptr)?);
                    }
                }
                other => return Err(bad(format!("page {page}: b-tree page type {other:#x}"))),
            }
        }
        Ok(out)
    }

    /// The bytes of page `n` (1-based) and where its b-tree header starts.
    fn page(&self, n: u32) -> Result<(&[u8], usize)> {
        let start = (n as usize).checked_sub(1).ok_or_else(|| bad("page 0"))? * self.page_size;
        let bytes = self
            .data
            .get(start..start + self.page_size)
            .ok_or_else(|| bad(format!("page {n} past the end of the file")))?;
        Ok((bytes, if n == 1 { 100 } else { 0 }))
    }

    fn leaf_cell(&self, page: &[u8], at: usize) -> Result<Row> {
        let (size, n1) = varint(page, at)?;
        let (rowid, n2) = varint(page, at + n1)?;
        let start = at + n1 + n2;
        let size = size as usize;
        let u = self.usable;
        let x = u - 35;
        let payload = if size <= x {
            page.get(start..start + size)
                .ok_or_else(|| bad("cell past the page"))?
                .to_vec()
        } else {
            let m = (u - 12) * 32 / 255 - 23;
            let k = m + (size - m) % (u - 4);
            let local = if k <= x { k } else { m };
            let mut payload = page
                .get(start..start + local)
                .ok_or_else(|| bad("cell past the page"))?
                .to_vec();
            let mut next = be32(page, start + local)?;
            while payload.len() < size {
                if next == 0 {
                    return Err(bad("overflow chain ends early"));
                }
                let (bytes, _) = self.page(next)?;
                next = be32(bytes, 0)?;
                let take = (size - payload.len()).min(u - 4);
                payload.extend_from_slice(&bytes[4..4 + take]);
            }
            payload
        };
        Ok((rowid as i64, record(&payload)?))
    }
}

fn be16(b: &[u8], at: usize) -> Result<usize> {
    b.get(at..at + 2)
        .map(|s| u16::from_be_bytes([s[0], s[1]]) as usize)
        .ok_or_else(|| bad("short page"))
}

fn be32(b: &[u8], at: usize) -> Result<u32> {
    b.get(at..at + 4)
        .map(|s| u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
        .ok_or_else(|| bad("short page"))
}

/// A SQLite varint: big-endian groups of 7 bits, the ninth byte whole.
pub(crate) fn varint(b: &[u8], at: usize) -> Result<(u64, usize)> {
    let mut v = 0u64;
    for i in 0..9 {
        let byte = *b.get(at + i).ok_or_else(|| bad("short varint"))?;
        if i == 8 {
            return Ok(((v << 8) | byte as u64, 9));
        }
        v = (v << 7) | (byte & 0x7f) as u64;
        if byte & 0x80 == 0 {
            return Ok((v, i + 1));
        }
    }
    unreachable!()
}

/// Decodes a record: a header of serial types, then the values.
fn record(p: &[u8]) -> Result<Vec<SqlValue>> {
    let (header_len, mut at) = varint(p, 0)?;
    let header_len = header_len as usize;
    let mut types = Vec::new();
    while at < header_len {
        let (t, n) = varint(p, at)?;
        types.push(t);
        at += n;
    }
    let mut body = header_len;
    let mut out = Vec::with_capacity(types.len());
    for t in types {
        let int = |len: usize| -> Result<i64> {
            let s = p.get(body..body + len).ok_or_else(|| bad("short record"))?;
            // Sign-extend from the first byte.
            let mut v = (s[0] as i8) as i64;
            for &b in &s[1..] {
                v = (v << 8) | b as i64;
            }
            Ok(v)
        };
        let (value, len) = match t {
            0 => (SqlValue::Null, 0),
            1..=4 => (SqlValue::Integer(int(t as usize)?), t as usize),
            5 => (SqlValue::Integer(int(6)?), 6),
            6 => (SqlValue::Integer(int(8)?), 8),
            7 => {
                let bits = int(8)? as u64;
                (SqlValue::Real(f64::from_bits(bits)), 8)
            }
            8 => (SqlValue::Integer(0), 0),
            9 => (SqlValue::Integer(1), 0),
            10 | 11 => return Err(bad(format!("reserved serial type {t}"))),
            n => {
                let len = ((n - 12) / 2) as usize;
                let s = p.get(body..body + len).ok_or_else(|| bad("short record"))?;
                if n % 2 == 0 {
                    (SqlValue::Blob(s.to_vec()), len)
                } else {
                    (SqlValue::Text(String::from_utf8_lossy(s).into_owned()), len)
                }
            }
        };
        out.push(value);
        body += len;
    }
    Ok(out)
}

/// Splits `text` at top-level commas (outside parentheses and quotes).
fn split_top(text: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let mut cur = String::new();
    for c in text.chars() {
        match quote {
            Some(q) => {
                if c == q || (q == '[' && c == ']') {
                    quote = None;
                }
                cur.push(c);
            }
            None => match c {
                '"' | '\'' | '`' | '[' => {
                    quote = Some(c);
                    cur.push(c);
                }
                '(' => {
                    depth += 1;
                    cur.push(c);
                }
                ')' => {
                    depth -= 1;
                    cur.push(c);
                }
                ',' if depth == 0 => parts.push(std::mem::take(&mut cur)),
                _ => cur.push(c),
            },
        }
    }
    if !cur.trim().is_empty() {
        parts.push(cur);
    }
    parts
}

/// The first identifier of `text` (quoted or bare) and the rest.
fn identifier(text: &str) -> (String, &str) {
    let t = text.trim_start();
    let close = match t.chars().next() {
        Some('"') => Some('"'),
        Some('`') => Some('`'),
        Some('[') => Some(']'),
        Some('\'') => Some('\''),
        _ => None,
    };
    match close {
        Some(q) => {
            let body = &t[1..];
            // A doubled quote stands for itself.
            let mut name = String::new();
            let mut chars = body.char_indices().peekable();
            while let Some((i, c)) = chars.next() {
                if c == q {
                    if q != ']' && chars.peek().is_some_and(|&(_, n)| n == q) {
                        name.push(q);
                        chars.next();
                        continue;
                    }
                    return (name, &body[i + 1..]);
                }
                name.push(c);
            }
            (name, "")
        }
        None => {
            let end = t
                .find(|c: char| c.is_whitespace() || c == '(')
                .unwrap_or(t.len());
            (t[..end].to_string(), &t[end..])
        }
    }
}

/// Column definitions from a CREATE TABLE statement.
fn parse_columns(sql: &str) -> Vec<Column> {
    let (Some(open), Some(close)) = (sql.find('('), sql.rfind(')')) else {
        return Vec::new();
    };
    let mut columns = Vec::new();
    for def in split_top(&sql[open + 1..close]) {
        let head = def.trim_start().to_ascii_uppercase();
        if ["CONSTRAINT", "PRIMARY", "UNIQUE", "CHECK", "FOREIGN"]
            .iter()
            .any(|k| head.starts_with(k) && head[k.len()..].starts_with([' ', '(', '\t', '\n']))
        {
            continue;
        }
        let (name, rest) = identifier(&def);
        let upper = rest.to_ascii_uppercase();
        // The type runs up to the first constraint keyword.
        let keywords = [
            " CONSTRAINT",
            " PRIMARY",
            " NOT",
            " NULL",
            " UNIQUE",
            " CHECK",
            " DEFAULT",
            " COLLATE",
            " REFERENCES",
            " GENERATED",
            " AS ",
        ];
        let padded = format!(" {}", upper.trim());
        let end = keywords
            .iter()
            .filter_map(|k| padded.find(k))
            .min()
            .unwrap_or(padded.len());
        let declared_type = padded[..end].trim().to_string();
        let words: Vec<&str> = upper.split_whitespace().collect();
        let primary = words.windows(2).any(|w| w == ["PRIMARY", "KEY"]);
        columns.push(Column {
            name,
            rowid_alias: primary && declared_type == "INTEGER",
            not_null: words.windows(2).any(|w| w == ["NOT", "NULL"]),
            declared_type,
        });
    }
    columns
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_from_create_table() {
        let cols = parse_columns(
            "CREATE TABLE \"my t\" ( \"fid\" INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, \
             \"geom\" POINT, [na,me] TEXT(20) DEFAULT 'a,b', n MEDIUMINT, x, \
             CONSTRAINT u UNIQUE (n))",
        );
        let names: Vec<_> = cols.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["fid", "geom", "na,me", "n", "x"]);
        assert!(cols[0].rowid_alias && cols[0].not_null);
        assert_eq!(cols[2].declared_type, "TEXT(20)");
        assert_eq!(cols[4].declared_type, "");
    }

    #[test]
    fn varints() {
        assert_eq!(varint(&[0x7f], 0).unwrap(), (127, 1));
        assert_eq!(varint(&[0x81, 0x00], 0).unwrap(), (128, 2));
        assert_eq!(varint(&[0xff; 9], 0).unwrap(), (u64::MAX, 9));
    }
}
