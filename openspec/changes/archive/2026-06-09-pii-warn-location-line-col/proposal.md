## Summary

Change the PII warn sink location from a raw byte offset to a human-readable `line:col` so the warning reads like the conventional `file:line:col` notation instead of a misleading large number.

## Motivation

The warn line format `pii warn: <pattern_name> at <relative_path>:<byte_offset>` prints the match's byte offset after the colon. This visually matches the universal `file:line` convention but is actually a byte offset, so a 367-line file reports `:18609`, which reads as a nonexistent line number and confuses the reader. The warn is functionally correct; only its location rendering is misleading.

## Proposed Solution

When emitting a warn line, convert the match's start byte offset into a 1-based line number and 1-based column by counting newlines in the already-in-memory file content up to that offset, and emit `<relative_path>:<line>:<col>`. The conversion runs only on the warn (cold) path — files with zero matches do nothing extra — and is at most an O(n) newline count over content the scanner already walked, so it does not change scan complexity.

## Non-Goals

- No change to PII detection, severity classification, match sorting, or redaction/mask behavior.
- No change to the `PiiMatch` struct: `start`/`end` remain byte offsets (they are correct as slicing indices and used by the masker); only the warn sink's rendered location changes.
- Not introducing a column-counting unit that is grapheme- or UTF-16-aware; column is a 1-based count of bytes is rejected — column is defined as the 1-based char (Unicode scalar) count within the line for human readability.
- Not suppressing or excluding any path (e.g. `openspec/`) from scanning — that is a separate scope.

## Alternatives Considered

- **Keep byte offset, relabel as `@<offset>`**: cheaper but still forces the reader to map an offset to a location; `line:col` is the format developers already expect from tooling.
- **Emit `line:col:byteoffset`**: redundant; the byte offset has no value to a human reading the warning.

## Impact

- Affected specs: pii-filter (Modified — warn sink location format)
- Affected code:
  - Modified: codebus-core/src/vault/raw_sync.rs (warn line formatting + the doc comment describing the format; add a byte-offset→line:col helper)
  - New: (none)
  - Removed: (none)
