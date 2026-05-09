//! Session continuity helpers for the fix outer ping loop.
//!
//! v3-lint uses `claude -p --session-id <uuid>` to mark a session, then
//! `claude -p --resume <uuid>` for follow-up pings. The agent retains
//! conversation context across pings, so subsequent rounds don't re-pay
//! the prompt-tokens cost of context the agent already saw.
//!
//! UUIDs MUST be valid v4 strings (claude's `--session-id` requires UUID
//! format per `claude --help` v2.1.137).

use std::sync::atomic::{AtomicU64, Ordering};

/// Generate a fresh v4 UUID string suitable for `--session-id`.
///
/// Implementation note: we don't pull in the `uuid` crate just for this —
/// session IDs are ephemeral (loop-scoped, never persisted) so a small
/// std-only generator using process state + time + atomic counter is
/// sufficient. If session IDs ever need to be reproducible across runs
/// (e.g. for debugging), this can be replaced with `uuid::Uuid::new_v4`.
pub fn new_uuid() -> String {
    // 16 bytes = 128 bits of state. Mix sources of entropy:
    //   - high-resolution monotonic time (nanos since epoch)
    //   - process id
    //   - per-call atomic counter (defends against same-nanosecond calls)
    use std::time::{SystemTime, UNIX_EPOCH};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let pid = std::process::id() as u64;
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);

    // Mix fields into 16 bytes via splitmix64.
    let mut bytes = [0u8; 16];
    let mut s0 = nanos.wrapping_add(0x9E3779B97F4A7C15);
    let mut s1 = pid.wrapping_add(counter).wrapping_add(0xBF58476D1CE4E5B9);
    s0 = splitmix64(s0);
    s1 = splitmix64(s1);
    bytes[..8].copy_from_slice(&s0.to_le_bytes());
    bytes[8..].copy_from_slice(&s1.to_le_bytes());

    // Set version (4) and variant (RFC 4122) bits.
    bytes[6] = (bytes[6] & 0x0F) | 0x40; // version 4
    bytes[8] = (bytes[8] & 0x3F) | 0x80; // variant 10xx

    format_uuid(&bytes)
}

fn splitmix64(mut x: u64) -> u64 {
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D049BB133111EB);
    x ^ (x >> 31)
}

fn format_uuid(b: &[u8; 16]) -> String {
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[0], b[1], b[2], b[3],
        b[4], b[5],
        b[6], b[7],
        b[8], b[9],
        b[10], b[11], b[12], b[13], b[14], b[15],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn new_uuid_format_is_8_4_4_4_12_hex() {
        let u = new_uuid();
        assert_eq!(u.len(), 36);
        let groups: Vec<&str> = u.split('-').collect();
        assert_eq!(groups.len(), 5);
        assert_eq!(groups[0].len(), 8);
        assert_eq!(groups[1].len(), 4);
        assert_eq!(groups[2].len(), 4);
        assert_eq!(groups[3].len(), 4);
        assert_eq!(groups[4].len(), 12);
        for g in groups {
            assert!(g.chars().all(|c| c.is_ascii_hexdigit()), "non-hex in `{u}`");
        }
    }

    #[test]
    fn new_uuid_version_4_bit_is_set() {
        // 13th hex char (offset 14 in dashed form) is the version nibble.
        let u = new_uuid();
        let version = u.chars().nth(14).unwrap();
        assert_eq!(version, '4', "expected v4 UUID, got `{u}`");
    }

    #[test]
    fn new_uuid_variant_bits_are_rfc4122() {
        // 17th hex char (offset 19) variant nibble: must be 8, 9, a, or b.
        let u = new_uuid();
        let variant = u.chars().nth(19).unwrap();
        assert!(
            "89ab".contains(variant),
            "expected RFC4122 variant nibble (8/9/a/b), got `{variant}` in `{u}`"
        );
    }

    #[test]
    fn new_uuid_produces_distinct_values_across_calls() {
        let mut seen = HashSet::new();
        for _ in 0..1000 {
            let u = new_uuid();
            assert!(seen.insert(u.clone()), "duplicate uuid: {u}");
        }
    }
}
