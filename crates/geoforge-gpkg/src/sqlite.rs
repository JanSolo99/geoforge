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

/// An entry of the schema table: a table, index, view or trigger.
#[derive(Debug, Clone)]
pub struct SchemaObject {
    /// `table`, `index`, `view` or `trigger`.
    pub kind: String,
    pub name: String,
    pub table: String,
    /// 0 for objects without a b-tree (views, triggers, virtual tables).
    pub root_page: u32,
    /// `None` for automatic indexes.
    pub sql: Option<String>,
}

/// An open database file, held in memory.
pub struct Database {
    path: PathBuf,
    data: Vec<u8>,
    page_size: usize,
    usable: usize,
    tables: Vec<Table>,
    objects: Vec<SchemaObject>,
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
            objects: Vec::new(),
        };
        let mut tables = Vec::new();
        let mut objects = Vec::new();
        for (_, values) in db.rows(1)? {
            let text = |i: usize| values.get(i).and_then(|v| v.as_str()).unwrap_or("");
            objects.push(SchemaObject {
                kind: text(0).to_string(),
                name: text(1).to_string(),
                table: text(2).to_string(),
                root_page: values.get(3).and_then(SqlValue::as_i64).unwrap_or(0) as u32,
                sql: values.get(4).and_then(|v| v.as_str()).map(str::to_string),
            });
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
        db.objects = objects;
        Ok(db)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Every schema entry, in schema order.
    pub fn objects(&self) -> &[SchemaObject] {
        &self.objects
    }

    /// The header's `application_id` and `user_version`.
    pub fn ids(&self) -> (u32, u32) {
        let be = |at: usize| u32::from_be_bytes(self.data[at..at + 4].try_into().unwrap());
        (be(68), be(60))
    }

    pub fn page_size(&self) -> usize {
        self.page_size
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
        let payload = self.payload(page, at + n1 + n2, size as usize, false)?;
        Ok((rowid as i64, record(&payload)?))
    }

    /// A cell's payload of `size` bytes starting at `start`, following its
    /// overflow chain.
    fn payload(&self, page: &[u8], start: usize, size: usize, index: bool) -> Result<Vec<u8>> {
        let u = self.usable;
        let x = if index {
            (u - 12) * 64 / 255 - 23
        } else {
            u - 35
        };
        if size <= x {
            return Ok(page
                .get(start..start + size)
                .ok_or_else(|| bad("cell past the page"))?
                .to_vec());
        }
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
        Ok(payload)
    }

    /// The entries of the index b-tree rooted at `root`, in key order: the
    /// indexed values and the rowid.
    pub fn index_entries(&self, root: u32) -> Result<Vec<(Vec<SqlValue>, i64)>> {
        let mut out = Vec::new();
        self.index_page(root, 0, &mut out)?;
        Ok(out)
    }

    fn index_page(
        &self,
        page: u32,
        depth: usize,
        out: &mut Vec<(Vec<SqlValue>, i64)>,
    ) -> Result<()> {
        if depth > 40 {
            return Err(bad("index b-tree too deep"));
        }
        let (bytes, h) = self.page(page)?;
        let kind = bytes[h];
        let cells = u16::from_be_bytes([bytes[h + 3], bytes[h + 4]]) as usize;
        let mut entry = |at: usize| -> Result<()> {
            let (size, n) = varint(bytes, at)?;
            let mut values = record(&self.payload(bytes, at + n, size as usize, true)?)?;
            let rowid = values.pop().and_then(|v| v.as_i64()).unwrap_or(0);
            out.push((values, rowid));
            Ok(())
        };
        match kind {
            0x0a => {
                for k in 0..cells {
                    entry(be16(bytes, h + 8 + 2 * k)?)?;
                }
            }
            0x02 => {
                for k in 0..cells {
                    let ptr = be16(bytes, h + 12 + 2 * k)?;
                    self.index_page(be32(bytes, ptr)?, depth + 1, out)?;
                    let (size, n) = varint(bytes, ptr + 4)?;
                    let mut values =
                        record(&self.payload(bytes, ptr + 4 + n, size as usize, true)?)?;
                    let rowid = values.pop().and_then(|v| v.as_i64()).unwrap_or(0);
                    out.push((values, rowid));
                }
                self.index_page(be32(bytes, h + 8)?, depth + 1, out)?;
            }
            other => return Err(bad(format!("page {page}: index page type {other:#x}"))),
        }
        Ok(())
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
pub fn parse_columns(sql: &str) -> Vec<Column> {
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

/// The columns of each automatic index SQLite creates for a table's PRIMARY
/// KEY (unless it aliases the rowid) and UNIQUE constraints, in its numbering
/// order (`sqlite_autoindex_<table>_1`, `_2`, ...): column constraints first,
/// then table constraints; a constraint repeating an earlier one's columns adds
/// no index.
pub fn autoindex_columns(sql: &str, columns: &[Column]) -> Vec<Vec<usize>> {
    let (Some(open), Some(close)) = (sql.find('('), sql.rfind(')')) else {
        return Vec::new();
    };
    let position = |name: &str| {
        columns
            .iter()
            .position(|c| c.name.eq_ignore_ascii_case(name))
    };
    let mut out: Vec<Vec<usize>> = Vec::new();
    let add = |cols: Vec<usize>, out: &mut Vec<Vec<usize>>| {
        if !cols.is_empty() && !out.contains(&cols) {
            out.push(cols);
        }
    };
    let mut table_constraints = Vec::new();
    let mut col = 0;
    for def in split_top(&sql[open + 1..close]) {
        let head = def.trim_start().to_ascii_uppercase();
        if ["CONSTRAINT", "PRIMARY", "UNIQUE", "CHECK", "FOREIGN"]
            .iter()
            .any(|k| head.starts_with(k) && head[k.len()..].starts_with([' ', '(', '\t', '\n']))
        {
            table_constraints.push(def);
            continue;
        }
        let (_, rest) = identifier(&def);
        let upper = rest.to_ascii_uppercase();
        let words: Vec<&str> = upper
            .split(|c: char| c.is_whitespace() || c == '(' || c == ')' || c == ',')
            .filter(|w| !w.is_empty())
            .collect();
        for (k, w) in words.iter().enumerate() {
            let primary = *w == "PRIMARY" && words.get(k + 1) == Some(&"KEY");
            if (primary && !columns[col].rowid_alias) || *w == "UNIQUE" {
                add(vec![col], &mut out);
            }
        }
        col += 1;
    }
    for def in table_constraints {
        let upper = def.to_ascii_uppercase();
        let kind_at = ["PRIMARY KEY", "UNIQUE"]
            .iter()
            .filter_map(|k| upper.find(k))
            .min();
        let Some(at) = kind_at else { continue };
        let Some(list_open) = def[at..].find('(') else {
            continue;
        };
        let list_close = def[at + list_open..]
            .find(')')
            .unwrap_or(def.len() - at - list_open);
        let list = &def[at + list_open + 1..at + list_open + list_close];
        let cols: Vec<usize> = split_top(list)
            .iter()
            .filter_map(|c| position(&identifier(c).0))
            .collect();
        let alias =
            upper[at..].starts_with("PRIMARY") && cols.len() == 1 && columns[cols[0]].rowid_alias;
        if !alias {
            add(cols, &mut out);
        }
    }
    out
}

/// The indexed columns of `CREATE [UNIQUE] INDEX name ON table (col, ...)`,
/// or `None` when it uses expressions, collations or descending order.
pub fn index_columns(sql: &str, columns: &[Column]) -> Option<Vec<usize>> {
    let open = sql.find('(')?;
    let close = sql.rfind(')')?;
    split_top(&sql[open + 1..close])
        .iter()
        .map(|c| {
            let (name, rest) = identifier(c);
            if !rest.trim().is_empty() && !rest.trim().eq_ignore_ascii_case("ASC") {
                return None;
            }
            columns
                .iter()
                .position(|col| col.name.eq_ignore_ascii_case(&name))
        })
        .collect()
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
    fn automatic_indexes() {
        let sql = "CREATE TABLE gpkg_metadata (id INTEGER CONSTRAINT m_pk PRIMARY KEY ASC \
                   AUTOINCREMENT NOT NULL UNIQUE,md_scope TEXT NOT NULL)";
        assert_eq!(autoindex_columns(sql, &parse_columns(sql)), vec![vec![0]]);
        let sql = "CREATE TABLE g (table_name TEXT NOT NULL,column_name TEXT NOT NULL,x INT,\
                   CONSTRAINT pk PRIMARY KEY (table_name, column_name),\
                   CONSTRAINT uk UNIQUE (table_name))";
        assert_eq!(
            autoindex_columns(sql, &parse_columns(sql)),
            vec![vec![0, 1], vec![0]]
        );
        let sql = "CREATE TABLE c (table_name TEXT NOT NULL PRIMARY KEY,identifier TEXT UNIQUE)";
        assert_eq!(
            autoindex_columns(sql, &parse_columns(sql)),
            vec![vec![0], vec![1]]
        );
    }

    #[test]
    fn varints() {
        assert_eq!(varint(&[0x7f], 0).unwrap(), (127, 1));
        assert_eq!(varint(&[0x81, 0x00], 0).unwrap(), (128, 2));
        assert_eq!(varint(&[0xff; 9], 0).unwrap(), (u64::MAX, 9));
    }
}
