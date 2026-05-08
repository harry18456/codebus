## Context

v3-init #2 archive 完成後 `.codebus/raw/code/` 是源碼 byte-for-byte 鏡射，無任何 PII 過濾。`codebus-core/src/vault/raw_sync.rs::sync_with_null_scanner` 函數命名借了 v2 概念但**內部完全沒呼叫 scanner**，是純 placeholder（檔頭 doc-comment 寫「PII filter wires in via change #3 v3-pii」）。

v2 `legacy/v2-rust/codebus-core/src/pii/` 有完整實作可 carry：`provider.rs`（trait + types）、`scanners/null_scanner.rs`、`scanners/regex_basic.rs`（含 4 條 builtin regex）、`factory.rs`（tagged-enum dispatch + serde）。本 change 選擇性 carry — 帶 trait + 兩個 scanner impl，**不**帶 factory / serde / Presidio / Aws。

vault 內容由 Claude Code agent 透過 goal / query / fix skill 讀取，安全敏感；secrets / API keys / emails 不該原封不動進 vault，是本 change 的 driver。

## Goals / Non-Goals

**Goals:**

- raw_sync 命中 PII 時 stderr 印 warn，**檔案仍 mirror**（`OnHit::Warn`）
- builtin 4 條 regex（`aws-access-key` / `anthropic-api-key` / `email` / `ipv4`）覆蓋常見 credential 洩漏
- `PiiScanner` trait + 兩個 impl（Null / RegexBasic）各自 unit test 覆蓋
- `codebus init` 終端輸出可見 PII match count

**Non-Goals:**

- 不接 `~/.codebus/config.yaml` config 入口；`patterns_extra` / `on_hit` 覆蓋是 #8 v3-config
- 不 carry v2 `ScannerConfig` enum / `build_scanner` factory / `on_hit_serde` 模組（#3 不需 serde 反序列化）
- 不 carry v2 `Presidio` / `Aws` 變體（speculative，零 impl，違反 §3 anti-pattern #1）
- 不切換 `OnHit::Skip` / `OnHit::Mask`：等 #8 開 config 入口
- 不動既有 5 MiB 檔案大小上限與 `.gitignore` honoring 行為
- 不對既存 vault 做 retroactive scan — 只在 init 流程的 raw_sync 階段 scan

## Decisions

### Scanner 構造位置：raw_sync 內部 hardcode

raw_sync 對外暴露 `&dyn PiiScanner` 參數，由 caller（`codebus-cli/src/commands/init.rs`）構造 `RegexBasicScanner::new(&[])` 傳入。raw_sync 自身不負責構造或選 default — 它是 mechanism，不是 policy。

**Rationale**：#3 caller 永遠傳 RegexBasic + 空 patterns_extra；#8 進來時 caller 改成從 config 構造，raw_sync 介面不動。

**Alternatives considered:**

- (A) raw_sync 內部 own 一個 `RegexBasicScanner` 實例：caller 無 wiring 責任，但 #8 進來時 raw_sync 要拆 — 違反「mechanism 不動，policy 上移」原則
- (B) carry v2 `build_scanner` factory：但 #3 沒有 ScannerConfig 反序列化路徑，factory 無消費者，純 dead code

### 函數重命名：`sync_with_null_scanner` → `sync_with_scanner`

現名在 #3 後語義謊言。新名直接表達「scanner-driven」+ 從簽名讀得出注入點：

```
pub fn sync_with_scanner(
    repo_root: &Path,
    raw_code_dir: &Path,
    scanner: &dyn PiiScanner,
) -> io::Result<SyncSummary>
```

caller `init.rs` 從 `sync_with_null_scanner(repo, &paths.raw_code)` 改成 `sync_with_scanner(repo, &paths.raw_code, &RegexBasicScanner::new(&[])?)`。

**Alternatives considered:**

- (A) 保留 `sync_with_null_scanner`：caller / test 免改，但語義誤導
- (B) `sync_repo_to_raw`（v2 carry 命名）：表達 mirror 動作但簽名不顯 scanner 注入

### `SyncSummary` 加 `pii_matches: usize` 欄位

```rust
pub struct SyncSummary {
    pub files: usize,
    pub bytes: u64,
    pub pii_matches: usize,
}
```

**Rationale**：init 進度行需要 PII count；獨立 return tuple 反而增加 caller 拆解。

**Manifest 不受影響**：`vault::manifest::compute_source_signal` 只讀 `summary.files` + `summary.bytes`，新欄位忽略 — manifest spec 既有「`file_count` 跟 `total_bytes`」requirement 不變動。

### stderr warn 格式：每個 match 一行，**不印 matched_text**

格式：`pii warn: <pattern_name> at <relative_path>:<byte_offset>`，例如 `pii warn: aws-access-key at src/aws.py:42`。

**Rationale**：terminal 顯示 secret 字面值（`AKIA…`）反而讓 secret 二度曝光（screen recording / shoulder surfing）。pattern_name + path + offset 足以給 user 定位來源。

**Alternatives considered:**

- 聚合一檔一行：失去 offset，user 要自己 grep
- JSON：過度，machine-readable 是 #6 v3-lint 的 `--json` 該管的事

### 不 carry `ScannerConfig` / `build_scanner` / `on_hit_serde`

v2 `factory.rs` 的 tagged-enum 4 變體（Null / RegexBasic / Presidio / Aws）+ serde 反序列化 + `on_hit_serde` 子模組整套不 carry。

**Rationale**：

- Presidio / Aws 變體零實作（return `FeatureNotCompiled`），撞 [feedback_dont_speculative_abstract](file:///C:/Users/harry/.claude/projects/D--side-project-codebus/memory/feedback_dont_speculative_abstract.md) — 沒 second impl 不寫進 code
- ScannerConfig serde 是「為了 config 反序列化」存在；#3 不接 config，無 consumer
- on_hit_serde 同理：#3 hardcode `OnHit::Warn` 用值，不需 wire format

**遷移路徑**：#8 v3-config 進來時新建 `pii::config` 模組（或 reuse `pii::factory` 命名空間），那時 config 是 consumer、Null + RegexBasic 是 second-impl，加 enum 跟 serde 不違反 anti-pattern。

### NullScanner carry：trait 二 impl 兼 test fixture

Carry NullScanner 滿足 anti-pattern #1（trait ≥ 2 impl）；同時 raw_sync 整合測試可注入 NullScanner 跑「無 PII」path 而不用建構 RegexBasic（regex 編譯雖然每次 test 都跑沒問題但 NullScanner 更直接）。**raw_sync default codepath 永遠用 RegexBasic**，NullScanner 不 wire production。

## Risks / Trade-offs

- [Risk] `OnHit::Warn` 模式下 stderr 大量 warn 可能淹沒 terminal（10000 個 email 命中） → Mitigation：v2 carry 同樣行為已實機驗證；#8 進來後 user 可切 `Skip` / `Mask`
- [Risk] RegexBasic email / ipv4 false-positive（`v1.2.3.4` 像 IP、`user@host.local` 像 email） → Mitigation：v2 已調過邊界（word boundary、要求 dot in TLD、IP 4 段），且 Warn 模式不阻塞 mirror；衝擊僅 stderr noise
- [Risk] regex compile 失敗（builtin 4 條都是 const literal，理論上不會） → Mitigation：unit test `RegexBasicScanner::new(&[])` 在 ci 跑過，compile 失敗在 ci 立刻擋
- [Trade-off] 不 carry factory.rs → #8 進來時新寫 dispatch；換來 #3 沒 dead code、沒 speculative variant
- [Trade-off] `SyncSummary` 加欄位是 source-breaking change → `codebus-core/tests/vault_init.rs` 既有斷言要更新；外部尚無消費者，影響面收斂

## Open Questions

- 進度行 PII count 為 0 時是否仍印？建議印（一致性 > 簡潔），格式 `✓ raw mirror: 142 files, 89 KiB, 0 PII matches`。tasks 階段定。
- stderr warn 即時 print 還是 buffer 到 raw_sync 結束？建議即時（v2 carry），避免大型 repo init 結束才湧出。tasks 階段定。
