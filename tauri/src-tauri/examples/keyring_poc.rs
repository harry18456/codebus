//! D-033 Change B Task 1.1 PoC — verify `keyring` crate 3.x round-trips
//! a sentinel value on the host OS keychain. Runs set → get → delete →
//! get-after-delete and asserts each step. Used to back the
//! `tauri-plugin-keyring` vs. `keyring-rs` selection decision recorded
//! in `docs/decisions.md` D-033.
//!
//! Run with:
//!   cd tauri/src-tauri
//!   cargo run --example keyring_poc
//!
//! Cross-platform validation lives in tasks.md task 12.5 (manual e2e
//! across macOS / Windows / GNOME Keyring); this PoC only proves the
//! dependency builds and round-trips on the host where it is run.

use keyring::Entry;

const SERVICE: &str = "codebus.poc.api_key";
const ACCOUNT: &str = "keyring-poc-sentinel";
const SENTINEL: &str = "sk-poc-sentinel-do-not-leak";

fn main() {
    println!("[poc] platform: {}", std::env::consts::OS);

    let entry = Entry::new(SERVICE, ACCOUNT).expect("Entry::new failed");

    entry
        .set_password(SENTINEL)
        .expect("set_password failed (does the OS keychain accept writes?)");
    println!("[poc] set ok");

    let read = entry.get_password().expect("get_password failed");
    assert_eq!(read, SENTINEL, "round-trip mismatch");
    println!("[poc] get ok");

    entry.delete_credential().expect("delete_credential failed");
    println!("[poc] delete ok");

    match entry.get_password() {
        Err(keyring::Error::NoEntry) => println!("[poc] post-delete NoEntry ok"),
        Ok(v) => panic!("[poc] expected NoEntry after delete, got value len={}", v.len()),
        Err(e) => panic!("[poc] unexpected error after delete: {e}"),
    }

    println!("[poc] PASS — keyring crate round-trip works on this host");
}
