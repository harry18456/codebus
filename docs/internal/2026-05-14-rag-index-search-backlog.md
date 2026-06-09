# Backlog: RAG index + search（LanceDB vector search）

**Date:** 2026-05-14
**Surfaced during:** backlog 討論（v3-app-chat-cmdk apply 期間）
**Severity:** feature gap（知識檢索品質）
**Owner:** harry
**Status:** parked

---

## 觀察

codebus wiki 目前靠 agent 自行 Read / Grep / Glob 探索頁面。
隨著 wiki 成長（頁面數 > 50），agent 需要的 explore tokens 增加，且語意相關但 keyword 不同的頁面會被漏掉。

v2-archive proposal 曾列 `LanceDB / vector RAG → phase 3+`，理由是「小規模 wiki 不需要」。
現在 app 即將 ship，wiki 使用量會成長，時機點值得重新評估。

## 三個 use case

```
wiki pages ──embed──▶ LanceDB index（~/.codebus/index/）
                           │
             ┌─────────────┼──────────────────┐
             ▼             ▼                  ▼
       codebus search  query pre-warm    chat session init
       （standalone）  （spawn 前注入）  （new session 注入）
```

### 1. Standalone search

`codebus search "query"` 回傳 top-K 相關 wiki pages（含 score + excerpt）。
App 層加 search UI（可能在 Workspace sidebar）。

### 2. Query pre-warm

`codebus query "..."` spawn 前：
1. embed query text
2. 找 top-3 wiki pages
3. 注入 agent system prompt：`Relevant pages: [[page1]], [[page2]]...`

Agent 不需要從頭 explore，token 更省、起點更準。

### 3. Chat session init

新開 chat session 時：
1. 無 query（chat 是 open-ended）→ 用最近 goal 的 topic 或 vault index 的摘要
2. 找 top-3 相關 pages 作為 first context message
3. Chat 開始前 assistant 已有知識背景

## 技術設計

| 元件 | 選擇 | 理由 |
|------|------|------|
| Vector DB | LanceDB（Rust crate `lancedb`） | embedded、無 server、跨平台 |
| Embedding model | `all-MiniLM-L6-v2`（ONNX） | 22M params、快、品質夠 |
| Index 更新 | goal 完成後 incremental update | 只 re-embed 新寫 / 修改的頁面 |
| Index 位置 | `<vault>/.codebus/index/` | per-vault 隔離 |

### Index 更新流程

```
goal 完成 → wiki 頁面寫入 → index_updater 掃 diff → 只 re-embed 變動頁 → upsert LanceDB
```

不跑 full re-index（除非用戶手動 `codebus index rebuild`）。

### Tasks（粗估）

1. spec ADDED `rag-index`：定義 index schema + update trigger + search API
2. `lancedb` + ONNX embedding crate 整合
3. `codebus-core/src/index/`：build / update / search
4. `codebus index rebuild` CLI command
5. Goal 完成 hook：incremental index update
6. `codebus search` CLI command
7. App search UI（Workspace sidebar 或 Cmd+K 整合）
8. Query pre-warm：`verb::query` spawn 前注入
9. Chat session init：`verb::chat` new session 前注入
10. Integration test：search round-trip + pre-warm token 節省驗證

工程量：重（1 週以上；embedding model 整合有未知風險）。

## Out of scope

- 不做 cross-vault search（per-vault 隔離）
- 不做 source code embedding（只 wiki pages）
- 不做 reranker（v1 vector similarity 已夠）
- 不做 streaming search result

## 依賴

- **after F**：wiki 需要有穩定內容可以 embed
- **MCP server** backlog 的 `wiki/search` operation 直接就是 standalone search 的 wrapper
- OpenAI Privacy Filter backlog 無直接依賴，但兩者都用 ONNX runtime → 可共用 build infrastructure

## 何時動

F `v3-app-polish-ship` archive 之後。
先評估 vault 實際頁面數，若 < 30 頁則繼續 parked；超過 50 頁時起 propose。
