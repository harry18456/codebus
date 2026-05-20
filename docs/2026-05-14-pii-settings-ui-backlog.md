# Backlog: PII 設定 UI（Settings 內加 extra regex rules）

**Date:** 2026-05-14
**Surfaced during:** backlog 討論（v3-app-chat-cmdk apply 期間）
**Severity:** feature gap（enterprise usability）
**Owner:** harry
**Status:** parked

---

## 觀察

目前 `RegexBasicScanner` 的 PII patterns 是 compile-time 硬寫在 Rust 原始碼中：

```rust
// codebus-core/src/pii/scanners/regex_basic.rs（示意）
const DEFAULT_PATTERNS: &[(&str, &str)] = &[
    ("email", r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}"),
    ("phone", r"\b\d{3}[-.\s]?\d{3}[-.\s]?\d{4}\b"),
    // ...
];
```

不同用戶 / 組織有不同的 PII 定義（員工編號格式、內部 project code、特定 domain email 等），目前無法自訂。

> **2026-05-19 schema 修正（discuss + settings-config-frontend apply）**：本文件原
> 提案的 `pii.extra_rules`（`{label, pattern}` 物件陣列）**與實作不符**。
> `codebus-core/src/config/pii.rs` 實際的 config key 是
> **`pii.patterns_extra`：純 regex 字串陣列（`Vec<String>`），無 label**。
> 2026-05-19 /spectra-discuss 決議：UI 直接對齊 `patterns_extra`、不引入 label
> （加 label 會破壞既有 CLI 讀取契約且需 migration，明確不做）。下方 UX /
> schema / Tasks 已據此更正。前端編輯器部分已於 `settings-config-frontend`
> change 完成（純 regex 字串列表 + 即時驗證）。後端 wiring **已確認早就串好**
> （grounded 核實：`codebus-cli/src/commands/init.rs` 與
> `codebus-core/src/verb/goal.rs` 皆 `RegexBasicScanner::new(&cfg.patterns_extra)`，
> 壞 regex 退 builtin，有測試覆蓋），故本 backlog **無剩餘後端工作**，視同
> 由 `settings-config-frontend` 收尾。完整盤點見
> `docs/2026-05-19-settings-config-coverage-backlog.md`。

## Proposed fix

在 Settings modal 新增「PII 額外規則」區塊，讓 user 管理 extra regex patterns。

### UX

```
PII 額外規則
─────────────────────────────────────
[A\d{6}]              [刪除]
[10\.0\.\d+\.\d+]     [刪除]
[新增規則…]
```

- 每條規則：單一 regex pattern 字串（Rust regex 語法），**無 label**
- 儲存到 `~/.codebus/config.yaml` 的 `pii.patterns_extra` 欄位（字串陣列）
- App 啟動時 merge default patterns + `patterns_extra` 傳給 scanner
- regex 驗證：輸入時即時 parse，invalid pattern 紅框提示 + 擋存檔

### Config schema

```yaml
pii:
  patterns_extra:
    - "A\\d{6}"
    - "10\\.0\\.\\d+\\.\\d+"
```

### Tasks（粗估）

1. ~~config.yaml schema 加欄位~~ — 已存在（`pii.patterns_extra: Vec<String>`，無需 schema 變更）
2. ~~Settings modal 加區塊 + regex validation~~ — **已於 `settings-config-frontend` 完成**（純字串列表 add/delete + 即時 parse + 無效擋存檔）
3. ~~`RegexBasicScanner` init 接受 extra patterns~~ — 已串（`init.rs` / `verb/goal.rs` 既有 `RegexBasicScanner::new(&cfg.patterns_extra)`）
4. ~~app 啟動讀 config → 傳 patterns_extra 給 scanner~~ — 同上，已串（sync 時生效）
5. ~~單元測試：extra rule 正確 mask + invalid regex 不 crash~~ — 已有（`custom_pattern_triggers_via_patterns_extra`、`init_with_bad_patterns_extra_falls_back_to_builtin`）

工程量：**無剩餘工作**（功能已端到端可用；本 backlog 可隨 `settings-config-frontend` 一併視為 close）。

## Out of scope

- 不支援 custom scanner（只支援 regex，不允許 JS / Lua scripting）
- 不支援 per-vault extra rules（global only）
- 不提供 rule 匯出 / 匯入

## 依賴

- 建議與 `git-context-tool` backlog 同批做（兩者都是 PII 體驗改善）
- 可與 OpenAI Privacy Filter 整合（`patterns_extra` 是 regex 層；privacy filter 是語意層，互補）

## 何時動

優先序中；可在 F `v3-app-polish-ship` 內或之後獨立做。
