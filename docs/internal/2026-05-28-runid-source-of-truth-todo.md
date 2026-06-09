# 2026-05-28 · RunId source-of-truth latent issue · TODO

## 背景

`backend-cleanup-codex-websearch-and-runid-millis` change (2026-05-28 archive)
把 IPC 層 + verb 層 共 7 處 timestamp 派生從 `SecondsFormat::Secs` 改 `Millis`、修好 active_runs key 同秒碰撞 + orphan-detection invariant break 兩個 bug。

但這次 fix **沒**根本消除 latent issue：RunId 的 source of truth 仍**散在兩處**獨立派生。

## Latent issue

```
IPC 層 (codebus-app/src-tauri/src/ipc/)
├── goals.rs::goal_run_id()      → 派生 active_runs key (millis)
├── chats.rs::chat_run_id()      → 派生 active_runs key (millis, chat- 前綴)
└── quiz.rs::quiz_run_id()       → 派生 active_runs key (millis, quiz-...- 前綴)

Verb 層 (codebus-core/src/verb/)
├── goal.rs:140  run_started_at  → 派生 events file slug + RunLog.started_at
├── chat.rs:123  run_started_at  → 同上
├── query.rs:80  run_started_at  → 同上
├── fix.rs:105   run_started_at  → 同上
└── quiz.rs:495  run_started_at  → 同上 (quiz_generate)
```

兩處皆呼 `chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)`、靠時間
接近 + 同 SecondsFormat 約定維持「IPC 派生 RunId」與「verb 派生 events file slug」
**string 一致**。

實際上兩次 `now()` 呼叫之間**仍可能跨 ms 邊界**、兩個 string 會差 1ms。`active_runs.get(events_slug)` 仍可能 miss。今天 fix 是「降低機率」、不是「保證」。

實機 CDP smoke 觀察過：IPC `spawn_goal` 派生 millis → 同一 thread spawn verb closure → verb 內 `Utc::now()` 派生 millis、兩次間隔 ~100µs、實測 4 次 spawn 都沒跨 ms 邊界。但 high-load / GC pause / Windows scheduler latency 下會跨。

`goal_run_id_precision_matches_verb_run_started_at_slug` unit test 只能驗 **format 同精度**（同 byte length）、不能驗 **值相等**。

## 真正的根本解（本 change 未做）

統一 RunId source of truth：**IPC 層派生一次、透過 `GoalOptions` / `ChatTurnOptions` / `QuizOptions` 下傳給 verb、verb 不再自己派生**。

Concrete：

1. 新增 `GoalOptions.run_id: String`（required field、不是 Option）
2. `spawn_goal_with_runner` 派生 `goal_run_id()` 一次、塞進 GoalOptions、丟給 runner
3. `run_goal(repo, options, ...)` 內**不再** call `Utc::now()` 派生 `run_started_at`、直接用 `options.run_id`
4. `EventsJsonlSink::new` 用 `options.run_id` 作 filename slug
5. `RunLog.started_at` 也派生自 `options.run_id`（slug → rfc3339、`-` → `:`）
6. CLI verb（`codebus goal`、`codebus chat`、`codebus query`、`codebus fix`、`codebus quiz`）必須在 entrypoint 派生 RunId 並注入 options，否則無法呼 verb
7. unit test `goal_run_id_precision_matches_verb_run_started_at_slug` 退役、改驗「verb 不再呼 `Utc::now()` 派生 run_started_at」

## 為何今天沒做這個根本解

1. **Scope 爆炸**：要改 5 個 verb function signature + 對應的 CLI entrypoint + 5 個 IPC spawn 函式。本 change 已 bundle 兩件 bug fix、user 明確 bundle 限定。
2. **CLI entrypoint 不在 IPC 層**：CLI run `codebus goal foo` 沒 IPC 層、要 CLI 自己派生 RunId、改 codebus-cli/src/commands/*.rs。
3. **Test signature 改動範圍大**：5 verb 的 既有 unit test 都要更新 stub runner 簽章。
4. **本 change 工時上限 ~1 hr** propose 階段約定、雖然 apply 階段 follow-on 已超、但繼續往源頭改會破天。

## 修法走過的兩個選項（為何選 B）

1. **選項 G (rejected by user)**：list_runs 對齊比對 strip `.fff`。IPC 層改一行。Symptom patch、留 invariant 隱藏分歧、未來其他 list_runs caller 也要 strip。User 拒絕 strip-at-lookup symptom patch。
2. **選項 B (chosen)**：verb 端也改 Millis。今天的 fix。Invariant 在「同 SecondsFormat 約定」上對齊、但 source of truth 仍二處派生、可能 ms 邊界 drift。
3. **選項 H (latent / this doc)**：統一 RunId source of truth、IPC 派生唯一、verb 接收。Permanent fix。

## 觸發條件 → 何時要做選項 H

- 任何 cross-component RunId-related bug 再次出現
- 高負載下 CDP smoke 抓到 active_runs.get(events_slug) miss case
- 加新 verb 時又要 copy `run_started_at` 派生樣板

或主動：當下次有 spec-driven change 已動 verb function signature（如新 verb 加入、verb options 重構），順手把 run_id 加進 options。

## 相關 artifact

- spec: `openspec/specs/app-workspace/spec.md` § Interrupted Run Detection NOTE「Precision Alignment Invariant」
- test: `codebus-app/src-tauri/src/ipc/goals.rs::tests::goal_run_id_precision_matches_verb_run_started_at_slug`
- archive: `openspec/changes/archive/2026-05-28-backend-cleanup-codex-websearch-and-runid-millis/`
- spike doc: `docs/2026-05-28-codex-hook-hard-gate-spike.md` E11（web_search 同 change archive、與此 latent issue 無關）
