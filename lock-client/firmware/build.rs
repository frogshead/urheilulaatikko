use std::time::{SystemTime, UNIX_EPOCH};

/// Bakes the build's Unix timestamp into the firmware as `LOCK_CLIENT_BUILD_EPOCH`.
///
/// The chip has no battery-backed RTC and (deliberately) no network time
/// source, so this is the only "now" it starts from — see the limitation
/// documented on `clock::SystemClock` in `src/clock.rs`.
fn main() {
    let build_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is before the Unix epoch")
        .as_secs();
    println!("cargo:rustc-env=LOCK_CLIENT_BUILD_EPOCH={build_epoch}");
}
