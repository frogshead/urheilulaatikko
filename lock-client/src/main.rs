use std::time::{SystemTime, UNIX_EPOCH};
use totp_embed::{totp_custom, Sha1};

fn main() {
    // Negotiated with totp-server via the provisioning endpoint.
    let password: &[u8] = b"top secret";

    // The number of seconds since the Unix Epoch.
    let seconds: u64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // SHA-1 / 6 digits / 30s step, matching totp-server (see
    // totp-server/src/totp/mod.rs) so both sides compute the same PIN.
    //
    // totp_custom returns the numeric value, not a zero-padded string, so it
    // must be formatted with leading zeros to match what totp-server hands
    // out and what the lock expects to see typed in.
    let code = totp_custom::<Sha1>(30, 6, password, seconds);
    println!("code: {code:06}");
}
