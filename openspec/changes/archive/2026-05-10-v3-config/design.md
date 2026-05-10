## Context

`~/.codebus/config.yaml` 目前只有 `lint.fix.enabled` 一條欄位，由 `codebus-core/src/config/lint_fix.rs` 載入。三個現實 gap：

1. **PII 行為硬寫死**：`pii-filter` capability 明文「SHALL NOT read `~/.codebus/config.yaml`」（spec line 161）；`OnHit::{Skip, Mask}` 已定義但無 implementation（v3-pii hardcode `Warn`）；`RegexBasicScanner::new(patterns_extra)` API 已收參，但 init.rs / goal.rs 兩處 call site 都 hardcode `&[]`。
2. **Claude Code 算力一律 default**：`InvokeAgentOptions` 沒有 `model` / `effort` 欄位；`invoke()` 只下 `--tools` / `--allowedTools` / `--permission-mode` 三旗。三個 verb 算力需求差異大（goal 重 reasoning、query 走 retrieval、fix 中等）但目前都跑同一 default model。
3. **Discoverability 為零**：沒人主動寫 `~/.codebus/config.yaml`；user 不知道有 config 可以調，也看不到 default 值是什麼。`fix.rs` 的 graceful fallback 走 default，沒有訊息提示「你可以在 `~/.codebus/config.yaml` 改」。

Roadmap §4 #9 原本提的 `lint.disabled_rules` / `lint.custom_rules_dir` 是 0-consumer 抽象，本 change 明確去 scope。

## Goals / Non-Goals

**Goals:**

- 補 `pii.{scanner, patterns_extra, on_hit}` config 入口，並首次落實 `OnHit::{Skip, Mask}` 的 raw_sync 行為
- 補 `claude_code.{goal, query, fix}.{model, effort}` per-verb config，spawn 時 forward 為 `--model` / `--effort` flag
- `codebus init` if-missing 寫 starter `~/.codebus/config.yaml` 改善 discoverability
- 反轉 `pii-filter` 既有「不讀 config」requirement，所有 PII 行為改 config-driven

**Non-Goals:**

- 不做 `lint.disabled_rules`：5 條 rule 都是 wiki 健康度基本；0-consumer
- 不做 `lint.custom_rules_dir`：plugin / DSL / sub-process 三選一皆無 second-impl 驗證
- 不做 vendor `provider:` 抽象：違反「single-impl 不寫 spec」原則；交由 follow-up `v3-multi-agentic-provider` change
- 不做 type-mismatch graceful warn：serde_yaml parse 失敗時 caller 已有 stderr warning + default fallback 路徑，不再加複雜 warn 機制
- 不為了讀 config 主動建立 `~/.codebus/` 目錄；只在 init 流程內建。讀路徑採 NotFound → default 既有 pattern

## Decisions

### Per-verb claude_code config（非全域單組）

```yaml
claude_code:
  goal:  { model: opus,   effort: high }
  query: { model: haiku,  effort: low }
  fix:   { model: sonnet, effort: medium }
```

三 verb 算力需求差距大：

- goal 走 wiki ingest，重 reasoning，配 opus
- query 走 read-only retrieval，重響應速度，配 haiku
- fix 走 lint-and-edit loop，平衡型，配 sonnet

全域單組會在 query 過度浪費或 goal 不夠用之間擇一妥協。Per-verb 一次設好預設，user 仍可改個別 verb 而不影響其他。

**Alternative considered**：「全域 + per-verb override」（`claude_code.model: sonnet` 加 `claude_code.goal.model: opus` 個別蓋）— 三個 default 已是最常見配比，override 機制現階段沒有第三層需求，YAGNI 拒絕。

### `model` / `effort` 字串放行不硬列舉

`--model` 接受 `sonnet` / `opus` / 全名（`claude-sonnet-4-6`）等，集合會隨模型升版而變；`--effort` 目前是 `low/medium/high/xhigh/max` 五值但同樣可能擴充。codebus 不在自己的 schema 端做 enum 驗證 — 直接 `Option<String>` 透到 Claude CLI，由 Claude CLI 自己 validate / 報錯。模型升版時 codebus 不用改。

### `pii.scanner` 直接用既存 NullScanner 當第二 impl

`null` 模式不是新建 impl，是把 `init.rs` / `goal.rs` 在 source-signal drift detection / re-sync 階段建構 scanner 的 call site 改成 dispatch：

```rust
let scanner: Box<dyn PiiScanner> = match pii_cfg.scanner {
    PiiScannerKind::Null => Box::new(NullScanner::new()),
    PiiScannerKind::RegexBasic => Box::new(RegexBasicScanner::new(&pii_cfg.patterns_extra)?),
};
```

`NullScanner` 已存在於 `codebus-core/src/pii/scanners/null_scanner.rs` 為 test fixture。本 change 把它升格為使用者選項是 second-impl 真正落地。

### `OnHit::{Skip, Mask}` 由 raw_sync 接 OnHit 參數實作

`sync_with_scanner_into(repo_root, raw_code_dir, scanner, warn_sink)` 需要再加一個 `on_hit: OnHit` 參數（破壞性）。三條分支：

- `Warn`：保留現行行為 — `fs::copy(path, &dst)` + 對每個 match 寫 `pii warn: ...` 到 `warn_sink`
- `Skip`：scan 結果不空 → 不執行 `fs::copy`，但仍寫 warn line（讓 user 知道是哪些檔案被 skip）；`SyncSummary` 新增 `pii_skipped_files: usize` 欄位
- `Mask`：對每個 match 由後往前替換 `matched_text` 為 `[REDACTED:<pattern_name>]`（由後往前以保 byte offset 不漂移）；寫 mask 後 string 到 dst（用 `fs::write`，不再 `fs::copy`）；warn line 仍寫；`SyncSummary` 新增 `pii_masked_matches: usize` 欄位

**Mask 的非 UTF-8 邊界**：現行 raw_sync 在 `fs::read_to_string` 失敗時 fall through 到 `fs::copy` 直接 mirror binary。`Mask` 模式下若檔案非 UTF-8 走相同 fall-through（無法 mask 就完整 mirror），不視為錯誤；warn line 不寫（沒 match 可寫）。

### Starter writer primitive 在 core，orchestration 在 cli

```rust
// codebus-core/src/config/global_starter.rs
pub fn write_starter_config_if_missing(path: &Path) -> io::Result<StarterOutcome>;
pub enum StarterOutcome { Written, AlreadyPresent }
```

primitive 負責：

- `path.exists()` 檢查（`AlreadyPresent` 短路返回，不讀也不寫）
- `parent.exists()` 不存在則 `create_dir_all`
- 寫硬編碼 starter 字串（包含全部 default + inline comment）

orchestration（init.rs）：

- 用 `default_config_path()` 取路徑
- 呼叫 primitive
- 印「✓ global config: wrote ~/.codebus/config.yaml」/「✓ global config: ~/.codebus/config.yaml already present」

跟現有 `write_schema_file` / `write_manifest` / `INTERNAL_GITIGNORE_LINES` 完全同形。

### Tolerance 範圍：5 條（不做 type-mismatch）

延伸 `lint_fix.rs` 既有 pattern：

| 條件 | 行為 |
|---|---|
| File 不存在 | 全部 default，無 stderr |
| YAML parse fail | `Err(ConfigLoadError::YamlParse)` → caller 印 stderr warning 走 default |
| Section 缺（如沒 `pii:` key） | 該 section 全走 default |
| Section 內 key 缺 | 該欄位走 default |
| Unknown top-level key（如 user 寫 `llm:`） | serde 預設靜默忽略，forward-compat |
| Unknown discriminator（如 `on_hit: hyperflood`） | parse fail → caller 走 default + warning（serde_yaml 對 enum 不認識的 variant 預設報錯，已是合理行為）|

不做 type-mismatch graceful warn（v2 carry 的「`enabled: "true"` 字串值想 graceful 收下」）— 走 serde 預設 parse fail，caller fallback。

## Implementation Contract

#### Behavior summary

##### `~/.codebus/config.yaml` schema

```yaml
pii:
  scanner: regex_basic             # default: regex_basic（或 none — 不可寫 YAML null literal）
  patterns_extra: []               # default: []，append 到內建 4 條
  on_hit: mask                     # default: mask（v3-pii 為 warn — BREAKING）

claude_code:
  goal:
    model: opus                    # default: opus（任意字串，透傳 --model）
    effort: high                   # default: high（任意字串，透傳 --effort）
  query:
    model: haiku                   # default: haiku
    effort: low                    # default: low
  fix:
    model: sonnet                  # default: sonnet
    effort: medium                 # default: medium

# 既存欄位（不變）
lint:
  fix:
    enabled: true                  # default: true
```

所有欄位 optional；缺則 default。Top-level unknown key 靜默忽略。

##### `pii` 行為矩陣

| `pii.scanner` | `pii.patterns_extra` 生效 | `pii.on_hit` 生效 |
|---|---|---|
| `regex_basic` | yes | yes |
| `null` | no（NullScanner 不 scan，永遠空 vec） | no（沒 match 觸發 OnHit） |

##### `claude_code` 行為

`InvokeAgentOptions` 新增兩個欄位 `model: Option<String>` / `effort: Option<String>`；`invoke()` 在 `Some` 時 append `--model <X>` / `--effort <Y>` 到 argv，`None` 時不加（讓 Claude CLI 用自己的 default）。三個 verb 各自從 `claude_code.{verb}` 讀，傳給 invoke 的 `Option<String>` 是 `Some` 還是 `None` 視 config 而定（default 是 `Some`）。

##### `codebus init` 新步驟

在現有 init 流程中插入「寫 starter ~/.codebus/config.yaml」步驟，位置：在所有 per-vault 步驟之後（不影響既有 vault layout 寫入順序）。觀察行為：

- 第一次 init：stdout 多一行 `✓ global config: wrote ~/.codebus/config.yaml`
- 第二、三次 init（檔案已存在）：stdout 多一行 `✓ global config: ~/.codebus/config.yaml already present`
- 用 `--no-obsidian-register` 不影響本步驟（兩者獨立）

#### Interface / data shape

```rust
// codebus-core/src/config/pii.rs
pub struct PiiConfig {
    pub scanner: PiiScannerKind,
    pub patterns_extra: Vec<String>,
    pub on_hit: OnHit,
}
pub enum PiiScannerKind { RegexBasic, Null }
pub fn load_pii_config(path: &Path) -> Result<PiiConfig, ConfigLoadError>;

// codebus-core/src/config/claude_code.rs
pub struct ClaudeCodeConfig {
    pub goal: VerbAgentConfig,
    pub query: VerbAgentConfig,
    pub fix: VerbAgentConfig,
}
pub struct VerbAgentConfig {
    pub model: Option<String>,
    pub effort: Option<String>,
}
pub fn load_claude_code_config(path: &Path) -> Result<ClaudeCodeConfig, ConfigLoadError>;

// codebus-core/src/config/global_starter.rs
pub enum StarterOutcome { Written, AlreadyPresent }
pub fn write_starter_config_if_missing(path: &Path) -> io::Result<StarterOutcome>;

// codebus-core/src/agent/claude_cli.rs（modified InvokeAgentOptions）
pub struct InvokeAgentOptions {
    pub slash_command: String,
    pub vault_root: PathBuf,
    pub toolset: &'static [&'static str],
    pub bash_whitelist: Option<&'static str>,
    pub model: Option<String>,    // NEW
    pub effort: Option<String>,   // NEW
}

// codebus-core/src/vault/raw_sync.rs（modified signature）
pub fn sync_with_scanner_into<W: io::Write>(
    repo_root: &Path,
    raw_code_dir: &Path,
    scanner: &dyn PiiScanner,
    on_hit: OnHit,                // NEW
    warn_sink: &mut W,
) -> io::Result<SyncSummary>;

pub struct SyncSummary {
    pub files: u64,
    pub bytes: u64,
    pub pii_matches: usize,
    pub pii_skipped_files: usize,    // NEW
    pub pii_masked_matches: usize,   // NEW
}
```

#### Failure modes

- **Config file missing**：所有 loader 回對應 `Default::default()` 結構（`PiiConfig::default()` / `ClaudeCodeConfig::default()`），無 stderr
- **YAML parse fail**：caller 印 `warning: <section> config load failed (using defaults): <err>` 到 stderr，使用 default 繼續執行
- **`pii.scanner: regex_basic` 但 `patterns_extra` 內含無法編譯 regex**：`RegexBasicScanner::new` 回 `Err(regex::Error)` → caller 印 stderr warning 改用「無 patterns_extra 的 RegexBasicScanner」（不是 fallback 到 NullScanner — 內建 4 條仍可掃）
- **Starter writer 在 `~/.codebus/` 父目錄無法建立（權限）**：回 `io::Error`；init 印 stderr warning 但不 abort（其他 init 步驟仍跑）
- **`--model` / `--effort` 值不被 Claude CLI 接受**：spawn 行為由 Claude CLI 自己負責報錯；codebus 印 stderr 並 propagate 非零 exit code

#### Acceptance criteria

- 全部新檔案有對應 unit test（`#[cfg(test)] mod tests`）覆蓋三條路徑：default / file present / parse fail
- `sync_with_scanner_into` 三條 OnHit 分支各有 integration test 在 `codebus-core/tests/` 驗證實際檔案輸出（Warn 未動 / Skip 未寫 / Mask 內容已被 `[REDACTED:<pattern>]` 替換）
- `cli_routing.rs` 整合測試新增「init 後 `~/.codebus/config.yaml` 存在且 parse 為 default」
- `goal_flow.rs` 整合測試驗證「`claude_code.goal.model: foo` 在 config 時，spawn argv 含 `--model foo`」（透過 `CODEBUS_CLAUDE_BIN` 指向 echo wrapper 攔截 argv）
- `pii-filter` spec：line 161「SHALL NOT read config」requirement 移除；新增「Config-driven scanner / patterns / on-hit」三條 requirement
- `cli` spec：goal / query / fix subcommand requirement 新增「reads `claude_code.{verb}.{model, effort}` from config and forwards as `--model` / `--effort`」；init 新增「writes starter `~/.codebus/config.yaml` if missing」requirement

#### Scope boundaries

**In scope**：

- 上述 3 個新 config 檔 + 1 個新 module（global_starter）+ 6 個 modified 檔的全部行為
- `pii-filter` spec 反轉 + cli spec 增訂

**Out of scope**：

- `lint.disabled_rules` / `lint.custom_rules_dir`（明確 drop）
- vendor `provider:` 抽象（明確 drop，留給 follow-up change）
- type-mismatch graceful warn tolerance（明確 drop）
- 既存 `lint.fix.enabled` 欄位行為調整（不動）
- raw_sync 對 binary（非 UTF-8）檔案的處理改善（保留現行 fall-through）

## Risks / Trade-offs

- **`on_hit` default 從 `warn` 改 `mask` 是 BREAKING** → mitigation：proposal 明確標註 BREAKING；既有用戶在新 init 後行為改變但 raw mirror 內容更安全（密鑰被 redact 而非原樣同步）；user 可改 config 回 `warn`
- **Mask 模式對非 UTF-8 檔案無作用** → mitigation：fall-through 至原樣 copy（保留現行行為），文件化於 raw_sync.rs 模組註解；非 UTF-8 檔案本就少有人類可讀 secret
- **Starter writer 在 `~/.codebus/` 不存在時建目錄** → 副作用：init 改變了 user 的 home directory 結構。Mitigation：if-missing 守住，多次 init 不疊加；目錄 / 檔案皆 `~/.codebus/` 命名空間下；不改其他既有 home dotdirs
- **`patterns_extra` regex 編譯失敗 fallback 到「內建 only」而非 NullScanner** → 兩派看法：(a) 用戶意圖是「掃 PII」，掉 patterns_extra 仍掃內建 4 條 (b) 用戶寫了壞 regex 應 fail-loud。本 change 採 (a)：印 stderr warning 後降級至「內建 only」。理由：fail-loud 在 init 階段會 abort 整個 vault 建立，使用者已 commit 跑 init，不該被 config 一處筆誤擋住；stderr warning 已足以告知

## Migration Plan

無 schema migration（config 完全 additive）。Breaking 範疇限於：

1. **`OnHit` default 改 `mask`**：legacy v3-pii 行為是 warn；user 沒改 config 的話，下次跑 init / goal 時 raw mirror 內容會從「含原樣密鑰」變成「`[REDACTED:xxx]`」。文件化於 proposal What Changes 段
2. **`sync_with_scanner_into` signature 加 `on_hit` 參數**：core API 破壞性變更；call site 由本 change 同步更新（init.rs / goal.rs）

無回滾步驟 — config 是 additive，舊 binary 讀新 config 會落到 unknown-key 靜默忽略路徑。

## Open Questions

無；discuss 階段已收斂。
