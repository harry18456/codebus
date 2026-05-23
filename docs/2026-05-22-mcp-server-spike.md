# T11 Spike：codebus 作為 MCP Server

**Date:** 2026-05-22
**Task:** loop T11（只讀探勘）
**背景:** [mcp-server backlog](2026-05-14-mcp-server-backlog.md)（2026-05-14, parked）

---

## TL;DR

backlog 設計合理且 provider-agnostic（無 PE2 耦合）。**目前仍卡兩個依賴未解**:
- **F**（v3-app-polish-ship / 穩定 IPC surface）— [T4 已確認](2026-05-22-github-repo-setup-spike.md) 未 archive。
- **RAG index**（T12）— 也是 parked,`wiki_search` 依賴它(可 grep fallback)。

且 **`rmcp` crate 尚未加入 deps**(全 workspace 無 mcp 相關 crate),所以動工需先評 protocol 層實作策略(rmcp vs 手寫 JSON-RPC)。

無實質新發現,backlog 維持 parked 等 F + RAG。

---

## 現況核對

1. **無 mcp/rmcp dep**:`grep "rmcp\|mcp" Cargo.toml*` 全空。新增依賴或自寫 protocol 是動工第一步。
2. **F 未到位**:[T4](2026-05-22-github-repo-setup-spike.md) 已確認 v3-app-polish-ship 沒 archive、`tauri.conf.json:31 bundle.active=false`。IPC surface 還在演化。
3. **可重用後端函數已具備**:
   - 讀 wiki page → `codebus-core` 的 wiki/verb 層已具讀取(query/chat read-only sandbox 用的就是這條)。
   - RunLog 讀取 → `log/verb_log.rs` 有 `write_run_log`；讀取面待確認。
   - vault list → 須對齊 `config` 的 vaults 結構(settings 端已有)。
4. **provider-agnostic**:MCP server 暴露的是 vault 資料/操作,**與 claude/codex 哪個 provider 無關**——不像 T2(parser 耦合 codex)或 T1(端點 UI 兩 provider)。乾淨任務。

## 對 backlog 的補充建議

backlog 已涵蓋大方向,僅補兩點:

1. **protocol 層先 spike**:rmcp 是 Anthropic 的 Rust SDK 但 release 成熟度需評估;若不穩,手寫 JSON-RPC 2.0 也只是中等工作(MCP 規範本身緊湊)。建議第一步單獨 spike 半天比較兩條路。
2. **wiki 讀取已可即時做**,不必等 F:`wiki_read` / `wiki_index` 只 wrap 現有 file read + 解析,跟 IPC surface 無關。如果想 incremental ship,**只暴露 query-only wiki 三件套（`vault_list / wiki_read / wiki_index`）+ grep-fallback `wiki_search`** 不依賴 F 也不依賴 RAG → 第一個能動的 MCP 版本可早於 F 落地。剩下的(`run_list` / 寫操作)等 F。

## 依賴與排序

照本 spike 補充,**incremental 落地路線**：
1. (現在可動) 三件 wiki 唯讀 tool + grep fallback search,protocol 層另 spike → 「MVP MCP」。
2. (F 後) `run_list` + IPC surface 對齊 → 完整 query-only。
3. (RAG 後) `wiki_search` 升級成語意搜尋。
4. (v2) 寫操作。

## 待 harry
此項仍維持 parked 為主。若哪天想做,**先試 MVP 路線**(現在就能動的三件唯讀工具),驗證 protocol 層 + MCP 接入流程,再等 F 補完整 surface,RAG 升級搜尋。
