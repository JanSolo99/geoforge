//! The clock, process id and entropy the writer seeds GUIDs and journals with, on the desktop
//! and in the browser (where `std::time::SystemTime::now`, `std::process::id` and
//! `/dev/urandom` don't exist).

#[cfg(target_arch = "wasm32")]
use std::sync::atomic::{AtomicU64, Ordering};

/// Nanoseconds since 1970.
pub(crate) fn now_nanos() -> u64 {
    #[cfg(not(target_arch = "wasm32"))]
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
    #[cfg(target_arch = "wasm32")]
    let t = web_time::SystemTime::now().duration_since(web_time::UNIX_EPOCH);
    t.map_or(0, |d| d.as_nanos() as u64)
}

/// This process's id (1 in the browser: one page, one writer).
pub(crate) fn pid() -> u32 {
    if cfg!(target_arch = "wasm32") {
        1
    } else {
        std::process::id()
    }
}

/// Fills `buf` with random bytes: the OS's (`/dev/urandom`) on the desktop; in the browser a
/// SplitMix64 stream seeded from the clock and a counter, unique enough for item GUIDs.
pub(crate) fn entropy(buf: &mut [u8]) -> std::io::Result<()> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::io::Read;
        std::fs::File::open("/dev/urandom")?.read_exact(buf)
    }
    #[cfg(target_arch = "wasm32")]
    {
        static STATE: AtomicU64 = AtomicU64::new(0);
        for chunk in buf.chunks_mut(8) {
            let s = STATE.fetch_add(0x9E37_79B9_7F4A_7C15, Ordering::Relaxed) ^ now_nanos();
            let mut z = s.wrapping_add(0x9E37_79B9_7F4A_7C15);
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^= z >> 31;
            chunk.copy_from_slice(&z.to_le_bytes()[..chunk.len()]);
        }
        Ok(())
    }
}
