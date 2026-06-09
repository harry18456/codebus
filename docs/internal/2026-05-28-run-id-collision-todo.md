# TODO · RunId same-second collision

> **Status：archived 2026-05-28** — fix landed in
> `openspec/changes/archive/2026-05-28-backend-cleanup-codex-websearch-and-runid-millis/`.
> IPC + verb 7 處 timestamp 派生統一升 `SecondsFormat::Millis`、`goal_run_id` /
> `chat_run_id` helper 抽出、`goal_run_id_precision_matches_verb_run_started_at_slug` +
> `goal_run_id_same_second_yields_distinct_ids` + `chat_run_id_same_second_yields_distinct_ids`
> three unit tests 守住 invariant。
>
> **Latent issue 未解**：IPC + verb 仍兩處獨立呼 `Utc::now()`、極端時鐘抖動下
> 仍可能差 1ms；list_runs orphan-detection 偶爾誤標 interrupted。長期解
> （RunId source of truth 統一、由 IPC 派生並下傳 verb）見
> `docs/2026-05-28-runid-source-of-truth-todo.md`。


## 現象

`active_runs` HashMap 鍵是 RunId、RunId 來自 `started_at` 秒精度 slug（`codebus-app/src-tauri/src/ipc/goals.rs:228`）：

```rust
let started_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
let run_id = started_at.replace(':', "-");
```

→ 同一秒內 spawn 兩個 goal（Bug 3 修完後 cross-vault 允許）會產生**相同 RunId**、`HashMap.insert` 第二次寫入覆蓋第一次的 cancel flag、cancel signal 路由錯。

## Bug 3 (`vault-switch-goal-regression`) archive 後新發現

Memory `project_vault_switch_goal_regression_lessons` follow-up #1（2026-05-28）。

## 後果

| 操作 | 預期 | 實際 |
|---|---|---|
| Cancel goal A | flip A 的 cancel flag | 因 active_runs[run_id] 已被 B 覆寫、flip 到 B |
| Thread A finishes、remove(run_id) | 移除 A 自己 entry | 把 B 的 entry 拔掉、B 失去 cancel 路徑 |

Cancel 是 idempotent（`cancel_goal_impl` line 293-298 missing key 無 op）、所以不 panic、但 cancel signal 路由錯誤 = correctness bug。

## 觸發機率

| 場景 | 機率 |
|---|---|
| Manual UI click 連續 spawn 兩 goal（切 vault + click + 輸入文字 + submit ×2）| 極低（chain 2-5+ 秒） |
| Programmatic / CLI 連續 spawn | 中 |
| Tests（fast-clicking、auto retry race）| 中-高 |
| Automation script | 中 |

Manual user 幾乎遇不到、但 test / CI / future automation 場景下會中。

## 修法候選

| 方向 | 改法 | Pro / Con |
|---|---|---|
| **A. 提高 timestamp 精度** | `chrono::SecondsFormat::Millis` 或 `Micros` | Pro：最小改動；Con：理論上同毫秒仍可能、但機率 1000x 降 |
| **B. RunId 加 nonce 後綴** | `format!("{}-{:08x}", started_at, rand_or_uuid)` | Pro：100% 不碰撞；Con：RunId 變長、cosmetic 醜 |
| **C. Atomic counter** | Process-wide AtomicU64、append 進 RunId | Pro：deterministic + 無外部依賴；Con：跨 process restart 不 unique（但 active_runs 也跨 restart 不 persist、無實際問題） |

→ 推 **A**（毫秒精度）+ 如果不放心再加 **C**（counter）。**B** uuid 重 + RunId 變得不像 timestamp、debug 體驗變差。

## 影響範圍 grep

預估動到：

- `codebus-app/src-tauri/src/ipc/goals.rs:228`（spawn_goal_with_runner RunId 生成）
- 其他 verb spawn 的 RunId 生成處（chat / quiz、grep 確認同 pattern）
- spec scenario 含 RunId 範例字串（若有）
- test fixture 用 hard-coded RunId 的地方
- frontend 解析 RunId 為時間顯示的地方（看 RunId format 是否被 parsed 來顯示 started_at）

## 預估工時

10-20 min（grep 完整 + 數值改 + test fixture + spec scenario 同步）。

## Priority

**低-中**。

Daily user 遇不到、但是 real correctness bug、test / automation 場景會中。

## 建議處理時機

跟 Bug 4 `codex-web-search-disabled` 合 1 個 cleanup change（兩者都 backend 小改、scope 相近、可同 apply session 跑完）。

OR 獨立放 backlog、後面有空檔順手做。

## 不在 scope

- 改 RunId 格式從 timestamp 改成 pure UUID（debug 體驗回退）
- 跨 process restart RunId uniqueness（active_runs 不 persist、無實際需求）
