//! Terminal output rendering: banners, capability detection, lint text formatting.
//!
//! v3-render-polish: replaces v3's bare `println!("✓ ...")` calls with a
//! structured banner system carrying the codebus brand identity (the bus /
//! boarding metaphor). Adds emoji ↔ ASCII fallback, ANSI color, and OSC 8
//! hyperlinks driven by [`RenderOptions`] capability detection.
//!
//! Module shape: plain enum + plain struct + free functions. No trait, no
//! factory — single terminal target means an `EventRenderer` abstraction
//! would be speculative (anti-pattern per `feedback_dont_speculative_abstract`).

pub mod banner;
pub mod lint_text;
pub mod options;

pub use banner::{Banner, format_banner, print_banner};
pub use lint_text::format_lint_text;
pub use options::RenderOptions;
