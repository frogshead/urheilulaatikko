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

    // embedded-test's own linker script, which defines the section its
    // macros store test metadata in (so probe-rs can read the test list back
    // out of the ELF) and a marker symbol it uses to catch a missing
    // `-Tembedded-test.x`. Scoped to `-tests` specifically: the bare,
    // unscoped form of this instruction applies to every "supported target"
    // in the package (bins included), and the release binary links nothing
    // that defines that marker symbol, so applying it there would turn every
    // `cargo build --release` into a linker error.
    println!("cargo::rustc-link-arg-tests=-Tembedded-test.x");
    // embedded-test-macros emits `#[cfg_attr(rust_analyzer, ...)]` so IDEs
    // that run rust-analyzer can show a "run test" button; declaring the cfg
    // here stops `clippy -D warnings` (CI runs it) from treating rustc's
    // "unexpected cfg" lint as an error.
    println!("cargo::rustc-check-cfg=cfg(rust_analyzer)");
}
