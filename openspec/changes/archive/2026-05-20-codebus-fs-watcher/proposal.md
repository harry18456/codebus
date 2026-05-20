## Why

codebus-app v1 把 vault 視為「GUI 是唯一 owner」，所以外部編輯目前無法被即時感知。但實際使用情境裡，以下都是合理場景：terminal 直接跑 `codebus goal "..."` 期間 GUI 想看 events 即時 append、外部編輯 `.codebus/wiki/*.md`（Obsidian / VS Code）後 Wiki 列表跟 preview 沒同步、終端跑 `codebus quiz` 新建的 attempt 在 GUI Quiz history 看不到、Lobby 加開的 vault 沒手動切回 Lobby 就看不見。沒有 watcher 的結果是 user 必須切 tab / 重 mount 才看到變動，違反「GUI 持續呈現真實狀態」這個基本期待。

## What Changes

新增跨平台檔案系統 watcher 模組與 IPC event 通道，覆蓋三層粒度（list / item-content / lobby vault list）：

- 後端新增 `codebus-app/src-tauri/src/watcher.rs` 模組，使用 `notify` crate 的 `RecommendedWatcher`（macOS FSEvents / Linux inotify / Windows ReadDirectoryChangesW 自動選擇）
- per-vault watcher 跟 Workspace mount / unmount 綁定生命週期；Lobby 的 `~/.codebus/app-state.json` watcher 與 app 同生命週期常駐
- 後端 per-path 合併 fs event ~200ms（debounce），避免一次 save 觸發多次 emit
- emit 七種 Tauri event：
  - `wiki-list-changed`（`<vault>/.codebus/wiki/` 任一 `.md` 新增/刪除/改名）
  - `wiki-page-changed { path }`（特定 `.md` 內容變動）
  - `goals-changed`（`<vault>/.codebus/log/` 新增 `events-*.jsonl` 或 `runs-*.jsonl`）
  - `goal-run-changed { run_id }`（特定 run 的 events / runs jsonl 變動）
  - `quiz-changed`（`<vault>/.codebus/quiz/` 子目錄或檔案變動）
  - `quiz-attempt-changed { slug, id }`（特定 attempt 的 `<id>.md` 或 `<id>.progress.json`）
  - `vault-list-changed`（`~/.codebus/app-state.json` 變動）
- 前端 store 訂閱：`useWikiStore` 收到 `wiki-list-changed` → 重抓 list；`WikiPreview` 收到 `wiki-page-changed` 且 path 對應目前頁時重 fetch；`useGoalsStore` 收到 `goals-changed` → `refreshRuns`；`RunDetail*` 收到 `goal-run-changed` 且 `run_id` 對應目前 run 時重 fetch；`useQuizStore` 收到 `quiz-changed` → 重掃 history；`QuizAnswering` / `QuizReview` 收到 `quiz-attempt-changed` 且對應目前 attempt 時重 fetch；`useVaultListStore` 收到 `vault-list-changed` → 重 load
- Settings modal **明確不納入** watcher（modal-as-snapshot 模式：開啟一次性 load，外部編輯下次開自然看到；watcher 反而會打斷 user 編輯中的狀態）

## Non-Goals

- 不 watch `~/.codebus/config.yaml`（Settings 載入是 modal 一次性、watcher 會打斷編輯）
- 不 watch `.codebus/raw/code/`（source mirror 只在下次 goal sync 時對 GUI 才有意義）
- 不 watch `.codebus/CLAUDE.md`（per-repo schema，agent 讀取，GUI 不直接呈現）
- 不做 mtime-based conflict detection（save 競態問題另立 backlog；本 change 只負責「來自外部的變動觸發 GUI 重 load」）
- 不做檔案內容 diff / partial reload（變動一律觸發整體重 fetch；複雜度 / 效益不成比例）
- 不做手動 refresh 按鈕（watcher 自動處理；手動 refresh 是 fallback 議題，獨立）
- 不引入 long-poll / SSE 等替代方案（discuss 已收斂走 notify crate）

## Capabilities

### New Capabilities

- `fs-watcher`：跨平台檔案系統監聽基建，涵蓋七種 Tauri event 的 emit 契約、debounce 合併視窗、watcher 生命週期（per-vault / lobby-level）、跨平台 backend 選擇與失敗 fallback 行為

### Modified Capabilities

- `app-shell`：Lobby 訂閱 `vault-list-changed` 自動 reload vault 列表；Workspace mount / unmount 控制 per-vault watcher 啟停
- `app-workspace`：Wiki / Goals / Quiz 三個 tab 與其 detail view（WikiPreview / RunDetail* / QuizAnswering / QuizReview）訂閱對應 event 自動 reload

## Impact

- Affected specs: `fs-watcher`（new）、`app-shell`（modified）、`app-workspace`（modified）
- Affected code:
  - New:
    - codebus-app/src-tauri/src/watcher.rs
    - codebus-app/src-tauri/src/watcher_tests.rs（per-path debounce 與 lifecycle 單元測試）
    - codebus-app/src/hooks/useWatcherEvent.ts（前端 Tauri event 訂閱共用 hook）
  - Modified:
    - codebus-app/src-tauri/Cargo.toml（新增 `notify` 依賴）
    - codebus-app/src-tauri/src/lib.rs（registry 接 watcher 命令、Workspace mount/unmount 觸發點）
    - codebus-app/src-tauri/src/state/app_state.rs（保存 Lobby-level watcher handle）
    - codebus-app/src/store/wiki.ts
    - codebus-app/src/store/goals.ts
    - codebus-app/src/store/vaults.ts
    - codebus-app/src/components/workspace/WikiTab.tsx
    - codebus-app/src/components/workspace/WikiPreview.tsx
    - codebus-app/src/components/workspace/GoalsTab.tsx
    - codebus-app/src/components/workspace/RunDetailRunning.tsx
    - codebus-app/src/components/workspace/RunDetailDone.tsx
    - codebus-app/src/components/workspace/QuizTab.tsx
    - codebus-app/src/components/workspace/QuizAnswering.tsx
    - codebus-app/src/components/workspace/QuizReview.tsx
    - codebus-app/src/components/lobby/Lobby.tsx
  - Removed: (無)
- 跨平台：Windows / macOS / Linux 三平台 acceptance 屬 F `v3-app-polish-ship` deferred registry 範圍，本 change tasks 不在每平台必跑（沿 roadmap policy）
