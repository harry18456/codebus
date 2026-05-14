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

## Proposed fix

在 Settings modal 新增「PII Rules」區塊，讓 user 管理 extra regex patterns。

### UX

```
PII Rules
─────────────────────────────────────
[員工編號] [A\d{6}]  [刪除]
[內網 IP]  [10\.0\.\d+\.\d+]  [刪除]
[新增規則…]
```

- 每條規則：label（顯示用）+ regex pattern（Rust regex 語法）
- 儲存到 `~/.codebus/config.yaml` 的 `pii.extra_rules` 欄位
- App 啟動時 merge default patterns + extra_rules 傳給 scanner
- regex 驗證：輸入時即時 parse，invalid pattern 紅框提示

### Config schema

```yaml
pii:
  extra_rules:
    - label: "員工編號"
      pattern: "A\\d{6}"
    - label: "內網 IP"
      pattern: "10\\.0\\.\\d+\\.\\d+"
```

### Tasks（粗估）

1. `config.yaml` schema 加 `pii.extra_rules: Vec<PiiRule>`（label + pattern）
2. Settings modal 加 PII Rules 區塊（list + add / delete）
3. Regex validation（前端即時 parse + 錯誤提示）
4. `RegexBasicScanner` init 改為接受 extra patterns 參數
5. App 啟動讀 config → 傳 extra_rules 給 scanner
6. 單元測試：extra rule 正確 mask + invalid regex 不 crash

工程量：中（2 個半天）。

## Out of scope

- 不支援 custom scanner（只支援 regex，不允許 JS / Lua scripting）
- 不支援 per-vault extra rules（global only）
- 不提供 rule 匯出 / 匯入

## 依賴

- 建議與 `git-context-tool` backlog 同批做（兩者都是 PII 體驗改善）
- 可與 OpenAI Privacy Filter 整合（extra_rules 是 regex 層；privacy filter 是語意層，互補）

## 何時動

優先序中；可在 F `v3-app-polish-ship` 內或之後獨立做。
