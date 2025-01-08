use std::time::{SystemTime, UNIX_EPOCH};
use totp_embed::{totp, totp_custom, Sha512};
fn main() {

// Negotiated between you and the authenticating service.
let password: &[u8] = b"top secret";

// The number of seconds since the Unix Epoch.
let seconds: u64 = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

// Specify the desired Hash algorithm via a type parameter.
// `Sha1` and `Sha256` are also available.
let result: u64 = totp::<Sha512>(password, seconds);
let r =   totp_custom::<Sha512>(30,6, password, seconds);
println!("code: {result}");
println!("code: {r}");
}
