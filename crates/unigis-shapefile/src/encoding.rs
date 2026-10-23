//! Text encoding of `.dbf` strings: `.cpg` file, then the header's language driver ID.

use encoding_rs::{Encoding, UTF_8, WINDOWS_1252};

/// How strings in a `.dbf` are decoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbfEncoding {
    Known(&'static Encoding),
    /// A DOS or Mac single-byte code page encoding_rs lacks: the characters of
    /// bytes 0x80-0xFF.
    SingleByte(u32, &'static [char; 128]),
    /// No `.cpg` and no language driver ID: UTF-8 when the bytes are valid UTF-8,
    /// otherwise Windows-1252 (ArcGIS's "ANSI" default on Western systems).
    Sniff,
}

impl DbfEncoding {
    pub fn decode(self, bytes: &[u8]) -> String {
        match self {
            DbfEncoding::Known(enc) => enc.decode_without_bom_handling(bytes).0.into_owned(),
            DbfEncoding::SingleByte(_, high) => bytes
                .iter()
                .map(|&b| {
                    if b < 0x80 {
                        char::from(b)
                    } else {
                        high[usize::from(b - 0x80)]
                    }
                })
                .collect(),
            DbfEncoding::Sniff => match std::str::from_utf8(bytes) {
                Ok(s) => s.to_owned(),
                Err(_) => WINDOWS_1252
                    .decode_without_bom_handling(bytes)
                    .0
                    .into_owned(),
            },
        }
    }

    /// The encoding's name, as GDAL and Esri label it (`UTF-8`, `CP437`, ...).
    pub fn name(self) -> String {
        match self {
            DbfEncoding::Known(enc) => enc.name().to_owned(),
            DbfEncoding::SingleByte(cp, _) => format!("CP{cp}"),
            DbfEncoding::Sniff => UTF_8.name().to_owned(),
        }
    }
}

/// Resolves a `.cpg` file's contents (e.g. `UTF-8`, `1252`, `ANSI 1251`, `88591`,
/// `437`).
pub fn from_cpg(contents: &str) -> Option<DbfEncoding> {
    let label = contents.trim().trim_start_matches('\u{feff}');
    let lower = label.to_ascii_lowercase();
    let code = lower
        .strip_prefix("ansi")
        .or_else(|| lower.strip_prefix("cp"))
        .unwrap_or(&lower)
        .trim();
    if let Ok(n) = code.parse::<u32>() {
        return from_code_page(n);
    }
    Encoding::for_label(label.as_bytes()).map(DbfEncoding::Known)
}

fn from_code_page(n: u32) -> Option<DbfEncoding> {
    if let Some(high) = crate::dos::table(n) {
        return Some(DbfEncoding::SingleByte(n, high));
    }
    encoding_rs_code_page(n).map(DbfEncoding::Known)
}

fn encoding_rs_code_page(n: u32) -> Option<&'static Encoding> {
    // Esri .cpg files use bare code page numbers; 8859x are written without dashes.
    let label = match n {
        65001 => "utf-8",
        88591 => "iso-8859-1",
        88592..=88599 => return Encoding::for_label(format!("iso-8859-{}", n - 88590).as_bytes()),
        885910..=885916 => {
            return Encoding::for_label(format!("iso-8859-{}", n - 885900).as_bytes());
        }
        932 => "shift_jis",
        936 => "gbk",
        949 => "euc-kr",
        950 => "big5",
        20866 => "koi8-r",
        866 => "ibm866",
        874 | 1250..=1258 => return Encoding::for_label(format!("windows-{n}").as_bytes()),
        10000 => "macintosh",
        10007 => "x-mac-cyrillic",
        _ => return None,
    };
    Encoding::for_label(label.as_bytes())
}

/// Resolves the language driver ID at byte 29 of the `.dbf` header, with GDAL's
/// table (ArcGIS reads the same DOS code pages; 895 and 620 have no mapping).
pub fn from_ldid(ldid: u8) -> Option<DbfEncoding> {
    let cp = match ldid {
        0x01 | 0x09 | 0x0B | 0x0D | 0x0F | 0x11 | 0x15 | 0x18 | 0x19 | 0x1B => 437,
        0x02 | 0x0A | 0x0E | 0x10 | 0x12 | 0x14 | 0x16 | 0x1A | 0x1D | 0x25 | 0x37 => 850,
        0x03 | 0x57 | 0x58 | 0x59 => 1252,
        0x04 => 10000,
        0x08 | 0x17 | 0x66 => 865,
        0x13 | 0x7B => 932,
        0x1C | 0x6C => 863,
        0x1F | 0x22 | 0x23 | 0x40 | 0x64 | 0x87 => 852,
        0x24 => 860,
        0x26 | 0x65 => 866,
        0x4D | 0x7A => 936,
        0x4E | 0x79 => 949,
        0x4F | 0x78 => 950,
        0x50 | 0x7C => 874,
        0x67 => 861,
        0x6A | 0x86 => 737,
        0x6B | 0x88 => 857,
        0x7D => 1255,
        0x7E => 1256,
        0x96 => 10007,
        0x97 => 10029,
        0xC8 => 1250,
        0xC9 => 1251,
        0xCA => 1254,
        0xCB => 1253,
        0xCC => 1257,
        _ => return None,
    };
    from_code_page(cp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpg_labels() {
        assert_eq!(from_cpg("UTF-8\n"), Some(DbfEncoding::Known(UTF_8)));
        assert_eq!(from_cpg("1252"), Some(DbfEncoding::Known(WINDOWS_1252)));
        let name = |s: &str| from_cpg(s).map(DbfEncoding::name);
        assert_eq!(name("ANSI 1251").as_deref(), Some("windows-1251"));
        assert_eq!(name("88591").as_deref(), Some("windows-1252"));
        assert_eq!(name("88595").as_deref(), Some("ISO-8859-5"));
        assert_eq!(name("437").as_deref(), Some("CP437"));
    }

    #[test]
    fn dos_code_pages() {
        let decode = |ldid: u8, bytes: &[u8]| from_ldid(ldid).unwrap().decode(bytes);
        assert_eq!(decode(0x01, b"Z\x81rich"), "Zürich");
        assert_eq!(decode(0x02, b"\xa5and\xa3"), "Ñandú");
        assert_eq!(decode(0x64, b"\x9d\xa2d\xab"), "Łódź");
    }

    #[test]
    fn sniff_falls_back_to_1252() {
        assert_eq!(DbfEncoding::Sniff.decode(b"Z\xfcrich"), "Zürich");
        assert_eq!(DbfEncoding::Sniff.decode("Zürich".as_bytes()), "Zürich");
    }
}
