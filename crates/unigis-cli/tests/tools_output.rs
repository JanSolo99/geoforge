//! Tool inventory remains usable when a pipeline consumer closes stdout early.
#![cfg(unix)]

use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::process::{Command, Stdio};

#[test]
fn closed_tool_inventory_stdout_exits_without_panicking() {
    // Close the reader before spawning, so the first write deterministically
    // fails rather than racing a pipe's buffer capacity or a `head` process.
    let (reader, writer) = UnixStream::pair().unwrap();
    drop(reader);
    let fd: OwnedFd = writer.into();
    let result = Command::new(env!("CARGO_BIN_EXE_ug"))
        .arg("tools")
        .stdout(Stdio::from(fd))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stderr.is_empty());
}
