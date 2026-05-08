//! End-to-end byte verification of the OSC 8 hyperlink pipeline.
//!
//! Spike-equivalent regression: confirms that when a `Thought` stream event
//! containing `[[wikilink]]` is rendered with a populated `RenderOptions`
//! (vault id + slug index + hyperlinks enabled + use_color enabled + terminal
//! advertised as supporting hyperlinks), the resulting stdout bytes contain
//! exactly the OSC 8 escape sequence + `obsidian://open?vault=...&file=...`
//! URI that Obsidian recognizes.
//!
//! This is the contract Windows Terminal / iTerm2 / VSCode integrated
//! terminals consume to make `[[wikilink]]` Ctrl+Clickable. Reverting any
//! one of (markdown styling, OSC 8 wrapping, slug-index resolution, vault
//! id wiring) would break this test.

use codebus_core::render::renderers::terminal::format_event_inner;
use codebus_core::render::RenderOptions;
use codebus_core::stream::StreamEvent;
use codebus_core::wiki::slug_index::{SlugIndex, SlugLocation};
use codebus_core::wiki::types::PageType;
use std::path::PathBuf;
use std::sync::Arc;

fn slug_index_with(slug: &str, loc: SlugLocation, rel: &str) -> Arc<SlugIndex> {
    let mut idx = SlugIndex::default();
    idx.insert_for_test(slug.to_string(), loc, PathBuf::from(rel));
    Arc::new(idx)
}

fn render_thought(text: &str, opts: &RenderOptions, hyperlinks_supported: bool) -> String {
    format_event_inner(
        &StreamEvent::Thought {
            text: text.to_string(),
        },
        opts,
        hyperlinks_supported,
    )
}

#[test]
fn full_pipeline_emits_obsidian_uri_for_resolvable_wikilink() {
    let opts = RenderOptions {
        use_color: true,
        hyperlinks: true,
        vault_id: Some("a38bcac8afd70c5e".to_string()),
        slug_index: Some(slug_index_with(
            "buddy-cli-commands",
            SlugLocation::Type(PageType::Concept),
            "concepts/buddy-cli-commands",
        )),
        ..RenderOptions::default()
    };

    let out = render_thought("see [[buddy-cli-commands]]", &opts, true);

    // Exact URI substring as it would appear in stdout.
    let expected_uri =
        "obsidian://open?vault=a38bcac8afd70c5e&file=concepts/buddy-cli-commands";
    assert!(
        out.contains(expected_uri),
        "missing exact obsidian URI; got: {out:?}"
    );

    // Byte-level confirmation that OSC 8 framing is present.
    // Open: `\x1b]8;;<URI>\x1b\\` ; Close: `\x1b]8;;\x1b\\`.
    assert!(
        out.contains(&format!("\x1b]8;;{expected_uri}\x1b\\")),
        "missing OSC 8 open sequence; got: {out:?}"
    );
    assert!(
        out.contains("\x1b]8;;\x1b\\"),
        "missing OSC 8 close sequence; got: {out:?}"
    );

    // Wikilink markdown styling (cyan + underline) survives inside the link.
    assert!(out.contains("\x1b[36m"), "missing cyan; got: {out:?}");
    assert!(out.contains("\x1b[4m"), "missing underline; got: {out:?}");
    assert!(
        out.contains("[[buddy-cli-commands]]"),
        "brackets must be preserved as visible text; got: {out:?}"
    );
}

#[test]
fn no_obsidian_register_path_emits_styling_only_no_osc8() {
    // Models the `--no-obsidian-register` opt-out path: no vault_id is
    // resolved by the run flow, so RenderOptions.vault_id stays None.
    let opts = RenderOptions {
        use_color: true,
        hyperlinks: true,
        vault_id: None,
        slug_index: Some(slug_index_with(
            "buddy-cli-commands",
            SlugLocation::Type(PageType::Concept),
            "concepts/buddy-cli-commands",
        )),
        ..RenderOptions::default()
    };

    let out = render_thought("see [[buddy-cli-commands]]", &opts, true);

    assert!(
        !out.contains("\x1b]8;;"),
        "OSC 8 escape must NOT appear when vault_id is None; got: {out:?}"
    );
    assert!(
        out.contains("\x1b[36m") && out.contains("\x1b[4m"),
        "styling must still apply; got: {out:?}"
    );
    assert!(
        out.contains("[[buddy-cli-commands]]"),
        "wikilink visible text preserved; got: {out:?}"
    );
}

#[test]
fn unsupported_terminal_emits_styling_only_no_osc8() {
    let opts = RenderOptions {
        use_color: true,
        hyperlinks: true,
        vault_id: Some("a38bcac8afd70c5e".to_string()),
        slug_index: Some(slug_index_with(
            "buddy-cli-commands",
            SlugLocation::Type(PageType::Concept),
            "concepts/buddy-cli-commands",
        )),
        ..RenderOptions::default()
    };

    let out = render_thought("see [[buddy-cli-commands]]", &opts, false);

    assert!(
        !out.contains("\x1b]8;;"),
        "OSC 8 escape must NOT appear when terminal does not support hyperlinks; got: {out:?}"
    );
    assert!(
        out.contains("\x1b[36m"),
        "cyan styling must still apply; got: {out:?}"
    );
}
