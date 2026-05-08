//! Vendor-neutral schema content embedded into the binary at compile time.
//! Subsequent verb-specific workflow lives in skill bundles, not here.

pub const NEUTRAL_RULES: &str = include_str!("neutral.md");
