//! Text encoding of `.dbf` strings: `.cpg` file, then the header's language driver ID.

use encoding_rs::{Encoding, UTF_8, WINDOWS_1252};

/// How strings in a `.dbf` are decoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbfEncoding {
    Known(&'static Encoding),
    /// No `.cpg` and no language driver ID: UTF-8 when the bytes are valid UTF-8,
    /// otherwise Windows-1252 (ArcGIS's "ANSI" default on Western systems).
    Sniff,
}

impl DbfEncoding {
    pub fn decode(self, bytes: &[u8]) -> String {
        match self {
            DbfEncoding::Known(enc) => enc.decode_without_bom_handling(bytes).0.into_owned(),
            DbfEncoding::Sniff => match std::str::from_utf8(bytes) {
                Ok(s) => s.to_owned(),
                Err(_) => WINDOWS_1252
                    .decode_without_bom_handling(bytes)
                    .0
                    .into_owned(),
            },
        }
    }

    pub fn encoding(self) -> &'static Encoding {
        match self {
            DbfEncoding::Known(enc) => enc,
            DbfEncoding::Sniff => UTF_8,
        }
    }
}

/// Resolves a `.cpg` file's contents (e.g. `UTF-8`, `1252`, `ANSI 1251`, `88591`).
pub fn from_cpg(contents: &str) -> Option<&'static Encoding> {
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
    Encoding::for_label(label.as_bytes())
}

fn from_code_page(n: u32) -> Option<&'static Encoding> {
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
        _ => return None,
    };
    Encoding::for_label(label.as_bytes())
}

/// Resolves the language driver ID at byte 29 of the `.dbf` header.
pub fn from_ldid(ldid: u8) -> Option<&'static Encoding> {
    let cp = match ldid {
        0x03 | 0x57 | 0x58 | 0x59 => 1252,
        0x4D | 0x7A => 936,
        0x4E | 0x79 => 949,
        0x4F | 0x78 => 950,
        0x13 | 0x7B => 932,
        0x50 | 0x7C => 874,
        0x26 | 0x65 => 866,
        0x7D => 1255,
        0x7E => 1256,
        0xC8 => 1250,
        0xC9 => 1251,
        0xCA => 1254,
        0xCB => 1253,
        0xCC => 1257,
        // DOS code pages (437, 850, 852, ...) are not supported by encoding_rs.
        _ => return None,
    };
    from_code_page(cp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpg_labels() {
        assert_eq!(from_cpg("UTF-8\n"), Some(UTF_8));
        assert_eq!(from_cpg("1252"), Some(WINDOWS_1252));
        assert_eq!(
            from_cpg("ANSI 1251").map(|e| e.name()),
            Some("windows-1251")
        );
        assert_eq!(from_cpg("88591").map(|e| e.name()), Some("windows-1252"));
        assert_eq!(from_cpg("88595").map(|e| e.name()), Some("ISO-8859-5"));
    }

    #[test]
    fn sniff_falls_back_to_1252() {
        assert_eq!(DbfEncoding::Sniff.decode(b"Z\xfcrich"), "Zürich");
        assert_eq!(DbfEncoding::Sniff.decode("Zürich".as_bytes()), "Zürich");
    }
}
