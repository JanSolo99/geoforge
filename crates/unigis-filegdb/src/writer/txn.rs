//! Crash-safe editing of an existing geodatabase: a rollback journal per `.gdb`.
//!
//! Every edit runs inside a transaction. Before a file of the geodatabase is changed for
//! the first time in a transaction, what is needed to undo the change is made durable in
//! `<gdb>/.gf-journal/`:
//! - a file that is replaced or removed is first moved into the journal (`moved`);
//! - a file that did not exist is noted as `created`;
//! - a `.gdbtable` that rows are appended to in place keeps its original length and a
//!   copy of its header and field section (the only bytes that are patched in place)
//!   (`grown`).
//!
//! Each manifest line is written and synced before the change it describes. Committing
//! syncs the changed files and then deletes the manifest: that deletion is the commit
//! point. A crash before it leaves a manifest that the next open rolls back (in reverse
//! order), restoring the geodatabase byte for byte; a crash after it leaves the new state.
//! ArcGIS itself knows nothing of the journal, so after a crash the geodatabase should be
//! opened once by GeoForge (any read or edit does it, or [`recover`]) before ArcGIS uses it.
//!
//! Transactions nest: an operation started while another one on the same geodatabase is
//! active (in the same thread) joins it, so a catalog operation that edits several system
//! tables commits or rolls back as a whole, and reads inside it see its own changes.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use unigis_core::{Error, Result};

const JOURNAL_DIR: &str = ".gf-journal";
const MANIFEST: &str = "manifest";
const MAGIC: &str = "geoforge-filegdb rollback journal 1";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Touch {
    /// The original was moved into the journal (or did not exist): the file may be
    /// rewritten freely, rollback restores the original as a whole.
    Whole,
    /// Appended to in place; the original length and header are journaled.
    Grown,
}

struct Journal {
    dir: PathBuf,
    /// Opened lazily by the first change.
    manifest: Option<File>,
    touched: HashMap<String, Touch>,
    backups: usize,
    /// Files to sync at commit.
    dirty: Vec<PathBuf>,
}

thread_local! {
    static ACTIVE: RefCell<HashMap<PathBuf, Rc<RefCell<Journal>>>> = RefCell::new(HashMap::new());
}

fn io(path: &Path) -> impl Fn(std::io::Error) -> Error + '_ {
    move |e| Error::io(path, e)
}

fn key(dir: &Path) -> PathBuf {
    std::fs::canonicalize(dir).unwrap_or_else(|_| std::path::absolute(dir).unwrap_or(dir.into()))
}

fn active_for(dir: &Path) -> Option<Rc<RefCell<Journal>>> {
    ACTIVE.with(|a| a.borrow().get(&key(dir)).cloned())
}

/// The journal of the geodatabase that `file` belongs to, if a transaction is active.
fn journal_of(file: &Path) -> Option<Rc<RefCell<Journal>>> {
    file.parent().and_then(active_for)
}

/// Test hook: `GF_FILEGDB_CRASH_AT=n` ends the process (exit code 86, no destructors,
/// no buffered writes flushed) at the n-th step of journaled
/// editing (after a manifest line is synced, after the change it covers, around the
/// commit point), to check that every interruption is recoverable.
fn step() {
    static STEP: AtomicU64 = AtomicU64::new(0);
    static AT: std::sync::OnceLock<Option<u64>> = std::sync::OnceLock::new();
    if let Some(at) = *AT.get_or_init(|| {
        std::env::var("GF_FILEGDB_CRASH_AT")
            .ok()
            .and_then(|v| v.parse().ok())
    }) && STEP.fetch_add(1, Ordering::SeqCst) + 1 == at
    {
        std::process::exit(86);
    }
}

fn sync_dir(dir: &Path) {
    if let Ok(d) = File::open(dir) {
        d.sync_all().ok();
    }
}

fn name_of(path: &Path) -> Result<String> {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or_else(|| Error::format("FileGDB", format!("{} has no file name", path.display())))
}

impl Journal {
    fn jdir(&self) -> PathBuf {
        self.dir.join(JOURNAL_DIR)
    }

    /// Appends a manifest line and makes it durable before the change it describes.
    fn record(&mut self, line: &str) -> Result<()> {
        if self.manifest.is_none() {
            let jdir = self.jdir();
            std::fs::create_dir_all(&jdir).map_err(io(&jdir))?;
            let path = jdir.join(MANIFEST);
            let mut f = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&path)
                .map_err(io(&path))?;
            writeln!(f, "{MAGIC}\t{}", std::process::id()).map_err(io(&path))?;
            f.sync_all().map_err(io(&path))?;
            sync_dir(&jdir);
            sync_dir(&self.dir);
            self.manifest = Some(f);
        }
        let path = self.jdir().join(MANIFEST);
        let f = self.manifest.as_mut().unwrap();
        writeln!(f, "{line}").map_err(io(&path))?;
        f.sync_all().map_err(io(&path))?;
        step();
        Ok(())
    }

    fn backup_name(&mut self) -> String {
        self.backups += 1;
        format!("b{}", self.backups)
    }

    /// Makes `path` free to be replaced or removed: its original (if any) is in the journal.
    fn take_whole(&mut self, path: &Path) -> Result<()> {
        let name = name_of(path)?;
        match self.touched.get(&name) {
            Some(Touch::Whole) => return Ok(()),
            Some(Touch::Grown) | None => {}
        }
        if path.exists() {
            let b = self.backup_name();
            self.record(&format!("moved\t{name}\t{b}"))?;
            let to = self.jdir().join(&b);
            std::fs::rename(path, &to).map_err(io(path))?;
            sync_dir(&self.dir);
            sync_dir(&self.jdir());
            step();
        } else {
            self.record(&format!("created\t{name}"))?;
        }
        self.touched.insert(name, Touch::Whole);
        Ok(())
    }

    /// Journals the length and header of a `.gdbtable` before rows are appended to it.
    fn take_grown(&mut self, path: &Path) -> Result<()> {
        let name = name_of(path)?;
        if self.touched.contains_key(&name) {
            return Ok(());
        }
        let mut f = File::open(path).map_err(io(path))?;
        let len = f.metadata().map_err(io(path))?.len();
        let mut head = vec![0u8; 40.min(len as usize)];
        f.read_exact(&mut head).map_err(io(path))?;
        // Header, then the field section (whose extent and grid sizes are patched).
        let mut keep = head.len() as u64;
        if head.len() == 40 {
            let at = u64::from_le_bytes(head[32..40].try_into().unwrap());
            let mut size = [0u8; 4];
            if std::os::unix::fs::FileExt::read_exact_at(&f, &mut size, at).is_ok() {
                keep = (at + 4 + u64::from(u32::from_le_bytes(size))).min(len);
            }
        }
        let mut bytes = vec![0u8; keep as usize];
        std::os::unix::fs::FileExt::read_exact_at(&f, &mut bytes, 0).map_err(io(path))?;
        let b = self.backup_name();
        let bpath = self.jdir().join(&b);
        // The backup must be durable before the manifest names it.
        if self.manifest.is_none() {
            std::fs::create_dir_all(self.jdir()).map_err(io(&self.jdir()))?;
        }
        let mut bf = File::create(&bpath).map_err(io(&bpath))?;
        bf.write_all(&bytes).map_err(io(&bpath))?;
        bf.sync_all().map_err(io(&bpath))?;
        self.record(&format!("grown\t{name}\t{len}\t{b}"))?;
        self.touched.insert(name, Touch::Grown);
        Ok(())
    }

    fn mark_dirty(&mut self, path: &Path) {
        if !self.dirty.iter().any(|p| p == path) {
            self.dirty.push(path.to_owned());
        }
    }

    fn commit(&mut self) -> Result<()> {
        if self.manifest.is_none() {
            return Ok(());
        }
        for p in &self.dirty {
            if let Ok(f) = File::open(p) {
                f.sync_all().map_err(io(p))?;
            }
        }
        sync_dir(&self.dir);
        step();
        let jdir = self.jdir();
        self.manifest = None;
        let m = jdir.join(MANIFEST);
        std::fs::remove_file(&m).map_err(io(&m))?;
        sync_dir(&jdir);
        step();
        std::fs::remove_dir_all(&jdir).ok();
        sync_dir(&self.dir);
        Ok(())
    }

    fn rollback(&mut self) -> Result<()> {
        self.manifest = None;
        rollback_dir(&self.dir)
    }
}

/// Undoes the changes listed in `dir`'s manifest, newest first, then removes the journal.
/// Every undo step can be repeated, so an interrupted rollback can simply run again.
fn rollback_dir(dir: &Path) -> Result<()> {
    let jdir = dir.join(JOURNAL_DIR);
    let mpath = jdir.join(MANIFEST);
    if let Ok(text) = std::fs::read_to_string(&mpath) {
        // Only complete lines count: a line cut short was never followed by its change.
        let complete = match text.rfind('\n') {
            Some(i) => &text[..i],
            None => "",
        };
        let mut lines = complete.lines();
        if lines.next().is_some_and(|l| l.starts_with(MAGIC)) {
            let entries: Vec<&str> = lines.collect();
            for line in entries.iter().rev() {
                let p: Vec<&str> = line.split('\t').collect();
                match p.as_slice() {
                    ["created", name] => {
                        let path = dir.join(name);
                        match std::fs::remove_file(&path) {
                            Ok(()) => {}
                            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                            Err(e) => return Err(Error::io(&path, e)),
                        }
                    }
                    ["moved", name, b] => {
                        let from = jdir.join(b);
                        if from.exists() {
                            let to = dir.join(name);
                            std::fs::rename(&from, &to).map_err(io(&to))?;
                        }
                    }
                    ["grown", name, len, b] => {
                        let path = dir.join(name);
                        let len: u64 = len
                            .parse()
                            .map_err(|_| Error::format("FileGDB", "corrupt journal"))?;
                        let head = std::fs::read(jdir.join(b)).map_err(io(&jdir.join(b)))?;
                        let mut f = OpenOptions::new()
                            .write(true)
                            .open(&path)
                            .map_err(io(&path))?;
                        f.set_len(len).map_err(io(&path))?;
                        f.write_all(&head).map_err(io(&path))?;
                        f.sync_all().map_err(io(&path))?;
                    }
                    _ => {
                        return Err(Error::format(
                            "FileGDB",
                            format!("corrupt journal line {line:?}"),
                        ));
                    }
                }
            }
        }
        sync_dir(dir);
        std::fs::remove_file(&mpath).map_err(io(&mpath))?;
        sync_dir(&jdir);
    }
    std::fs::remove_dir_all(&jdir).ok();
    sync_dir(dir);
    Ok(())
}

/// The process that owns `dir`'s journal, if it is another process that is still running.
fn live_owner(dir: &Path) -> Option<u32> {
    let text = std::fs::read_to_string(dir.join(JOURNAL_DIR).join(MANIFEST)).ok()?;
    let pid: u32 = text
        .lines()
        .next()?
        .split('\t')
        .nth(1)?
        .trim()
        .parse()
        .ok()?;
    let alive = pid == std::process::id() || Path::new(&format!("/proc/{pid}")).exists();
    alive.then_some(pid)
}

/// Rolls back an edit of the geodatabase at `dir` that was interrupted (a crash, a killed
/// process). Returns whether there was one. Does nothing while a transaction on `dir` is
/// active in this thread or the journal belongs to a process that is still running.
pub fn recover(dir: impl AsRef<Path>) -> Result<bool> {
    let dir = dir.as_ref();
    if !dir.join(JOURNAL_DIR).exists() || active_for(dir).is_some() {
        return Ok(false);
    }
    if live_owner(dir).is_some() {
        return Ok(false);
    }
    rollback_dir(dir)?;
    Ok(true)
}

/// A transaction on one geodatabase. [`Guard::commit`] makes its changes permanent;
/// dropping it without committing rolls them back. A guard that joined an enclosing
/// transaction leaves both to that one.
pub(crate) struct Guard {
    journal: Rc<RefCell<Journal>>,
    owner: bool,
}

/// Starts a transaction on the geodatabase at `dir` (or joins the active one).
pub(crate) fn begin(dir: &Path) -> Result<Guard> {
    if let Some(journal) = active_for(dir) {
        return Ok(Guard {
            journal,
            owner: false,
        });
    }
    if dir.join(JOURNAL_DIR).exists() {
        if let Some(pid) = live_owner(dir) {
            return Err(Error::Unsupported(format!(
                "{} is being edited by another process ({pid})",
                dir.display()
            )));
        }
        rollback_dir(dir)?;
    }
    let journal = Rc::new(RefCell::new(Journal {
        dir: dir.to_owned(),
        manifest: None,
        touched: HashMap::new(),
        backups: 0,
        dirty: Vec::new(),
    }));
    ACTIVE.with(|a| a.borrow_mut().insert(key(dir), journal.clone()));
    Ok(Guard {
        journal,
        owner: true,
    })
}

impl Guard {
    pub fn commit(self) -> Result<()> {
        if !self.owner {
            return Ok(());
        }
        let r = self.journal.borrow_mut().commit();
        // On failure, Drop rolls back what is left.
        if r.is_ok() {
            self.release();
        }
        r
    }

    fn release(&self) {
        let dir = self.journal.borrow().dir.clone();
        ACTIVE.with(|a| a.borrow_mut().remove(&key(&dir)));
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        if !self.owner {
            return;
        }
        let dir = self.journal.borrow().dir.clone();
        let still_active = ACTIVE.with(|a| a.borrow().contains_key(&key(&dir)));
        if still_active {
            self.journal.borrow_mut().rollback().ok();
            self.release();
        }
    }
}

/// Runs `f` as one transaction on the geodatabase at `dir` (joining an active one):
/// committed if it succeeds, rolled back if it fails.
pub(crate) fn atomic<T>(dir: &Path, f: impl FnOnce() -> Result<T>) -> Result<T> {
    let guard = begin(dir)?;
    let out = f()?;
    guard.commit()?;
    Ok(out)
}

/// Writes a whole file of a geodatabase (replacing it if it exists).
pub(crate) fn write_file(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(j) = journal_of(path) {
        let mut j = j.borrow_mut();
        j.take_whole(path)?;
        std::fs::write(path, bytes).map_err(io(path))?;
        j.mark_dirty(path);
        drop(j);
        step();
        return Ok(());
    }
    std::fs::write(path, bytes).map_err(io(path))
}

/// Creates (truncates) a file of a geodatabase for writing.
pub(crate) fn create_file(path: &Path) -> Result<File> {
    if let Some(j) = journal_of(path) {
        let mut j = j.borrow_mut();
        j.take_whole(path)?;
        j.mark_dirty(path);
    }
    File::create(path).map_err(io(path))
}

/// Removes a file of a geodatabase; a missing file is not an error.
pub(crate) fn remove_file(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    if let Some(j) = journal_of(path) {
        let mut j = j.borrow_mut();
        j.take_whole(path)?;
        // `take_whole` moved an untouched original away already.
    }
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(Error::io(path, e)),
    }
    step();
    Ok(())
}

/// Moves a finished file (from a scratch directory) to its place in the geodatabase.
pub(crate) fn install(from: &Path, to: &Path) -> Result<()> {
    if let Some(j) = journal_of(to) {
        let mut j = j.borrow_mut();
        j.take_whole(to)?;
        j.mark_dirty(to);
    }
    std::fs::rename(from, to).map_err(io(to))?;
    step();
    Ok(())
}

/// Opens a `.gdbtable` to append rows and patch its header in place.
pub(crate) fn open_for_append(path: &Path) -> Result<File> {
    if let Some(j) = journal_of(path) {
        let mut j = j.borrow_mut();
        j.take_grown(path)?;
        j.mark_dirty(path);
    }
    OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(io(path))
}

/// A scratch directory for building replacement files of the table `stem` in `dir`;
/// inside the journal when a transaction is active, so a crash cannot leave it behind.
pub(crate) fn scratch_dir(dir: &Path, stem: &str) -> Result<PathBuf> {
    let scratch = match active_for(dir) {
        Some(j) => j.borrow().jdir().join(format!("scratch-{stem}")),
        None => dir.join(format!(".gf-rewrite-{stem}")),
    };
    std::fs::remove_dir_all(&scratch).ok();
    std::fs::create_dir_all(&scratch).map_err(io(&scratch))?;
    Ok(scratch)
}
