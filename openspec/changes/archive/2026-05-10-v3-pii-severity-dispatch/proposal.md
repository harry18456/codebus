## Why

UV repo 驗收（`docs/v3-uv-verification-2026-05-10.md` finding #1）暴露：v3-config 把 `pii.on_hit` default 從 `warn` 改為 `mask` 的決定，**對 docs / test 內容過於激進** —

- uv repo 1289 個檔案觸發 672 PII matches，**多數是無害內容**：`127.0.0.1`（CONTRIBUTING.md docs）、`example@... ` email（test 的 `pyproject.toml` author）、test fixture data
- 這些 false-positive 全被 mask 為 `[REDACTED:ipv4]` / `[REDACTED:email]`，**降低 raw mirror 對 wiki agent 的可讀性**
- 真實 credential（AWS key、Anthropic API key）和 false-positive 一視同仁，沒有區分

`PiiSeverity` enum 早就把 match 分兩類（`Critical` 給 credential，`Warn` 給 contextual PII），但 `OnHit` 政策無視這分類。本 change 讓 severity 變 load-bearing — Critical 強制保護，Warn 給 user 控制。

## What Changes

- **`OnHit::Mask` / `OnHit::Skip` / `OnHit::Warn` 政策改為「只對 Warn 嚴重度生效」**：raw_sync 內部 dispatch — Critical-severity 命中**永遠走 Mask 行為**（`[REDACTED:<pattern>]` 取代）不接受 user override；Warn-severity 命中才依 `pii.on_hit` 決定 warn / skip / mask。
- **default `pii.on_hit` 從 `mask` 改為 `warn`**（**BREAKING** — 第二次翻 default，從 v3-config 那次改起算）：實際效果 — Critical 仍 mask（更安全於 v3-pii 原始 warn-only default），Warn 回到 warn-only（v3-pii 原始行為，docs / test 友善）
- **PiiSummary banner 的 `action` 欄位改為複合字串**：呈現實際 dispatch 結果，例如 `action critical=mask, warn=warn`（current 是單一 `action mask`）。讓 user 一眼看出 Critical / Warn 各走哪條路。
- **starter `~/.codebus/config.yaml` 註解更新**：說明 `on_hit` 只控 Warn-severity；Critical 永遠 mask。
- **`on_hit_label()` 在 init.rs 中變成兩段式**（顯示 critical=X, warn=Y）；callers 不變只改 banner 內容。

## Non-Goals

- **不新增 config 欄位** — `pii.on_hit` schema 不變（仍 scalar `warn` / `skip` / `mask`），只改語意。避免把 schema 弄成 `on_hit: { critical: ..., warn: ... }` 的兩層結構（user 用不到 Critical 控制，硬塞會增加混淆）
- **不新增 severity tier** — 維持 closed enum `Critical` / `Warn`，不加 `Info` 之類
- **不允許 Critical user-configurable** — 從威脅模型角度，AWS / Anthropic key 進入 raw mirror 是 unacceptable risk；不開洞讓 user 關掉
- **不做 `patterns_exclude` config** — 相同問題的另一條解法（roadmap 提過），但與 severity dispatch 正交。如果 severity dispatch 後仍有 false-positive 困擾，再開 follow-up
- **不改 Warn / Critical 的 pattern 分類** — `RegexBasicScanner` 內建 4 條 pattern 的 severity 維持不變（aws-access-key=Critical、anthropic-api-key=Critical、email=Warn、ipv4=Warn）
- **不影響 v3-pii 的 `OnHit` enum 定義** — 三 variant 仍存在，只是 dispatch 邏輯加 severity 分流

## Success Criteria

- 對 UV repo 跑 `codebus init`：raw mirror 內 `127.0.0.1` 與 `alice@example.com` **保持原貌**（不再被 `[REDACTED:...]` 取代），但若植入 `AKIAIOSFODNN7EXAMPLE` 仍被 mask。Verifiable：cli 測試比對特定檔案內容
- `PiiSummary` banner 顯示 `action critical=mask, warn=warn`（default）或 `action critical=mask, warn=mask`（user override `on_hit: mask`）
- `pii.on_hit: mask` 設定下，UV repo raw mirror 行為與本 change 之前一致（user 想保留全 mask 仍可達成）
- 全 workspace `cargo test` 綠

## Impact

- Affected specs: `pii-filter` (modified), `cli` (modified — Banner Output for Verb Commands action field semantics)
- Affected code:
  - Modified:
    - codebus-core/src/vault/raw_sync.rs
    - codebus-core/src/config/pii.rs
    - codebus-core/src/config/global_starter.rs
    - codebus-cli/src/commands/init.rs
