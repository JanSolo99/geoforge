//! The file system the geodatabase reader and writer use: the real one, or folders held in
//! memory and mounted at a path (an uploaded geodatabase being edited, in the browser or on the
//! desktop). Paths under a mounted folder — every path in the browser, which has no file
//! system — are files in memory; the rest go to the disk. The writer's journal, renames and
//! positioned writes work the same on both, so every edit runs unchanged on an upload.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

type Data = Arc<Mutex<Vec<u8>>>;

#[derive(Default)]
struct Memory {
    /// Mounted roots.
    roots: Vec<PathBuf>,
    files: BTreeMap<PathBuf, Data>,
    dirs: BTreeSet<PathBuf>,
}

static MEMORY: Mutex<Memory> = Mutex::new(Memory {
    roots: Vec::new(),
    files: BTreeMap::new(),
    dirs: BTreeSet::new(),
});

fn memory() -> MutexGuard<'static, Memory> {
    MEMORY.lock().unwrap_or_else(|e| e.into_inner())
}

fn lock(d: &Data) -> MutexGuard<'_, Vec<u8>> {
    d.lock().unwrap_or_else(|e| e.into_inner())
}

/// `p` without `.` and `..` parts (memory paths are compared as written).
fn clean(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

/// Whether `p` is in memory: under a mounted folder, or anywhere in the browser.
fn in_memory(p: &Path) -> bool {
    cfg!(target_arch = "wasm32") || {
        let p = clean(p);
        memory().roots.iter().any(|r| p.starts_with(r))
    }
}

fn not_found() -> io::Error {
    io::ErrorKind::NotFound.into()
}

/// Mounts a folder of files held in memory at `root` (made unique by the caller, e.g.
/// `/unigis-memory/1/Roads.gdb`): `files` by name, directly in it.
pub fn mount(root: &Path, files: impl IntoIterator<Item = (String, Vec<u8>)>) {
    let root = clean(root);
    let mut m = memory();
    m.roots.retain(|r| *r != root);
    m.roots.push(root.clone());
    m.dirs.insert(root.clone());
    for (name, bytes) in files {
        let leaf = Path::new(&name)
            .file_name()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(&name));
        m.files.insert(root.join(leaf), Arc::new(Mutex::new(bytes)));
    }
}

/// Unmounts `root`, returning the files directly in it (not those of subfolders such as an
/// edit's journal), by name.
pub fn unmount(root: &Path) -> Vec<(String, Vec<u8>)> {
    let root = clean(root);
    let mut m = memory();
    m.roots.retain(|r| *r != root);
    let inside: Vec<PathBuf> = m
        .files
        .keys()
        .filter(|p| p.starts_with(&root))
        .cloned()
        .collect();
    let mut out = Vec::new();
    for p in inside {
        if let Some(d) = m.files.remove(&p)
            && p.parent() == Some(root.as_path())
            && let Some(name) = p.file_name().and_then(|n| n.to_str())
        {
            out.push((name.to_string(), std::mem::take(&mut *lock(&d))));
        }
    }
    m.dirs.retain(|d| !d.starts_with(&root));
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

pub fn read(p: impl AsRef<Path>) -> io::Result<Vec<u8>> {
    let p = p.as_ref();
    if !in_memory(p) {
        return std::fs::read(p);
    }
    let d = memory()
        .files
        .get(&clean(p))
        .cloned()
        .ok_or_else(not_found)?;
    Ok(lock(&d).clone())
}

pub fn read_to_string(p: impl AsRef<Path>) -> io::Result<String> {
    String::from_utf8(read(p)?).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

pub fn write(p: impl AsRef<Path>, bytes: impl AsRef<[u8]>) -> io::Result<()> {
    let p = p.as_ref();
    if !in_memory(p) {
        return std::fs::write(p, bytes);
    }
    let p = clean(p);
    let mut m = memory();
    if !p.parent().is_some_and(|d| m.dirs.contains(d)) {
        return Err(not_found());
    }
    m.files
        .insert(p, Arc::new(Mutex::new(bytes.as_ref().to_vec())));
    Ok(())
}

pub fn remove_file(p: impl AsRef<Path>) -> io::Result<()> {
    let p = p.as_ref();
    if !in_memory(p) {
        return std::fs::remove_file(p);
    }
    memory()
        .files
        .remove(&clean(p))
        .map(|_| ())
        .ok_or_else(not_found)
}

pub fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<()> {
    let (from, to) = (from.as_ref(), to.as_ref());
    match (in_memory(from), in_memory(to)) {
        (false, false) => std::fs::rename(from, to),
        (true, true) => {
            let (from, to) = (clean(from), clean(to));
            let mut m = memory();
            if let Some(d) = m.files.remove(&from) {
                m.files.insert(to, d);
                return Ok(());
            }
            // A folder: everything under it moves.
            if !m.dirs.contains(&from) {
                return Err(not_found());
            }
            let moved: Vec<PathBuf> = m
                .files
                .keys()
                .filter(|p| p.starts_with(&from))
                .cloned()
                .collect();
            for p in moved {
                if let (Some(d), Ok(rest)) = (m.files.remove(&p), p.strip_prefix(&from)) {
                    m.files.insert(to.join(rest), d);
                }
            }
            let dirs: Vec<PathBuf> = m
                .dirs
                .iter()
                .filter(|d| d.starts_with(&from))
                .cloned()
                .collect();
            for d in dirs {
                m.dirs.remove(&d);
                if let Ok(rest) = d.strip_prefix(&from) {
                    m.dirs.insert(to.join(rest));
                }
            }
            Ok(())
        }
        _ => Err(io::Error::other(
            "can't move a file between memory and the disk",
        )),
    }
}

pub fn create_dir_all(p: impl AsRef<Path>) -> io::Result<()> {
    let p = p.as_ref();
    if !in_memory(p) {
        return std::fs::create_dir_all(p);
    }
    let mut m = memory();
    for a in clean(p).ancestors() {
        if !a.as_os_str().is_empty() {
            m.dirs.insert(a.to_path_buf());
        }
    }
    Ok(())
}

pub fn remove_dir_all(p: impl AsRef<Path>) -> io::Result<()> {
    let p = p.as_ref();
    if !in_memory(p) {
        return std::fs::remove_dir_all(p);
    }
    let p = clean(p);
    let mut m = memory();
    if !m.dirs.contains(&p) {
        return Err(not_found());
    }
    m.files.retain(|f, _| !f.starts_with(&p));
    m.dirs.retain(|d| !d.starts_with(&p));
    Ok(())
}

/// A folder's entry, as [`std::fs::DirEntry`].
pub struct DirEntry(PathBuf);

impl DirEntry {
    pub fn path(&self) -> PathBuf {
        self.0.clone()
    }
    pub fn file_name(&self) -> std::ffi::OsString {
        self.0
            .file_name()
            .map(|n| n.to_os_string())
            .unwrap_or_default()
    }
}

/// The entries directly in a folder, as [`std::fs::read_dir`] gives them.
pub fn read_dir(p: impl AsRef<Path>) -> io::Result<std::vec::IntoIter<io::Result<DirEntry>>> {
    let p = p.as_ref();
    if !in_memory(p) {
        let all: Vec<io::Result<DirEntry>> = std::fs::read_dir(p)?
            .map(|e| e.map(|e| DirEntry(e.path())))
            .collect();
        return Ok(all.into_iter());
    }
    let p = clean(p);
    let m = memory();
    if !m.dirs.contains(&p) {
        return Err(not_found());
    }
    let mut out: Vec<PathBuf> = m
        .files
        .keys()
        .chain(m.dirs.iter())
        .filter(|f| f.parent() == Some(p.as_path()))
        .cloned()
        .collect();
    out.sort();
    out.dedup();
    Ok(out
        .into_iter()
        .map(|p| Ok(DirEntry(p)))
        .collect::<Vec<_>>()
        .into_iter())
}

pub fn is_file(p: impl AsRef<Path>) -> bool {
    let p = p.as_ref();
    if !in_memory(p) {
        return p.is_file();
    }
    memory().files.contains_key(&clean(p))
}

pub fn is_dir(p: impl AsRef<Path>) -> bool {
    let p = p.as_ref();
    if !in_memory(p) {
        return p.is_dir();
    }
    memory().dirs.contains(&clean(p))
}

pub fn exists(p: impl AsRef<Path>) -> bool {
    is_file(&p) || is_dir(&p)
}

/// The path with symbolic links resolved (on disk); a memory path as it is.
pub fn canonicalize(p: impl AsRef<Path>) -> io::Result<PathBuf> {
    let p = p.as_ref();
    if !in_memory(p) {
        return std::fs::canonicalize(p);
    }
    if exists(p) {
        Ok(clean(p))
    } else {
        Err(not_found())
    }
}

/// How to open a file, as [`std::fs::OpenOptions`].
#[derive(Default, Clone)]
pub struct OpenOptions {
    read: bool,
    write: bool,
    create: bool,
    truncate: bool,
    append: bool,
    create_new: bool,
}

impl OpenOptions {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn read(&mut self, v: bool) -> &mut Self {
        self.read = v;
        self
    }
    pub fn write(&mut self, v: bool) -> &mut Self {
        self.write = v;
        self
    }
    pub fn create(&mut self, v: bool) -> &mut Self {
        self.create = v;
        self
    }
    pub fn truncate(&mut self, v: bool) -> &mut Self {
        self.truncate = v;
        self
    }
    pub fn append(&mut self, v: bool) -> &mut Self {
        self.append = v;
        self
    }
    /// Fails if the file exists.
    pub fn create_new(&mut self, v: bool) -> &mut Self {
        self.create_new = v;
        self
    }

    pub fn open(&self, p: impl AsRef<Path>) -> io::Result<File> {
        let p = p.as_ref();
        if !in_memory(p) {
            let f = std::fs::OpenOptions::new()
                .read(self.read)
                .write(self.write)
                .create(self.create)
                .truncate(self.truncate)
                .append(self.append)
                .create_new(self.create_new)
                .open(p)?;
            return Ok(File::Disk(f));
        }
        let path = clean(p);
        let mut m = memory();
        let data = match m.files.get(&path) {
            Some(_) if self.create_new => return Err(io::ErrorKind::AlreadyExists.into()),
            Some(d) => d.clone(),
            None if self.create || self.create_new => {
                if !path.parent().is_some_and(|d| m.dirs.contains(d)) {
                    return Err(not_found());
                }
                let d: Data = Arc::new(Mutex::new(Vec::new()));
                m.files.insert(path.clone(), d.clone());
                d
            }
            None => return Err(not_found()),
        };
        drop(m);
        if self.truncate {
            lock(&data).clear();
        }
        let pos = if self.append {
            lock(&data).len() as u64
        } else {
            0
        };
        Ok(File::Memory {
            data,
            pos,
            append: self.append,
        })
    }
}

/// An open file, on disk or in memory.
pub enum File {
    Disk(std::fs::File),
    Memory { data: Data, pos: u64, append: bool },
}

impl File {
    pub fn open(p: impl AsRef<Path>) -> io::Result<File> {
        OpenOptions::new().read(true).open(p)
    }

    pub fn create(p: impl AsRef<Path>) -> io::Result<File> {
        OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(p)
    }

    /// Whether the file holds nothing.
    pub fn is_empty(&self) -> io::Result<bool> {
        self.len().map(|n| n == 0)
    }

    pub fn len(&self) -> io::Result<u64> {
        match self {
            File::Disk(f) => f.metadata().map(|m| m.len()),
            File::Memory { data, .. } => Ok(lock(data).len() as u64),
        }
    }

    pub fn sync_all(&self) -> io::Result<()> {
        match self {
            File::Disk(f) => f.sync_all(),
            File::Memory { .. } => Ok(()),
        }
    }

    pub fn set_len(&self, len: u64) -> io::Result<()> {
        match self {
            File::Disk(f) => f.set_len(len),
            File::Memory { data, .. } => {
                lock(data).resize(
                    usize::try_from(len).map_err(|_| io::Error::other("too large"))?,
                    0,
                );
                Ok(())
            }
        }
    }

    /// Fills `buf` from `offset`, failing if the file ends first.
    pub fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> io::Result<()> {
        match self {
            File::Disk(f) => crate::store::read_exact_at(f, buf, offset),
            File::Memory { data, .. } => {
                let d = lock(data);
                let start = usize::try_from(offset)
                    .map_err(|_| io::Error::from(io::ErrorKind::UnexpectedEof))?;
                let src = start
                    .checked_add(buf.len())
                    .and_then(|end| d.get(start..end))
                    .ok_or(io::ErrorKind::UnexpectedEof)?;
                buf.copy_from_slice(src);
                Ok(())
            }
        }
    }

    /// Writes `buf` at `offset`, growing the file as needed.
    pub fn write_all_at(&self, buf: &[u8], offset: u64) -> io::Result<()> {
        match self {
            File::Disk(f) => crate::store::write_all_at(f, buf, offset),
            File::Memory { data, .. } => {
                let mut d = lock(data);
                let start = usize::try_from(offset).map_err(|_| io::Error::other("too large"))?;
                let end = start
                    .checked_add(buf.len())
                    .ok_or_else(|| io::Error::other("too large"))?;
                if d.len() < end {
                    d.resize(end, 0);
                }
                d[start..end].copy_from_slice(buf);
                Ok(())
            }
        }
    }
}

impl Read for File {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            File::Disk(f) => f.read(buf),
            File::Memory { data, pos, .. } => {
                let d = lock(data);
                let start = (*pos as usize).min(d.len());
                let n = buf.len().min(d.len() - start);
                buf[..n].copy_from_slice(&d[start..start + n]);
                *pos += n as u64;
                Ok(n)
            }
        }
    }
}

impl Write for File {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            File::Disk(f) => f.write(buf),
            File::Memory { data, pos, append } => {
                if *append {
                    *pos = lock(data).len() as u64;
                }
                let at = *pos;
                let f = File::Memory {
                    data: data.clone(),
                    pos: 0,
                    append: false,
                };
                f.write_all_at(buf, at)?;
                *pos += buf.len() as u64;
                Ok(buf.len())
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            File::Disk(f) => f.flush(),
            File::Memory { .. } => Ok(()),
        }
    }
}

impl Seek for File {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        match self {
            File::Disk(f) => f.seek(to),
            File::Memory { data, pos, .. } => {
                let len = lock(data).len() as i64;
                let new = match to {
                    SeekFrom::Start(n) => n as i64,
                    SeekFrom::End(d) => len + d,
                    SeekFrom::Current(d) => *pos as i64 + d,
                };
                if new < 0 {
                    return Err(io::Error::from(io::ErrorKind::InvalidInput));
                }
                *pos = new as u64;
                Ok(*pos)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_folder_behaves_like_the_disk() {
        let root = Path::new("/unigis-vfs-test/A.gdb");
        mount(root, [("a00000001.gdbtable".to_string(), b"abc".to_vec())]);
        assert!(is_file(root.join("a00000001.gdbtable")) && is_dir(root));
        assert_eq!(read(root.join("./a00000001.gdbtable")).unwrap(), b"abc");
        // Positioned and streamed writes, append, set_len.
        let f = OpenOptions::new()
            .read(true)
            .write(true)
            .open(root.join("a00000001.gdbtable"))
            .unwrap();
        f.write_all_at(b"XY", 4).unwrap();
        assert_eq!(read(root.join("a00000001.gdbtable")).unwrap(), b"abc\0XY");
        f.set_len(2).unwrap();
        let mut g = File::create(root.join("new")).unwrap();
        g.write_all(b"hello").unwrap();
        g.seek(SeekFrom::Start(1)).unwrap();
        g.write_all(b"E").unwrap();
        let mut a = OpenOptions::new()
            .append(true)
            .open(root.join("new"))
            .unwrap();
        a.write_all(b"!").unwrap();
        assert_eq!(read_to_string(root.join("new")).unwrap(), "hEllo!");
        // A journal folder, renames, removal.
        create_dir_all(root.join(".j")).unwrap();
        write(root.join(".j/backup"), b"b").unwrap();
        rename(root.join("new"), root.join("renamed")).unwrap();
        assert!(!is_file(root.join("new")));
        assert_eq!(
            read_dir(root).unwrap().count(),
            3,
            "two files and the journal folder"
        );
        assert!(
            write(root.join("nowhere/x"), b"").is_err(),
            "no such folder"
        );
        remove_file(root.join("renamed")).unwrap();
        let back = unmount(root);
        assert_eq!(
            back,
            vec![("a00000001.gdbtable".to_string(), b"ab".to_vec())],
            "the journal isn't returned"
        );
        assert!(!is_dir(root) && !is_file(root.join("a00000001.gdbtable")));
    }
}
