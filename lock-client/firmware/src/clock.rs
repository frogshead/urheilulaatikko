use esp_hal::time::Instant;
use lock_client_core::lock::Clock;

/// Reconstructs wall-clock Unix time from a build-time epoch plus elapsed
/// monotonic time since boot.
///
/// **Known limitation:** the ESP32-C3 has no battery-backed RTC, and this
/// project deliberately has no network time source — the lock is meant to
/// keep working with no internet connection at all. So "now" here is only
/// ever `build epoch + time since this boot`: a power loss resets the clock
/// to the moment the firmware was *compiled*, not flashed or booted, and any
/// downtime between build and deployment is unaccounted-for drift. This is a
/// first-pass placeholder — a real deployment needs either a battery-backed
/// RTC module or a provisioning step that sets the correct time before each
/// boot.
pub struct SystemClock {
    boot_epoch: u64,
}

impl SystemClock {
    /// `boot_epoch` should be read as soon as possible after `esp_hal::init`,
    /// since it anchors every timestamp this clock will ever report.
    pub fn start() -> Self {
        let boot_epoch: u64 = env!("LOCK_CLIENT_BUILD_EPOCH")
            .parse()
            .expect("LOCK_CLIENT_BUILD_EPOCH must be a valid u64 (set by build.rs)");
        SystemClock { boot_epoch }
    }
}

impl Clock for SystemClock {
    fn unix_seconds(&self) -> u64 {
        self.boot_epoch + Instant::now().duration_since_epoch().as_secs()
    }
}
