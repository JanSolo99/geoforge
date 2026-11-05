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
    memory: Option<Arc<HashMap<String, Arc<[u8]>>>>,
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
            .map(|(name, bytes)| (key(Path::new(&name)), Arc::<[u8]>::from(bytes)))
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
            None => File::open(path).map(Handle::Disk),
            Some(m) => m
                .get(&key(path))
                .cloned()
                .map(Handle::Memory)
                .ok_or_else(|| io::ErrorKind::NotFound.into()),
        }
    }

    pub(crate) fn is_file(&self, path: &Path) -> bool {
        match &self.memory {
            None => path.is_file(),
            Some(m) => m.contains_key(&key(path)),
        }
    }

    pub(crate) fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        match &self.memory {
            None => std::fs::read(path),
            Some(_) => self.open(path).and_then(|h| match h {
                Handle::Memory(b) => Ok(b.to_vec()),
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
    Disk(File),
    Memory(Arc<[u8]>),
}

impl Handle {
    pub fn len(&self) -> io::Result<u64> {
        match self {
            Handle::Disk(f) => f.metadata().map(|m| m.len()),
            Handle::Memory(b) => Ok(b.len() as u64),
        }
    }

    /// Fills `buf` from `offset`, failing if the file ends first.
    pub fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> io::Result<()> {
        match self {
            Handle::Disk(f) => read_exact_at(f, buf, offset),
            Handle::Memory(b) => {
                let start = usize::try_from(offset).map_err(|_| eof())?;
                let end = start.checked_add(buf.len()).ok_or_else(eof)?;
                buf.copy_from_slice(b.get(start..end).ok_or_else(eof)?);
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
}
