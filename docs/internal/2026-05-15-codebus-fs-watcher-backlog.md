# Backlog: .codebus 目錄即時監聽（fs watcher）

**Date:** 2026-05-15
**Surfaced during:** backlog 討論（v3-app-chat-cmdk apply 期間）
**Severity:** UX gap（外部修改 GUI 不感知）
**Owner:** harry
**Status:** parked

---

## 觀察

目前 GUI 的即時更新只涵蓋「由 GUI 自己 spawn」的操作：

| 情境 | 狀態 | 機制 |
|------|------|------|
| GUI spawn 的 goal → events 即時流 | ✓ | `goal-stream` Tauri event channel |
| GUI spawn 的 goal 完成 → Wiki tab 自動 relist | ✓ | `goal-terminal` → `useWikiStore` → `listPages` |
| GUI spawn 的 goal 完成 → Goals 列表自動 refresh | ✓ | `goal-terminal` → `useGoalsStore.refreshRuns` |
| GUI spawn 的 chat turn → 即時 stream | ✓ | `chat-stream` channel |

以下情境 GUI 完全不感知：

| 情境 | 現況 |
|------|------|
| 在 terminal 跑 `codebus goal "..."` | GUI 不知道有新 run，要手動切 tab 觸發重 load |
| 在 terminal 跑 `codebus chat` | 同上 |
| 在 Obsidian / VS Code 直接編輯 `.codebus/wiki/*.md` | Wiki preview 不會 reload |
| `.codebus/raw/` source mirror 被外部 sync | 下次 goal 才會 detect |
| 任何 `.codebus/log/` jsonl 被外部寫入 | GUI 看不到 |

## 為什麼現在沒做

- `v3-app-workspace-goal` 設計時的權衡：GUI 是 vault 的「owner」，外部修改是 edge case
- 未引入 `notify` crate（Rust fs watcher）或 `tauri-plugin-fs` watch API
- 跨平台 fs watcher 有不少坑（macOS FSEvents / Linux inotify / Windows `ReadDirectoryChangesW` 各自有 edge case）

## 三個選項

### A. 什麼都不做

維持現況，需要時手動切 tab 重 mount。對 v1「GUI 是唯一操作入口」的 user 沒有問題。

工程量：0。

### B. Polling refresh（輕量，推薦先做）

Wiki / Goals tab focus 時，或定時（30s）自動 call：
- `listPages` → 更新 wiki 列表
- `refreshRuns` → 更新 goals 列表

不感知具體是哪個檔案變了，只做 reconcile。

工程量：小（半天）。

### C. fs watcher（完整方案）

後端用 `notify` crate watch：
- `.codebus/log/` → 新 `.jsonl` 或現有 `.jsonl` append → emit `run-log-changed` Tauri event
- `.codebus/wiki/` → 任何 `.md` 新增 / 修改 → emit `wiki-changed` Tauri event

前端 store subscribe events → 精確 reload 對應資料（不是全量 refresh）。

涉及：
- `codebus-app/src-tauri/src/watcher.rs`：`notify` crate 整合，per-vault watcher lifecycle
- Watcher 跟著 Workspace mount / unmount 啟停
- 跨平台驗證（macOS FSEvents delay、Linux inotify 需 `inotify` feature、Windows RDCW latency）

工程量：中（2-3 個半天 + 跨平台驗證）。

## 建議

先做方案 B（polling），解決大多數外部修改的感知問題，成本極低。
方案 C 在 F `v3-app-polish-ship` 跨平台驗證階段一起評估，若 polling 不夠精確再升級。

## Tasks（方案 B，粗估）

1. `useWikiStore`：tab focus hook + 30s interval → call `listPages`
2. `useGoalsStore`：同上 → call `refreshRuns`
3. Workspace mount 時啟動 interval，unmount 時清除
4. 單元測試：focus / blur / unmount 各自觸發正確次數的 IPC call

## Tasks（方案 C，粗估）

1. spec ADDED `fs-watcher`：watch 範圍、event schema、lifecycle 規格
2. `notify` crate 整合（`codebus-app/src-tauri/Cargo.toml`）
3. `src-tauri/src/watcher.rs`：per-vault watcher + emit Tauri events
4. Workspace mount / unmount hook（啟停 watcher）
5. 前端 store 訂閱 `wiki-changed` / `run-log-changed` events
6. 跨平台驗證（macOS / Linux / Windows 各自 edge case）

工程量：中（2-3 個半天）。

## Out of scope

- `.codebus/raw/` 的 watch（source mirror 變動不影響 GUI 顯示）
- Watch vault config file（`~/.codebus/config.yaml`）—— 另一條獨立需求
- 跨 vault 的 watcher（一次只 watch active vault）

## 何時動

方案 B：可在 E `v3-app-quiz` 之後、F 之前獨立做，工程量小影響面低。
方案 C：F `v3-app-polish-ship` 跨平台驗證階段一起評估。
