# TODO · Cancelling 卡住

> **archived: 2026-05-28** — landed via change `cancelling-stuck-fix`. Final root cause: blocking `BufReader::lines()` cancel-check coupled to stdout activity（hypothesis b）COMBINED WITH Windows process-tree leak（孫 process `node.exe` 繼承 stdout pipe）. Fix: background cancel watcher thread + cross-platform `KillHandle`（Windows Job Object `KILL_ON_JOB_CLOSE`、Unix `process_group(0)` + `killpg`）. Verified on both codex AND claude providers via CDP smoke（`codebus-app/scripts/.cancelling-stuck-fix-smoke/`）, cancel→spawn_end ≤ 1s.



## 現象

User 點 Cancel goal → UI 進「Cancelling...」state、但不會切到 terminal（cancelled / interrupted）。卡住、需要手動 reload 或重啟。

## 2026-05-28 update：RunId millis fix 後仍 reproduce

`backend-cleanup-codex-websearch-and-runid-millis`（commit `cc580dc`）archive 後、user 實機驗證 cancel 仍卡。原 hypothesis e（run_id collision 影響 cancel 路由）**排除**、不是 root。

Agent archive 階段 CDP smoke 留下關鍵 evidence：

> events file 只 3 行沒 spawn_end、runner 沒返回、emit_terminal 沒發
> (memory `project_backend_cleanup_codex_websearch_runid_millis_archive_lessons` lesson 3)

→ 問題在 backend **runner 自己卡死、沒返回**、不是 cancel signal 路由錯、也不是 emit 失敗。

## 修正後可能成因排序

| # | 假設 | 機率 | 對應 evidence |
|---|---|---|---|
| **b** | **Child process（claude / codex）忽略 cancel signal、繼續跑、`run_goal` 等不到結束** | **HIGH** | events file 3 行停下 = stream 中斷但 child 沒退出 |
| d | Thread::Builder::spawn 後 deadlock / stdin/stdout poll 卡死 | 中 | runner 沒返回、可能 IO poll 永久阻塞 |
| a | Backend cancel flag set 後、polling loop 沒及時 observe | 中 | 若是這個、應 polling interval 內 observe 到、不該「永久卡」 |
| c | Frontend 沒收到 goal-terminal | 低-中 | symptom 一致但不是 root（runner 沒返回所以 emit_terminal 沒 fire）|
| ~~e~~ | ~~run_id collision~~ | **排除** | millis fix 已 land、cancel 仍卡 |

## 相關 anchor

- Frontend `activeRun.cancelling` flag：`codebus-app/src/store/goals.ts:151-162`（`cancelGoal` action 設 flag）
- Backend cancel impl：`codebus-app/src-tauri/src/ipc/goals.rs:284-298`（`cancel_goal` / `cancel_goal_impl`）
- Cancel flag observe point：`codebus-core` `run_goal` 內 polling loop（grep `cancel.load(Ordering::Relaxed)` 或同 pattern）
- Child process kill：可能 `claude_cli.rs` / `codex_backend.rs` 內、grep `Child` / `kill` / `wait` / `SIGTERM`

## Pre-apply 起手

1. Read `codebus-core` 的 `run_goal` polling 機制（cancel flag 怎麼 check）、child process 怎麼 spawn / wait / kill
2. Read claude_cli / codex_backend 看 child process 是 stdin pipe + JSON-line / stdout pipe blocking read？ stdin closed 是否 trigger child 退出？
3. CDP smoke reproduce：開 goal → 跑幾秒 click Cancel → backend log 看 cancel flag 收到沒、child kill 發出沒、child 多久才 exit
4. 對照 `project_backend_cleanup_codex_websearch_runid_millis_archive_lessons` 的 archive 階段 CDP evidence
5. 推測修法（待 Task 1.x verify）：
   - 若 b：cancel flag set 後、明確 `Child::kill()` 而不只 set flag 等 polling observe
   - 若 d：把 stdin/stdout poll 改 timeout-based、cancel flag set 時 break poll loop
   - 若 a：縮短 polling interval（但根本問題 = 為什麼 polling 沒在 child 退出後馬上 return）

## Priority

**HIGH**（confirmed workflow blocker、user 實機驗證 millis fix 後仍 reproduce、不能再 defer）。

順序考量：插隊到 Bug 2 之前。Cancel 是 critical safety affordance、user 沒辦法 escape long-running goal 是 unacceptable。

## 不在 scope

- 全 redesign cancel mechanism
- 加 force-kill timeout 之類粗暴解（先找 root cause）

## 觸發時機

下一個 spectra change。優先於原 backlog Bug 2 / 1。
