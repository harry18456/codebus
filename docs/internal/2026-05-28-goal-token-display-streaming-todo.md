# TODO · Goal 執行中 token 顯示 UX 優化

> **Status: archived 2026-05-28** — fix landed in
> `openspec/changes/archive/2026-05-28-chatwidget-pulse-and-goal-token-display/`.
> A 路徑（Running 期間顯示 placeholder「—」、第一 Usage event 抵達後切真實累積值）已落實；
> spec `app-workspace § Run Detail Views — Running` 加 NOTE + 兩條 scenarios + example table；
> Claude + Codex 雙 provider CDP smoke verify（見 `codebus-app/scripts/.pulse-and-token-smoke/`）。
> B（Claude CLI incremental usage flag）+ D（estimated tokens）defer、不在本 change scope。

## 現象

Goal 執行過程中、token 計數一直顯示 **0**。直到 goal 完成、才一次性顯示完整 token usage。

UX 體感：user 看不到「現在花了多少」、無法 mid-flight 判斷成本 / 進度。

## 技術背景

`StreamEvent::Usage` 來自 Claude CLI 的 `{"type":"result", "usage":{...}}` line、**一個 spawn 整段 stream 只 emit 一次**、且是在 result 階段（即 agent 結束時）。

→ 不是 codebus bug、是 Claude stream-json 格式本身的限制（usage 在 final result event 才給）。

## 優化方向（待評估）

| 方向 | 內容 | 工 |
|---|---|---|
| **A. 隱藏 0** | Running 期間 token 顯示「—」或「計算中…」、不顯示誤導的「0」 | 小（純 frontend）|
| **B. Stream-incremental usage**（若 Claude 支援） | 看 Claude CLI 是否有 `--show-usage-per-turn` 或同等 flag、每次 tool turn 都 emit usage | 中（需查 Claude docs + 改 backend parser）|
| **C. Estimate 顯示** | 從 user input + tool_use input 估算大概 token、加 ~ 前綴提示是估算 | 中（estimator 邏輯 + 容易誤導） |
| **D. Codex provider 是否同樣問題** | 5.3 提到 codex `turn.completed` emit usage、可能 per-turn 不是 per-spawn | 待驗 |

## 建議起手

1. Verify 現況：開 goal、看 RunDetailRunning 上的 token display source 是 frontend 哪個 component / store field
2. 查 Claude CLI 文件 / `--help` 看有沒有 incremental usage flag
3. Codex 同樣場景比較（per `project_stream_event_tool_kind_lessons` codex 經驗）
4. Design: 短期走 A（不誤導）、長期若 B 可行再走

## Priority

中（user-visible UX、不阻塞功能）。

跟 [[2026-05-28-four-bugs-backlog]] 的 4 bug 同 batch、考慮收完 bug 3/4 後、bug 2 之前順手做（chat 跟 goal 都有 token display、shared concern）。

## 不在 scope

- 改 Claude CLI 行為（不可控）
- Token cost estimation logic（accuracy 風險）
