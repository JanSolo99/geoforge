//! Where a geodatabase's files come from: a `.gdb` folder on disk, or its files held in memory
//! (a browser upload, a zip archive). Reading goes through [`Store`], so the same reader serves
//! both; writing is disk-only.

use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::path::Path;
use std::sync::Arc;

/// The files of one geodatabase. The default is the file system.
#[derive(Clone, Default)]
pub struct Store {
    /// Lower-case file name → contents; `None` reads the file system.
    memory: Option<Arc<HashMap<String, Slice>>>,
}

/// A file held in memory: a range of a shared buffer (the whole of its own, or an entry of an
/// uncompressed zip archive, read in place).
#[derive(Clone)]
pub(crate) struct Slice {
    buf: Arc<Vec<u8>>,
    start: usize,
    end: usize,
}

impl Slice {
    fn bytes(&self) -> &[u8] {
        self.buf.get(self.start..self.end).unwrap_or_default()
    }
}

impl std::fmt::Debug for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.memory {
            None => f.write_str("Store::Disk"),
            Some(m) => write!(f, "Store::Memory({} files)", m.len()),
        }
    }
}

impl Store {
    /// Files read from the file system.
    pub fn disk() -> Self {
        Self::default()
    }

    /// Files held in memory, by name. A geodatabase is a flat folder, so only each name's last
    /// path component counts (`Roads.gdb/a00000001.gdbtable` and `a00000001.gdbtable` are the
    /// same file); names are compared without regard to case, as on Windows.
    pub fn memory(files: impl IntoIterator<Item = (String, Vec<u8>)>) -> Self {
        let map = files
            .into_iter()
            .map(|(name, bytes)| {
                let end = bytes.len();
                (
                    key(Path::new(&name)),
                    Slice {
                        buf: Arc::new(bytes),
                        start: 0,
                        end,
                    },
                )
            })
            .collect();
        Self {
            memory: Some(Arc::new(map)),
        }
    }

    /// Files that are ranges of one buffer held in memory (the entries of an uncompressed zip
    /// archive), read in place without copying. Ranges past the buffer are left out.
    pub fn shared(
        buf: Arc<Vec<u8>>,
        files: impl IntoIterator<Item = (String, std::ops::Range<usize>)>,
    ) -> Self {
        let map = files
            .into_iter()
            .filter(|(_, r)| r.start <= r.end && r.end <= buf.len())
            .map(|(name, r)| {
                (
                    key(Path::new(&name)),
                    Slice {
                        buf: buf.clone(),
                        start: r.start,
                        end: r.end,
                    },
                )
            })
            .collect();
        Self {
            memory: Some(Arc::new(map)),
        }
    }

    /// True when the files are held in memory.
    pub fn is_memory(&self) -> bool {
        self.memory.is_some()
    }

    pub(crate) fn open(&self, path: &Path) -> io::Result<Handle> {
        match &self.memory {
            None => crate::vfs::File::open(path).map(Handle::Disk),
            Some(m) => m
                .get(&key(path))
                .cloned()
                .map(Handle::Memory)
                .ok_or_else(|| io::ErrorKind::NotFound.into()),
        }
    }

    /// Whether the file exists.
    pub fn is_file(&self, path: &Path) -> bool {
        match &self.memory {
            None => crate::vfs::is_file(path),
            Some(m) => m.contains_key(&key(path)),
        }
    }

    pub(crate) fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        match &self.memory {
            None => crate::vfs::read(path),
            Some(_) => self.open(path).and_then(|h| match h {
                Handle::Memory(b) => Ok(b.bytes().to_vec()),
                Handle::Disk(_) => Err(io::ErrorKind::NotFound.into()),
            }),
        }
    }
}

fn key(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

/// One open file.
pub(crate) enum Handle {
    Disk(crate::vfs::File),
    Memory(Slice),
}

impl Handle {
    pub fn len(&self) -> io::Result<u64> {
        match self {
            Handle::Disk(f) => f.len(),
            Handle::Memory(b) => Ok(b.bytes().len() as u64),
        }
    }

    /// Fills `buf` from `offset`, failing if the file ends first.
    pub fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> io::Result<()> {
        match self {
            Handle::Disk(f) => f.read_exact_at(buf, offset),
            Handle::Memory(b) => {
                let start = usize::try_from(offset).map_err(|_| eof())?;
                let end = start.checked_add(buf.len()).ok_or_else(eof)?;
                buf.copy_from_slice(b.bytes().get(start..end).ok_or_else(eof)?);
                Ok(())
            }
        }
    }
}

fn eof() -> io::Error {
    io::ErrorKind::UnexpectedEof.into()
}

/// Positioned read from a file without moving a shared cursor where the platform allows it.
#[cfg(unix)]
pub(crate) fn read_exact_at(file: &File, buf: &mut [u8], offset: u64) -> io::Result<()> {
    std::os::unix::fs::FileExt::read_exact_at(file, buf, offset)
}

/// Positioned read from a file (seek, then read) on platforms without `pread`.
#[cfg(not(unix))]
pub(crate) fn read_exact_at(mut file: &File, buf: &mut [u8], offset: u64) -> io::Result<()> {
    use std::io::{Read, Seek, SeekFrom};
    file.seek(SeekFrom::Start(offset))?;
    file.read_exact(buf)
}

/// Positioned write to a file.
#[cfg(unix)]
pub(crate) fn write_all_at(file: &File, buf: &[u8], offset: u64) -> io::Result<()> {
    std::os::unix::fs::FileExt::write_all_at(file, buf, offset)
}

/// Positioned write to a file (seek, then write) on platforms without `pwrite`.
#[cfg(not(unix))]
pub(crate) fn write_all_at(mut file: &File, buf: &[u8], offset: u64) -> io::Result<()> {
    use std::io::{Seek, SeekFrom, Write};
    file.seek(SeekFrom::Start(offset))?;
    file.write_all(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_reads_by_last_component_without_case() {
        let s = Store::memory([("X.gdb/A00000001.GDBTABLE".to_string(), vec![1, 2, 3, 4])]);
        let p = Path::new("anything/a00000001.gdbtable");
        assert!(s.is_file(p));
        let h = s.open(p).unwrap();
        assert_eq!(h.len().unwrap(), 4);
        let mut b = [0u8; 2];
        h.read_exact_at(&mut b, 2).unwrap();
        assert_eq!(b, [3, 4]);
        assert!(h.read_exact_at(&mut b, 3).is_err());
        assert!(h.read_exact_at(&mut b, u64::MAX).is_err());
        assert!(!s.is_file(Path::new("a00000002.gdbtable")));
    }

    #[test]
    fn shared_ranges_read_in_place() {
        let buf = Arc::new(b"xxABCDyyEF".to_vec());
        let s = Store::shared(
            buf,
            [
                ("a.gdbtable".into(), 2..6),
                ("B.gdbtablx".into(), 8..10),
                ("bad".into(), 9..40),
            ],
        );
        let mut b = [0u8; 2];
        s.open(Path::new("A.GDBTABLE"))
            .unwrap()
            .read_exact_at(&mut b, 2)
            .unwrap();
        assert_eq!(&b, b"CD");
        assert_eq!(s.read(Path::new("b.gdbtablx")).unwrap(), b"EF");
        assert!(!s.is_file(Path::new("bad")), "past the buffer");
        assert!(
            s.open(Path::new("a.gdbtable"))
                .unwrap()
                .read_exact_at(&mut b, 3)
                .is_err()
        );
    }
}
