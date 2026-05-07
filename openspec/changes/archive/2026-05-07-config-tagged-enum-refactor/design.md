## Context

codebus-core 有四個 plugin factory：`llm` / `pii` / `log` / `render`。每個 factory 都有「`Kind` enum 當 discriminator + 共用 flat `Config` struct」的形態，把 variant-specific 欄位攤在同一個 struct 裡靠 `Option<>` 表示「不適用」。

各 factory 目前 leak 程度：

| Factory | Variant 數 | 已實作 | 共用 struct 內 variant-specific 欄位 |
|---------|------------|--------|-----------------------------------|
| `llm`   | 4          | 1（ClaudeCli） | `binary_path` (CC), `api_key` (HTTP), `timeout_secs` (HTTP) |
| `pii`   | 4          | 2（Null, RegexBasic） | `patterns_extra`（RegexBasic 限定，但概念可放 Presidio 自訂 pattern） |
| `log`   | 3          | 2（Null, Jsonl） | `jsonl_dir`, `retention_days`（兩個都 Jsonl 限定） |
| `render`| 3          | 1（Terminal） | `terminal: RenderOptions`（100% Terminal 限定） |

Roadmap 上四個 factory 都會在後續階段（#1 token tracking → Otel sink、#2 multi-LLM → 三家 provider 變實作、Tauri app → Tauri renderer、Heavy-dep PII → Presidio/Aws）長出更多 variant-specific 欄位。每次 retrofit「flat → 加欄位」都會：

- 讓 `Config` struct 越來越多 `Option<>` 攤平欄位，可讀性降低
- 讓 reviewer 難判斷新欄位該放哪
- 變更 YAML schema、用戶要遷移 config.yaml

把這個 refactor 提前到 standalone change 做，後續每個 motivating feature change 是「在對應 variant 內加欄位」純加法，不再動底層 schema。

## Goals / Non-Goals

**Goals:**

- 四個 factory 的 config 都改成 serde-tagged enum，type system 強制 variant 邊界
- 所有現存 plugin runtime 行為對等（純 shape refactor）
- 所有現存 YAML 寫法繼續可用（discriminator key + 對應 variant 欄位的形態跟以前 flat 寫法一樣）
- loader 維持「unknown discriminator → warn + 退回 default」的容錯行為
- design.md 寫入「Tagged-enum config pattern」作為未來 plugin factory 加 variant 的範本

**Non-Goals:**

- 不加任何新欄位（model / effort / fallback / budget / endpoint 等留給後續 motivating feature change）
- 不改變 plugin trait surface（LlmProvider / PiiScanner / LogSink / EventRenderer 簽名不動）
- 不動非 factory 形態的 config（`LintConfig` / `AutoFixConfig` / `EmojiMode` / `GlobalConfig`）
- 不順便為 RegexBasic / Jsonl 之類已有 variant 加新欄位

## Decisions

### Tagged-enum config pattern

每個 plugin factory 採用以下統一 shape：

```rust
// 1. enum 取代 (Kind enum + flat Config struct) 雙胞胎
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "<discriminator>", rename_all = "snake_case")]
pub enum SomeConfig {
    VariantA { /* A 限定欄位 */ },
    VariantB { /* B 限定欄位 */ },
    // ...
}

// 2. Default 給「最保守 / 0.x.0 行為對齊」的 variant
impl Default for SomeConfig {
    fn default() -> Self { Self::VariantA { /* defaults */ } }
}

// 3. build_some(cfg) 直接 match variant 解構，不再透過 Kind discriminator
pub fn build_some(cfg: SomeConfig) -> Result<Box<dyn SomeTrait>, SomeError> {
    match cfg {
        SomeConfig::VariantA { .. } => Ok(...),
        SomeConfig::VariantB { .. } => Ok(...),
        // ...
    }
}
```

YAML 對應：

```yaml
section:
  <discriminator>: variant_a    # tag field
  field_x: ...                  # variant_a 的欄位（serde tag 模式自動 flatten）
```

加新 variant 的工序：

1. 在 enum 加 `NewVariant { /* 限定欄位 */ }`
2. 在 build 函數的 match 加對應 arm
3. 不需要動 YAML schema 容器，新 variant 自動 deserialize

**Rationale**：tagged enum 是 Rust 表達 sum type 最自然的方式；`#[serde(tag = "...")]` 讓 YAML 形態跟原 flat 寫法兼容；future 加 variant 是純加法、不動既有 variant；type system 保證「variant 不會多帶不該帶的欄位」。

**Alternatives considered**：

- **flat struct + Option fields**（現況）：優點是序列化淺、新人容易讀；缺點是 variant edge silent leak（用戶設了不該設的欄位無 type 警告）、隨 variant 多會雪球。否決。
- **flat struct + variant-specific sub-struct**（例：`Config { kind, claude_cli: ClaudeCliSettings, openai: OpenAiSettings, ... }`）：優點是序列化結構穩、子 struct 內 variant-specific 欄位有命名空間；缺點是仍無 type 強制（用戶可同時設 `kind: claude_cli` 跟 `openai.organization: x`，邏輯矛盾不被攔），且未來真要強制時還是要再 refactor 一次。否決。

### Loader 容錯行為的整合方式

`config/loader.rs` 既有契約（spec 「Load global config tolerantly」requirement）有兩條 fallback 行為：

- unknown discriminator value（`provider: gibberish`）→ warn + 整個 section 退回 default
- type-incompatible sub-field（`timeout_secs: "thirty"`）→ warn + **只那個 sub-field 退回 unset**（同 section 其他 valid sub-field 保留）

第二條是 **field-level 容錯**，現有 loader.rs 透過手動 `serde_yaml::Value` 走訪實作（每個 sub-field 各自 try-parse、失敗各自 warn 後 None）。**這是 codebus 既有 UX**，本 refactor 不應退步。

選擇實作方式：

**選 (i) 沿用手動 Value 走訪 + 直接輸出 tagged enum**

每個 plugin section 的 `parse_*` 函數結構不變，但輸出型別從中繼 flat struct（`LlmConfig` 等）改成 factory 的 tagged enum（`ProviderConfig::ClaudeCli { binary_path }` 等）。

```rust
fn parse_llm(v: &Value) -> Option<ProviderConfig> {
    // 1. 讀 discriminator (provider field)，dispatch 到 variant
    //    unknown value → warn + return None（整段退 default）
    //    missing → 用預設 variant
    // 2. 對該 variant 走訪剩餘 sub-fields，per-field try-parse + per-field warn
    //    field-level 容錯保留：bad timeout_secs 只讓 timeout_secs = None
    // 3. 構造對應 variant 並回傳 Some(variant)
}
```

**Alternatives**：

- **(ii)** 直接用 `serde_yaml::from_value::<ProviderConfig>` + catch error：簡單但**整段 catch** 會讓 type-mismatched 單一 sub-field 把整個 section 拉回 default，**field-level 容錯退步**。否決。
- **(iii)** 用 serde `#[serde(default, deserialize_with = "...")]` 為每個可能 fail 的 sub-field 寫自定義 deserializer：能保留 field-level 容錯，但每個 enum 的每個 fallible sub-field 都要寫一個 helper，總共 ~5 個 helper + 各 variant 上對應 attribute，樣板量多於 (i)。否決。
- **(iv)** 用 `#[serde(other)]` 接 unknown discriminator：對 struct variant 不適用（serde 限制只能 unit variant），需污染型別加 Unknown variant。否決。

選 **(i)**：既有 UX 完全不退步、loader 現有結構複用率最高（只是輸出型別換）、schema.rs 簡化（`GlobalConfig` 直接持有 tagged enum、不需要中繼 struct）天然對齊。中繼 flat struct（現有 `LlmConfig` / `PiiConfig` / `LogConfig` / `RenderConfig`）連同 `#[serde(skip)]` discriminator hack 整批移除。

### Default variant 的選擇

每個 enum 要 `impl Default`。原則：**選最保守、不需外部資源、與 0.2.0 行為對等的 variant**。

| Enum | Default variant | 理由 |
|------|----------------|------|
| `ProviderConfig` | `ClaudeCli { binary_path: None }` | 0.2.0 唯一實作；無需 API key 等外部資源 |
| `ScannerConfig` | `Null { on_hit: OnHit::Warn }` | 0.2.0 預設；不掃任何 PII |
| `SinkConfig` | `Null {}` | 0.2.0 預設；不持久化 |
| `RendererConfig` | `Terminal { options: Default::default() }` | 0.2.0 預設；唯一實作 |

### `on_hit` 在 ScannerConfig 內的定位

`on_hit` 概念上對所有 scanner 通用（每家 PII scanner 都需要決定 hit 後行為）。雖然 `Null` 從不 hit、`on_hit` 對它形同虛設，仍把 `on_hit` 放在每個 variant 內保持「scanner 自己的設定就在自己的 variant 結構裡」一致性，優於拉到 enum 外作為通用欄位。理由：

1. 一致性：每個 variant 自包含其所有設定
2. 未來彈性：若某 scanner 想覆寫 `on_hit` 預設（如 Aws Detect-PII 強制 mask），可在自己 variant 內處理
3. YAML 表達自然：`pii: { scanner: regex_basic, on_hit: skip, patterns_extra: [...] }` 的寫法不變

## Risks / Trade-offs

**Risk: YAML 微幅 BREAKING — 用戶把 variant-specific 欄位寫在錯 variant 下會被 serde 忽略**

譬如：

```yaml
llm:
  provider: claude_cli
  api_key: xxx          # 0.2.0 silently ignored；refactor 後仍 silently ignored（serde 預設行為）
```

→ Mitigation：**user-observable 行為其實對等**（兩種寫法都導致 api_key 不被使用）。差別在 type system 內部 —— 開發者寫 Rust code 時無法做出「provider: claude_cli + api_key 同時存在」的 ProviderConfig 值，這是改善而非倒退。文件不需要強調，scenarios 加一條覆蓋這個案例即可。

**Risk: design.md 文件化的 pattern 跟未來真實需求不符，導致範本失效**

→ Mitigation：(a) 已對 4 個 factory 都驗過 pattern 適用；(b) pattern 是 Rust + serde 慣用形態、業界廣泛採用，黑天鵝風險低；(c) 若未來某 factory 真有特殊需求（譬如需要動態 variant），可在那次 motivating feature change 偏離 pattern 並更新範本，不算 design.md 失效。

**Risk: refactor 範圍大（4 個 factory + 4 個 schema entry + loader + main mapper + tests）一次改完容易漏改某些 corner case**

→ Mitigation：(a) cargo check / cargo test 在 refactor 過程中持續驗證；(b) 既有測試覆蓋率高（256 tests），絕大多數 regression 會被現存 test 抓到；(c) tasks.md 拆成「逐 factory 一段」的 mechanical 順序，每段獨立可驗。

**Trade-off: 序列化形態的 verbosity vs type 安全**

tagged enum 的 YAML 跟 flat struct 對比：兩者寫法**完全一樣**（discriminator 還是同一個 key、欄位還是同層）。所以對用戶 YAML 寫作體驗 0 影響。Rust code 內部：建構 enum 比建構 struct 多一點點 ceremony（要選 variant），但換來 type 安全，划算。

## Migration Plan

不需要使用者主動遷移。所有現存合法 YAML 寫法都繼續通過解析。Variant-mismatched 欄位（前述 risk）原本就被 silently ignore，refactor 後行為對等。

部署：

1. 此 change archive 後，下個 release（預計 0.3.0-dev）含此 refactor
2. CHANGELOG 註明「Internal refactor: plugin config 使用 tagged enum；YAML 寫法不變」
3. 後續 motivating feature change（#1 / #2 / Tauri / Presidio）順著 pattern 加自己的 variant 欄位

## Open Questions

無。各設計決策（discriminator key 名、default variant、on_hit 歸屬、loader fallback 機制）已在「Decisions」段明確選擇並記錄理由。
