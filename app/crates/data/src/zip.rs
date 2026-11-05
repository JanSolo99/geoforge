//! Zip archives (Deflate or stored): packages bundle a project or map with its data files.

/// Most entries read from an archive.
const MAX_ENTRIES: usize = 10_000;
/// Largest entry inflated (1 GiB).
const MAX_ENTRY: usize = 1 << 30;
/// Most bytes inflated from one archive, all entries together (less in the browser).
pub const MAX_TOTAL: usize = if cfg!(target_arch = "wasm32") { 256 << 20 } else { 1 << 30 };

/// The CRC-32 lookup table, built at compile time.
const CRC_TABLE: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            k += 1;
        }
        table[i] = c;
        i += 1;
    }
    table
};

fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for b in data {
        c = CRC_TABLE.get(((c ^ *b as u32) & 0xFF) as usize).copied().unwrap_or(0) ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

/// Writes a zip archive of (name, bytes) entries, Deflate-compressed. Plain zip (no Zip64)
/// holds at most 65,535 entries and 4 GiB, and names of up to 65,535 bytes; beyond that it's an
/// error rather than a damaged archive.
pub fn write(entries: &[(String, Vec<u8>)]) -> Result<Vec<u8>, String> {
    write_with(entries, true)
}

/// [`write`] without compression: fast, for an archive that only carries files in memory (a
/// folder picked in the browser).
pub fn write_stored(entries: &[(String, Vec<u8>)]) -> Result<Vec<u8>, String> {
    write_with(entries, false)
}

fn write_with(entries: &[(String, Vec<u8>)], compress: bool) -> Result<Vec<u8>, String> {
    let too_big = || "the archive is too large for a zip file".to_string();
    let count = u16::try_from(entries.len()).map_err(|_| too_big())?;
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in entries {
        let n = name.as_bytes();
        let name_len = u16::try_from(n.len()).map_err(|_| too_big())?;
        let data_len = u32::try_from(data.len()).map_err(|_| too_big())?;
        let crc = crc32(data);
        let z = if compress { miniz_oxide::deflate::compress_to_vec(data, 6) } else { Vec::new() };
        let (method, body): (u16, &[u8]) = if compress && z.len() < data.len() { (8, &z) } else { (0, data) };
        let offset = u32::try_from(out.len()).map_err(|_| too_big())?;
        let body_len = u32::try_from(body.len()).map_err(|_| too_big())?;
        let header = |sig: u32, central: bool| {
            let mut h = Vec::new();
            h.extend_from_slice(&sig.to_le_bytes());
            if central {
                h.extend_from_slice(&20u16.to_le_bytes()); // made by
            }
            h.extend_from_slice(&20u16.to_le_bytes()); // needed to extract
            h.extend_from_slice(&0x0800u16.to_le_bytes()); // UTF-8 names
            h.extend_from_slice(&method.to_le_bytes());
            h.extend_from_slice(&0u16.to_le_bytes()); // time
            h.extend_from_slice(&0x21u16.to_le_bytes()); // date: 1980-01-01
            h.extend_from_slice(&crc.to_le_bytes());
            h.extend_from_slice(&body_len.to_le_bytes());
            h.extend_from_slice(&data_len.to_le_bytes());
            h.extend_from_slice(&name_len.to_le_bytes());
            h.extend_from_slice(&0u16.to_le_bytes()); // extra
            if central {
                h.extend_from_slice(&0u16.to_le_bytes()); // comment
                h.extend_from_slice(&0u16.to_le_bytes()); // disk
                h.extend_from_slice(&0u16.to_le_bytes()); // internal attributes
                h.extend_from_slice(&0u32.to_le_bytes()); // external attributes
                h.extend_from_slice(&offset.to_le_bytes());
            }
            h.extend_from_slice(n);
            h
        };
        out.extend_from_slice(&header(0x0403_4B50, false));
        out.extend_from_slice(body);
        central.extend_from_slice(&header(0x0201_4B50, true));
    }
    let cd_offset = u32::try_from(out.len()).map_err(|_| too_big())?;
    let cd_len = u32::try_from(central.len()).map_err(|_| too_big())?;
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4B50u32.to_le_bytes());
    out.extend_from_slice(&[0, 0, 0, 0]);
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&cd_len.to_le_bytes());
    out.extend_from_slice(&cd_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    Ok(out)
}

fn u16_at(b: &[u8], i: usize) -> Option<usize> {
    let s = b.get(i..i.checked_add(2)?)?;
    Some(u16::from_le_bytes([*s.first()?, *s.get(1)?]) as usize)
}

fn u32_at(b: &[u8], i: usize) -> Option<usize> {
    let s = b.get(i..i.checked_add(4)?)?;
    usize::try_from(u32::from_le_bytes([*s.first()?, *s.get(1)?, *s.get(2)?, *s.get(3)?])).ok()
}

/// `a + b + …` of offsets read from the archive, `None` on overflow.
fn add(parts: &[usize]) -> Option<usize> {
    parts.iter().try_fold(0usize, |acc, x| acc.checked_add(*x))
}

/// Reads every entry of a zip archive (stored or Deflate), within [`MAX_TOTAL`] inflated bytes.
pub fn read(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>, String> {
    read_matching(bytes, |_| true)
}

/// Reads the entries whose names pass `want` (others aren't inflated), within [`MAX_TOTAL`]
/// inflated bytes in all.
pub fn read_matching(bytes: &[u8], want: impl Fn(&str) -> bool) -> Result<Vec<(String, Vec<u8>)>, String> {
    read_matching_within(bytes, want, MAX_TOTAL)
}

/// One entry of an archive's central directory: where its (stored or compressed) bytes are.
struct Entry {
    name: String,
    method: usize,
    crc: usize,
    csize: usize,
    usize_: usize,
    data_at: usize,
}

/// The entries of an archive's central directory (folders left out).
fn entries(bytes: &[u8]) -> Result<Vec<Entry>, String> {
    let bad = || "not a zip archive (or a damaged one)".to_string();
    let start = bytes.len().saturating_sub(65_557);
    let eocd = (start..bytes.len().saturating_sub(21))
        .rev()
        .find(|&i| bytes.get(i..i.saturating_add(4)) == Some(&[0x50, 0x4B, 0x05, 0x06]))
        .ok_or_else(bad)?;
    let count = u16_at(bytes, add(&[eocd, 10]).ok_or_else(bad)?).ok_or_else(bad)?;
    if count > MAX_ENTRIES {
        return Err(format!("the archive holds {count} entries; at most {MAX_ENTRIES} can be read"));
    }
    let mut p = u32_at(bytes, add(&[eocd, 16]).ok_or_else(bad)?).ok_or_else(bad)?;
    let mut out = Vec::new();
    for _ in 0..count {
        if u32_at(bytes, p) != Some(0x0201_4B50) {
            return Err(bad());
        }
        let at = |off: usize| add(&[p, off]).ok_or_else(bad);
        let method = u16_at(bytes, at(10)?).ok_or_else(bad)?;
        let crc = u32_at(bytes, at(16)?).ok_or_else(bad)?;
        let csize = u32_at(bytes, at(20)?).ok_or_else(bad)?;
        let usize_ = u32_at(bytes, at(24)?).ok_or_else(bad)?;
        let (nl, xl, cl) =
            (u16_at(bytes, at(28)?).ok_or_else(bad)?, u16_at(bytes, at(30)?).ok_or_else(bad)?, u16_at(bytes, at(32)?).ok_or_else(bad)?);
        let local = u32_at(bytes, at(42)?).ok_or_else(bad)?;
        let name = String::from_utf8_lossy(bytes.get(at(46)?..add(&[p, 46, nl]).ok_or_else(bad)?).ok_or_else(bad)?).to_string();
        p = add(&[p, 46, nl, xl, cl]).ok_or_else(bad)?;
        if name.ends_with('/') {
            continue;
        }
        let lnl = u16_at(bytes, add(&[local, 26]).ok_or_else(bad)?).ok_or_else(bad)?;
        let lxl = u16_at(bytes, add(&[local, 28]).ok_or_else(bad)?).ok_or_else(bad)?;
        let data_at = add(&[local, 30, lnl, lxl]).ok_or_else(bad)?;
        if bytes.len() < add(&[data_at, csize]).ok_or_else(bad)? {
            return Err(bad());
        }
        out.push(Entry { name, method, crc, csize, usize_, data_at });
    }
    Ok(out)
}

/// Where the wanted entries' bytes are in the archive, when every one is stored uncompressed (so
/// they can be read in place, without copying); `None` when one is compressed.
pub fn stored_ranges(bytes: &[u8], want: impl Fn(&str) -> bool) -> Result<Option<Vec<(String, std::ops::Range<usize>)>>, String> {
    let mut out = Vec::new();
    for e in entries(bytes)? {
        if !want(&e.name) {
            continue;
        }
        if e.method != 0 || e.csize != e.usize_ {
            return Ok(None);
        }
        out.push((e.name, e.data_at..e.data_at + e.csize));
    }
    Ok(Some(out))
}

/// Writes an uncompressed zip archive an entry at a time, so a folder's files can be added as
/// they are read (only the archive is held, not the files as well).
#[derive(Default)]
pub struct StoredWriter {
    out: Vec<u8>,
    central: Vec<u8>,
    count: u16,
}

impl StoredWriter {
    /// Adds an entry. Plain zip holds at most 65,535 entries and 4 GiB.
    pub fn add(&mut self, name: &str, data: &[u8]) -> Result<(), String> {
        let too_big = || "the archive is too large for a zip file".to_string();
        let n = name.as_bytes();
        let name_len = u16::try_from(n.len()).map_err(|_| too_big())?;
        let len = u32::try_from(data.len()).map_err(|_| too_big())?;
        let offset = u32::try_from(self.out.len()).map_err(|_| too_big())?;
        self.count = self.count.checked_add(1).ok_or_else(too_big)?;
        let crc = crc32(data);
        let header = |sig: u32, central: bool| {
            let mut h = Vec::with_capacity(46);
            h.extend_from_slice(&sig.to_le_bytes());
            if central {
                h.extend_from_slice(&20u16.to_le_bytes());
            }
            h.extend_from_slice(&20u16.to_le_bytes());
            h.extend_from_slice(&0x0800u16.to_le_bytes());
            h.extend_from_slice(&0u16.to_le_bytes());
            h.extend_from_slice(&0u32.to_le_bytes());
            h.extend_from_slice(&crc.to_le_bytes());
            h.extend_from_slice(&len.to_le_bytes());
            h.extend_from_slice(&len.to_le_bytes());
            h.extend_from_slice(&name_len.to_le_bytes());
            h.extend_from_slice(&0u16.to_le_bytes());
            if central {
                h.extend_from_slice(&[0; 10]);
                h.extend_from_slice(&offset.to_le_bytes());
            }
            h.extend_from_slice(n);
            h
        };
        self.out.extend_from_slice(&header(0x0403_4B50, false));
        self.out.extend_from_slice(data);
        self.central.extend_from_slice(&header(0x0201_4B50, true));
        u32::try_from(self.out.len()).map_err(|_| too_big())?;
        Ok(())
    }

    /// The archive.
    pub fn finish(mut self) -> Result<Vec<u8>, String> {
        let too_big = || "the archive is too large for a zip file".to_string();
        let at = u32::try_from(self.out.len()).map_err(|_| too_big())?;
        let size = u32::try_from(self.central.len()).map_err(|_| too_big())?;
        self.out.append(&mut self.central);
        self.out.extend_from_slice(&0x0605_4B50u32.to_le_bytes());
        self.out.extend_from_slice(&[0; 4]);
        self.out.extend_from_slice(&self.count.to_le_bytes());
        self.out.extend_from_slice(&self.count.to_le_bytes());
        self.out.extend_from_slice(&size.to_le_bytes());
        self.out.extend_from_slice(&at.to_le_bytes());
        self.out.extend_from_slice(&0u16.to_le_bytes());
        Ok(self.out)
    }
}

/// [`read_matching`] within `budget` inflated bytes in all: at most [`MAX_TOTAL`], or a larger
/// budget a caller sets for an archive it knows (a geodatabase upload, whose size it checked).
pub fn read_matching_within(bytes: &[u8], want: impl Fn(&str) -> bool, budget: usize) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut budget = budget;
    let mut out = Vec::new();
    for Entry { name, method, crc, csize, usize_, data_at } in entries(bytes)? {
        if !want(&name) {
            continue;
        }
        let body = bytes.get(data_at..data_at + csize).unwrap_or_default();
        if usize_ > MAX_ENTRY || usize_ > budget {
            return Err(format!("{name} is too large"));
        }
        // Inflate no more than the entry says it holds (and the archive's budget allows).
        let data = match method {
            0 if csize != usize_ => return Err(format!("{name} is damaged (wrong size)")),
            0 => body.to_vec(),
            8 => miniz_oxide::inflate::decompress_to_vec_with_limit(body, usize_).map_err(|_| format!("{name} is damaged"))?,
            m => return Err(format!("{name} uses an unsupported compression ({m})")),
        };
        if data.len() != usize_ {
            return Err(format!("{name} is damaged (wrong size)"));
        }
        budget -= data.len();
        if crc32(&data) as usize != crc {
            return Err(format!("{name} is damaged (checksum mismatch)"));
        }
        out.push((name, data));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let big = "UniGIS ".repeat(1000).into_bytes();
        let entries = vec![("project.json".to_string(), big.clone()), ("data/x.bin".to_string(), vec![1, 2, 3])];
        let z = write(&entries).unwrap();
        assert!(z.len() < big.len(), "compressed");
        let back = read(&z).unwrap_or_default();
        assert_eq!(back, entries);
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert!(read(b"not a zip").is_err());
        assert!(read(&z[..z.len() / 2]).is_err());
        // A flipped byte in stored data fails its checksum.
        let mut stored = write(&[("a.bin".to_string(), vec![7u8; 3])]).unwrap();
        if let Some(b) = stored.get_mut(30 + 5) {
            *b ^= 0xFF;
        }
        assert!(read(&stored).is_err());
        // Only the wanted entries are read.
        let only = read_matching(&z, |n| n.starts_with("data/")).unwrap();
        assert_eq!(only.len(), 1);
        // Offsets pointing past the end are refused, not followed.
        let mut far = z.clone();
        let eocd = far.len() - 22;
        far[eocd + 16..eocd + 20].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(read(&far).is_err());
    }

    #[test]
    fn stored_entries_are_read_in_place_and_written_incrementally() {
        let mut w = StoredWriter::default();
        w.add("Town.gdb/a00000001.gdbtable", b"catalog").unwrap();
        w.add("Town.gdb/a00000001.gdbtablx", &[1, 2, 3]).unwrap();
        let z = w.finish().unwrap();
        assert_eq!(
            read(&z).unwrap(),
            vec![
                ("Town.gdb/a00000001.gdbtable".to_string(), b"catalog".to_vec()),
                ("Town.gdb/a00000001.gdbtablx".to_string(), vec![1, 2, 3])
            ]
        );
        let ranges = stored_ranges(&z, |_| true).unwrap().unwrap();
        assert_eq!(&z[ranges[0].1.clone()], b"catalog");
        assert_eq!(&z[ranges[1].1.clone()], &[1, 2, 3]);
        // A compressed entry can't be read in place.
        let deflated = write(&[("a".to_string(), vec![0u8; 1000])]).unwrap();
        assert!(stored_ranges(&deflated, |_| true).unwrap().is_none());
        assert!(stored_ranges(b"junk", |_| true).is_err());
        for n in 0..z.len() {
            let _ = stored_ranges(&z[..n], |_| true);
        }
    }
}
