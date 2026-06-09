# Backlog: Settings 缺少 chat verb 的 model / effort 設定

**Date:** 2026-05-14
**Surfaced during:** backlog 討論（v3-app-chat-cmdk apply 期間）
**Severity:** UX gap（設定不透明）
**Owner:** harry
**Status:** parked

---

## 觀察

`v3-chat-verb` 設計時決定 chat 沿用 query 的 model / effort：

```rust
// codebus-core/src/verb/chat.rs:15-16
// "reuses query model / effort by design — chat is read-only exploration,
//  no dedicated config section at v1"
```

結果是：

- Settings Endpoint Section 只有 `goal` / `query` / `fix` 三列，**沒有 `chat`**
- User 不知道 chat 用哪個 model
- 調整 query 的設定會意外影響 chat

## 問題層面

### 透明度問題

User 開 Settings 看到三個 verb 的 model 設定，不知道 chat 用什麼。
至少應該顯示「Chat: 沿用 query 設定（目前：haiku-4-5 / low）」這樣的 read-only hint。

### 長期設計問題

Chat 是 multi-turn read-only exploration，query 是 single-shot lookup——兩者的 optimal model 可能不同：

| Verb | 特性 | 推薦 model |
|------|------|-----------|
| query | 單次問答，cost 敏感 | haiku（快、便宜）|
| chat | 多輪對話，reasoning 重要 | sonnet（品質較高）|

沿用 query 設定在 v1 是合理的省事決定，但 v2 可能需要獨立。

## Proposed fix 選項

### 方案 A（輕量）：read-only hint

在 Settings Endpoint Section 加第 4 列 `chat`，但顯示為 read-only：

```
chat    [沿用 query：haiku-4-5 / low]    (不可編輯)
```

讓 user 至少知道行為，不需改 config schema。

### 方案 B（完整）：獨立 chat config

`config.yaml` 加 `claude_code.system.chat`，`SystemProfile` / `AzureProfile` 加 `chat` 欄位。
Settings 加第 4 列可編輯的 model / effort dropdown。
`chat.rs` 改讀 `cc_cfg.resolve(Verb::Chat)` 而不是 fallback to query。

涉及：
- Rust config schema 修改（`SystemProfile` + `AzureProfile`）
- `ipc.ts` `SystemProfile` / `AzureProfile` interface 加 `chat` 欄位
- `VERBS` 陣列從 `["goal","query","fix"]` 改為 `["goal","query","fix","chat"]`
- `SYSTEM_PROFILE_DEFAULTS` 加 `chat` 預設值
- `EndpointSection.tsx` 自動 render 第 4 列（不需改 template，VERBS loop 即可）
- 測試更新

### 推薦

方案 A 先做（透明度問題最小代價解決），方案 B 在 v2 multi-provider 或 user 反映 chat 需要不同 model 時再做。

## Tasks（方案 A，粗估）

1. `EndpointSection.tsx`：chat read-only hint 列（不進 VERBS loop，獨立 row）
2. 顯示目前 query 的 model / effort（live，跟 query row 聯動）
3. tooltip 說明「chat 沿用 query 設定」

工程量：輕（半天）。

## Tasks（方案 B，粗估）

1. Rust `SystemProfile` / `AzureProfile` + serde schema 加 `chat` 欄位（含 migration default）
2. `ipc.ts` interface + `VERBS` 更新
3. `SYSTEM_PROFILE_DEFAULTS` 加 `chat: { model: "sonnet-4-6", effort: "low" }`
4. 測試更新（EndpointSection.test.tsx / SettingsModal.test.tsx）
5. `chat.rs` 改讀獨立 chat config 而非 fallback

工程量：中（1-2 個半天）。

## Out of scope

- Azure profile 的 chat 欄位（若做方案 B 需一起加，但 azure chat 是否合理需另評估）

## 何時動

方案 A 可在 `v3-app-chat-cmdk` tasks 7.1 Workspace 整合後順帶做（Settings UX 完整性）。
方案 B 等 v2 需求明確後再動。
