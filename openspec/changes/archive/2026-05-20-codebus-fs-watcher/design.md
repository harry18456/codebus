## Context

codebus-app 目前是 vault 的「單一 owner」假設：所有狀態變動透過 GUI 自身觸發的 IPC 完成，外部編輯一律仰賴 user 手動切 tab / 重 mount 才被看到。實際使用中四種情境破壞此假設：

1. terminal 直接跑 `codebus goal "..."` 期間，GUI 想看 events.jsonl 即時 append
2. 外部編輯器（Obsidian / VS Code）改 `.codebus/wiki/*.md` 後，Wiki tree 與 preview 不同步
3. terminal 跑 `codebus quiz` 新建 attempt，Quiz history 不更新
4. Lobby 加開的 vault（外部 init）無法不重 mount Lobby 就看見

現有 Tauri runtime 已掛 `tauri-plugin-fs`（檔案 dialog 用），但沒有 fs watcher 基建。前端各 tab 的 store（`useWikiStore` / `useGoalsStore` / `useQuizStore` / `useVaultListStore`）均提供 `listX` / `refreshRuns` 等 reload API，可直接被 watcher event handler 觸發。

## Goals / Non-Goals

### Goals

- 在 codebus-app/src-tauri 引入 `notify` crate 並建立單一 `watcher.rs` seam，負責所有跨平台 fs 監聽
- 後端 per-path 合併 fs event 約 200 毫秒（debounce），確保前端對每個邏輯變動只收一個 event
- 暴露七種 Tauri event：`wiki-list-changed` / `wiki-page-changed` / `goals-changed` / `goal-run-changed` / `quiz-changed` / `quiz-attempt-changed` / `vault-list-changed`
- per-vault watcher 與 Workspace mount/unmount 綁定，Lobby watcher 與 app 同生命週期
- 前端共用 `useWatcherEvent` hook 統一訂閱 Tauri event，避免每個元件重寫 `listen()` / cleanup 樣板

### Non-Goals

- 不 watch `~/.codebus/config.yaml`（modal-as-snapshot pattern；watcher 會在 user 編輯到一半時清掉本地變動）
- 不 watch `.codebus/raw/code/`（source mirror，僅在下次 goal sync 對 GUI 有意義）
- 不 watch `.codebus/CLAUDE.md`（per-repo schema，agent 讀取，GUI 不直接呈現）
- 不解決外部與 GUI 同步 save 競態（mtime-based conflict detection 屬獨立 backlog）
- 不做檔案內容 diff / partial reload；event 觸發即整體重 fetch
- 不在 watcher 失敗時 fallback polling（fail loud：watcher 啟動失敗向 user 顯示一次性 toast 並停用該 vault 的 auto-refresh，user 仍可手動切 tab 重 load）

## Decisions

### D1：採用 `notify::RecommendedWatcher` 自動選擇平台 backend

選 `notify::RecommendedWatcher`（macOS FSEvents、Linux inotify、Windows ReadDirectoryChangesW），而非：

- 自寫各平台 binding：維護成本高、`notify` 已是 Rust 生態事實標準
- 純 polling fallback：discuss 階段已收斂直接走 watcher

跨平台已知差異於 D4 處理。

### D2：debounce 在後端、固定 200ms、per-path key

fs save 一次操作常觸發 2-5 個 raw event（temp file → rename、metadata update 等）。後端在 watcher.rs 維護 `HashMap<PathBuf, tokio::time::Instant>` 並以最後一次 raw event 起算 200ms timer 後才 emit Tauri event。

選 200ms 因：低於人眼可察覺延遲、足以合併 atomic-rename 序列、不會與下一次有意義的編輯衝突。Per-path 而非全域，因不同檔案的變動互不影響。

替代：前端 debounce — 拒絕，因每個 tab 都要重複實作且收到多 event 仍會浪費 IPC。

### D3：watcher 生命週期繫於 Workspace / Lobby state，而非 vault open / close

- Per-vault watcher：Workspace 元件 mount 時 invoke `start_vault_watcher(vault_path)`、unmount 時 invoke `stop_vault_watcher`
- Lobby watcher：app `setup` hook 啟動，watch `~/.codebus/app-state.json`，整個 app session 常駐
- watcher handle 存於 `AppRuntimeState`（既有 state 結構），與 active_runs / cancel flag 同檔

替代：跟著 vault 「最近開啟」清單常駐 watch 多 vault — 拒絕，浪費 OS resource（inotify watch 數有上限），且 v1 同時只 active 一個 vault

### D4：跨平台已知差異與處理

| 平台 | backend | 已知差異 | 處理 |
|---|---|---|---|
| Windows | ReadDirectoryChangesW | 部分編輯器 save 時短暫 file lock，read 立刻可能失敗 | 前端重 fetch 失敗時自動 retry 一次（500ms 後），仍失敗才顯示錯誤 |
| macOS | FSEvents | 路徑 coalescing（多檔變動可能 batch 成單目錄 event） | 收到目錄 event 時對該目錄做一次 `listPages` / `listRuns`（不依賴精確 path） |
| Linux | inotify | watch descriptor 上限（預設 8192） | 啟動時若 `notify::Watcher::new` 回 `ENOSPC` → 顯示一次性 toast 教 user 提升 `fs.inotify.max_user_watches`，不再 emit 該 vault 的 event |

acceptance 屬 F `v3-app-polish-ship` deferred registry；本 change tasks 只跑 Windows 必過。

### D5：事件 payload schema

```
wiki-list-changed   → no payload
wiki-page-changed   → { path: string }              // 絕對路徑
goals-changed       → no payload
goal-run-changed    → { run_id: string }            // started_at slug
quiz-changed        → no payload
quiz-attempt-changed → { slug: string, id: string }
vault-list-changed  → no payload
```

無 payload 的 event 由前端直接 reload 整份 list；有 payload 的 event 由元件比對自己關心的 key 決定是否 reload。

### D6：watcher 啟動失敗 → fail-loud、不 fallback

若 notify::Watcher::new 失敗（最常見是 Linux ENOSPC、或 macOS 缺少 file access permission），後端：

1. 不重試
2. 透過 `vault-watcher-error` event 通知前端
3. 該 vault session 不再有 auto-refresh
4. 前端顯示 toast：「自動重新整理已停用：<原因>」並提供 docs link

替代：watcher 失敗時自動退 polling — 拒絕，會讓 user 不確定當前處於哪種模式、且兩條路徑同時維護成本翻倍

## Implementation Contract

| 行為 / 介面 | 契約 |
|---|---|
| `start_vault_watcher(vault_path)` IPC | 對 `<vault>/.codebus/wiki/`、`.codebus/log/`、`.codebus/quiz/` 三個目錄遞迴 watch；同一個 vault 重複呼叫為冪等（先 stop 舊的） |
| `stop_vault_watcher(vault_path)` IPC | 釋放對應 watcher handle；對未啟動過的 vault 為 no-op |
| 七種 emit event | 名稱與 payload 嚴格依 D5；事件 emit 順序對應 fs 事件順序（per-path 內保序，跨 path 無保證） |
| Debounce | 每個 path 上一個 raw event 起算 200ms 內若有新 raw event 則重置 timer；timer 到期才 emit 對應 Tauri event 一次 |
| 平台 fallback | 不存在；watcher 啟動失敗 → emit `vault-watcher-error { reason }` 並停止該 vault 的所有監聽 |
| Lobby watcher | app 啟動時於 `setup` 內 spawn 常駐 watcher 對 `~/.codebus/app-state.json`，app 關閉時自動釋放 |
| 前端 `useWatcherEvent(event_name, handler)` | 用 `@tauri-apps/api/event::listen` 訂閱，回傳 cleanup function 給 React `useEffect` 用 |

scope 內：watcher.rs 模組、所有七種 event 的端到端流通、前端對應 store / 元件訂閱、`useWatcherEvent` 共用 hook、Cargo 加 `notify` 依賴

scope 外：手動 refresh 按鈕、配置 watch 範圍（hardcoded）、watcher 重啟邏輯、跨 vault 同時 watch、Settings live-reload、mtime conflict detection

## Risks & Mitigations

- **Linux inotify 上限不足**：D6 fail-loud + toast 提供 user 自助步驟；未來若成為大宗回報再評估遞迴方式優化
- **macOS coalescing 導致 over-reload**：D4 設計即「不假設精確 path」、整目錄 reload；reload 操作本身是 idempotent，多次無害
- **Windows file lock race**：前端重 fetch 失敗時 single retry 已涵蓋；極端情境 user 仍可切 tab 觸發重 mount
- **watcher 失敗時 user 不感知**：D6 toast + 持續顯示「自動重整已停用」狀態避免 silent broken
