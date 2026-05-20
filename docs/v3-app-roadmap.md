# codebus-app v1 Roadmap — 已併入 [`v3-roadmap.md`](v3-roadmap.md)

> 本檔已於 2026-05-20 併入 [`docs/v3-roadmap.md`](v3-roadmap.md) Stage 3。本檔保留為 stub 避免既有外連（active specs / archived changes / discussion docs 共 19 處引用）撲空。
>
> **動態更新請看 [`v3-roadmap.md`](v3-roadmap.md)。** 本檔不再維護。

## 8 條主序列（snapshot）

CLI 主線（`docs/v3-roadmap.md`）2026-05-10 全 ship 後，app 層 v1 切 8 條序列化 change（foundation + A + B + chat + C + D + E + F）。每一條都假設前一條已 archive；不是平行可換序。

| # | Change | Scope (one line) |
|---|---|---|
| 1 | `v3-app-foundation` | Tauri shell + IPC bridge + Lobby + Settings stub + Workspace stub + design system |
| A | `v3-goal-library` | 3 spawn verb 搬進 `codebus_core::verb::*`；invoke 加 callback + cancel |
| B | `v3-run-log-events` | RunLog `outcome` + per-run events.jsonl 持久化 |
| chat | `v3-chat-verb` | 新 CLI verb `codebus chat` multi-turn REPL + `/goal` promote |
| C | `v3-app-workspace-goal` | Vault Workspace 真內容：sidebar tabs + Wiki preview + Goal flow |
| D | `v3-app-chat-cmdk` | Cmd+K spotlight chat 抽屜（multi-turn + 引用 + Promote to goal） |
| E | `v3-app-quiz` | Quiz flow（pending / reviewing）+ 從 wiki page 觸發 / 評分 / 寫回 |
| F | `v3-app-polish-ship` | Release build / installer / E2E infra / 跨平台 macOS+Linux 驗收 |

切點背後的 rationale（2026-05-11 brainstorming → 2026-05-12 quiz/cmdk 拆分 → 2026-05-12 前插 A+B → 2026-05-13 前插 chat）以及完整 deferred acceptance registry / cross-platform policy / out-of-scope，**全部已搬進 [`v3-roadmap.md`](v3-roadmap.md)**。

## Discussion 紀錄

各條 change 的設計討論留在原 docs 不動：

- [`docs/2026-05-12-v3-app-workspace-goal-discussion.md`](2026-05-12-v3-app-workspace-goal-discussion.md)
- [`docs/2026-05-13-chat-verb-discussion.md`](2026-05-13-chat-verb-discussion.md)
- [`docs/2026-05-13-v3-run-log-events-discussion.md`](2026-05-13-v3-run-log-events-discussion.md)
- [`docs/2026-05-15-v3-app-quiz-discussion.md`](2026-05-15-v3-app-quiz-discussion.md)
- [`docs/2026-05-18-quiz-progress-redesign-discussion.md`](2026-05-18-quiz-progress-redesign-discussion.md)
