# v3-app-workspace-goal — 動工前討論結論

> 2026-05-12 spectra-discuss session 紀錄。對應 `/spectra-propose v3-goal-library` 動工前 reread。
>
> 上游：`docs/v3-app-roadmap.md` §Sequence（已 update 反映 6 條序列）、`docs/2026-05-11-app-ux-flow-design.md` §4.2-4.4。

## TL;DR

原本 v3-app-roadmap §Sequence 從 #2 `v3-app-workspace-goal` 接 #1 foundation 直接做。實機 design discussion 後發現 #2 倚賴的 CLI 側基建有 2 個未做的洞，必須先以獨立 change 補完再做 GUI。

**5 條 sequence → 6 條**：最前面插 A `v3-goal-library` + B `v3-run-log-events` 兩條 CLI side prerequisite；原本 #2-#5 變 C-F。

## 觸發點 — 設計 vs 實作對不上的 2 個地方

### 1. `codebus_core::agent::invoke()` 沒 callback hook

`codebus-core/src/agent/claude_cli.rs:84` 是 sync function，內部 loop：

```
parse_claude_stream_line(line) → print_event(&event, render_opts) → stdout println!
```

stream render 跟 invoke 綁死。GUI 想 reuse `invoke()` 把 events emit 到 Tauri event bus 沒入口。

**唯二乾淨路徑**：
- (a) refactor `invoke()` 加 `on_event: impl FnMut(StreamEvent)` callback（鏡像 foundation 的 `init::run_init(on_event)` pattern）
- (b) GUI 端 spawn `codebus goal` CLI binary 再 parse stdout — **不可行**，CLI stdout 是 terminal-rendered 文字（emoji + 中文 label），不是原始 stream-json

選 (a)。同時順手把 `codebus-cli/src/commands/goal.rs::run()` 整套 orchestration（drift detection / re-sync / invoke / fix loop / auto-commit）抽進 `codebus_core::verb::goal::run_goal(repo, options, on_event, cancel)`，CLI 端變 thin wrapper。cancel 參數型別為 `Option<Arc<AtomicBool>>` — `None` 給 CLI 用（無 cancel button），`Some(flag)` 給 GUI 的 Cancel 按鈕用；不引入 tokio 依賴。

**這不違反 anti-pattern #1（no single-impl trait）** — `invoke()` 一直是 single impl，這次只是 caller-supplied closure 取代 hard-coded println，純函數簽名擴展。

### 2. Run-log 只存 summary，stream events 沒持久化

`openspec/specs/run-log/spec.md` schema 只記 `goal / mode / model / effort / started_at / finished_at / tokens / wiki_changed / lint counts` — 全部是 summary，**thought / tool calls 沒持久化**。

但 design doc §4.3.4 sub-state B 畫了「Stream history collapse ▼」可展開區。想滿足這個 UX 需要 stream events 持久化。

另外：

- Run-log 只在 verb 跑完 `Done` banner 前才寫一行 jsonl，**沒有 run_id 可定址**
- `log.sink: none` opt-out 全部關掉（CLI user 用情境合理）
- CLI auto-commit 是 spawn 完才下；mid-stream cancel 不會 auto-commit 半成品

要支援 GUI Goals overview list、completed goal detail timeline、cancel UX，**必須擴 run-log spec**。

## Q1 / Q2 / Q3 決議

### D1（Q1）：CLI 三個 spawn verb 全部抽進 codebus-core library

**拆獨立 prerequisite change** `v3-goal-library`：

- `codebus_core::agent::invoke()` 加 `on_event: impl FnMut(StreamEvent)` callback
- CLI 端 closure 包 `print_event` 保持 byte-equivalent stdout
- 抽 3 個 spawn verb 為 library function（鏡像 foundation 的 `init::run_init` pattern）：
  - `codebus_core::verb::goal::run_goal(repo, options, on_event, cancel)`
  - `codebus_core::verb::query::run_query(repo, options, on_event, cancel)`
  - `codebus_core::verb::fix::run_fix(repo, options, on_event, cancel)`
  - cancel 型別為 `Option<Arc<AtomicBool>>`（不引入 tokio 依賴）
- CLI `commands/{goal,query,fix}.rs` 變 thin wrapper byte-equivalent

#### D1.1：為什麼 3 個一起抽，不分批

實機 grep `codebus-cli/src/commands/*.rs` 後確認：

| Verb | CLI 端內容 | core lib 狀態 |
|---|---|---|
| **lint** | 40 行 thin wrapper（clap arg + `wiki::lint::lint_wiki()` + format text/json） | ✅ 已 library，**不抽** |
| **goal** | ~250 行完整 orchestration（drift / sync / invoke / fix loop / auto-commit / run-log） | ❌ 抽 |
| **query** | ~100 行 orchestration（vault precondition / config / env build / invoke / run-log） | ❌ 抽 |
| **fix** | ~150 行 orchestration（vault precondition / lint pre-check / invoke / fix loop / final lint / auto-commit / run-log）— 結構跟 goal 幾乎一模一樣 | ❌ 抽 |

3 個一起抽的理由：

1. **`invoke()` callback refactor 一動，3 個 verb callsite 都要改** — 既然 3 個都要動，順手抽進 library 比讓 3 份 `closure 包 print_event` 散在 CLI 端乾淨
2. **shape 同類** — goal / query / fix 三個都是「load config → spawn agent → run-log」骨架，抽 1 留 2 或抽 2 留 1 都是人為不一致；抽 0 或抽 3 一致
3. **不違反 anti-pattern #1**（spec 不寫 single-impl 抽象）— `run_goal` / `run_query` / `run_fix` 是 library function 不是 trait；fix 雖然 GUI v1 不用，caller 仍只有 CLI 一個，但跟 foundation 把 `init::run_init` 抽進 core 同 pattern（caller 也只有 CLI），這是「組織程式碼位置」不是「設計 abstract surface」

lint 不抽是因為 `commands/lint.rs` 已經是 thin wrapper，core lint logic 早在 `codebus_core::wiki::lint` library；GUI 要用直接 call `lint_wiki()`。

### D2（Q2）：Run-log 擴充存 stream events，CLI 也要做

**拆獨立 prerequisite change** `v3-run-log-events`：

- RunLog schema 加 `outcome` 欄位：`succeeded` / `failed` / `cancelled`
- 新增 per-run events.jsonl：`<vault>/.codebus/log/events-<started_at_slug>.jsonl`
  - slug 規則：`started_at` RFC 3339 字串中的 `:` 改成 `-`（避 Windows 檔名限制），例 `events-2026-05-12T03-25-11Z.jsonl`
  - stream events 一筆筆 live append，crash-resilient
- 只有 `outcome=succeeded` 才下 auto-commit（保留 CLI 現行行為）
- Cancel path 寫 `outcome=cancelled` 且**不 auto-commit**

#### D2.1：`log.sink: none` opt-out 對 GUI 不適用

- CLI： `log.sink: none` 一視同仁關掉 events.jsonl + run-log（一致行為）
- **GUI-spawned runs 一律寫 events.jsonl 與 run-log，忽略 `log.sink: none`**
  - 理由：events.jsonl 是 GUI Goals overview / detail view 的**唯一資料來源**，砍掉等於砍 UI 自己腳
  - 理由：CLI user 想 opt-out 動機合理（自己有 log pipeline）；GUI user 想 opt-out 動機極低
- B change spec 明寫此例外
- Foundation 既有 foot-gun（Settings UI Log sink 欄位永遠把 sink 寫成 `jsonl`，hand-edit `none` 一動 Change folder / Reset 就被覆蓋）**本次不處理** — events.jsonl 反正強制寫，不擴大這個 foot-gun

### D3（Q3）：Cancel / interruption UX 採最小 surface

**保留**：

- `[Retry with same goal]` 一顆按鈕 — pre-fill goal text 進 New Goal modal，user 點 Run goal 走同 spawn_goal 路徑

**不加**：

- ~~Reset 按鈕（destructive git op）~~：UI 上做就得 typed confirmation 多 UX 路徑；partial 改動沒實際傷害；CLI 安全網（`git -C .codebus/.git reset --hard`）已寫進 README §Security；YAGNI
- ~~Continue 按鈕~~：agent context 沒持久化、claude CLI 沒 resume API，continue 技術上等於 retry，不需要兩顆

**Cancelled detail view 長相**（修正 design doc §4.3.4 sub-state B）：

```
┌─────────────────────────────────────────────────┐
│  ← back     "搞懂 auth 模組怎麼運作"   ⏹ Cancelled│
│             Cancelled after 12s · 3.1k tokens   │
├─────────────────────────────────────────────────┤
│                                                 │
│  ⚠ Wiki has uncommitted changes — not auto-     │
│    committed. Review in terminal if needed.     │
│                                                 │
│  Partial timeline:                              │
│  ▶ Reading codebase                             │
│    📄 src/auth/middleware.ts                    │
│  ▶ Writing wiki                                 │
│    ✏ modules/auth-middleware.md (new, partial)  │
│                                                 │
│  ─── stream history (collapse ▼) ────           │
│                                                 │
│                  [Retry with same goal]         │
└─────────────────────────────────────────────────┘
```

**"Interrupted" case（app killed / OS crash mid-stream）**：

- 用同一畫面，header 換成 `⚠ Interrupted` + 文案「App was closed before this goal finished」
- 偵測機制：下次 app 啟動時 scan `<vault>/.codebus/log/`，「events-*.jsonl 有但對應 run-log finish entry 沒有」即為孤兒，標 interrupted
- 同樣顯示 partial timeline + `[Retry]`

## Change 序列（修正版）

| # | Change name | 內容 | 依賴 |
|---|---|---|---|
| A | `v3-goal-library` | 3 個 spawn verb（goal / query / fix）orchestration 搬進 `codebus_core::verb::*`；`invoke()` 加 `on_event` callback + `Option<Arc<AtomicBool>>` cancel signal；`run_goal` / `run_query` / `run_fix` 接同樣的 callback + cancel；CLI 三個 commands 變 thin wrapper byte-equivalent。lint 已 library 不動。 | — |
| B | `v3-run-log-events` | RunLog schema 加 outcome；per-run events.jsonl 持久化；cancel 不 auto-commit；GUI-spawned runs 強制寫（忽略 `sink: none`） | A |
| C | `v3-app-workspace-goal` | Vault Workspace sidebar + Goals overview + Goal flow（modal / inline mini-stream / running detail / done detail / cancelled detail / interrupted）+ Wiki preview（Milkdown + wikilinks）；Quiz tab 留 placeholder | A + B + foundation |
| D | `v3-app-query-cmdk` | 原 v3-app-roadmap #3，依賴 C 的 stream rendering pipeline + wiki page model | C |
| E | `v3-app-quiz` | 原 v3-app-roadmap #4 | D |
| F | `v3-app-polish-ship` | 原 v3-app-roadmap #5 | A-E 全 ship |

A / B 都改 CLI 但不破 CLI 對外行為：
- A 是 byte-equivalent refactor（CLI 端 closure 包原本 `print_event`）
- B 是擴 schema + 新檔（舊 RunLog 仍可讀；`outcome` 缺欄默認 `succeeded`；events.jsonl 是新增物，不存在不影響舊 reader）

## 副產品（不在本批 change 範圍，後續另解）

- **README §Final destination 段落跟現實 diverge** — 提 `<Checkpoint>` / 投影片模式，但這些已在 v3-app-roadmap §Out of scope。需後續 docs PR 更新（user-facing 文件，按 [[feedback_user_facing_docs_discuss_first]] 動手前先討論）。
- **Foundation Settings UI Log sink foot-gun** — Change folder / Reset 永遠寫回 `sink: "jsonl"`，把 hand-edit 的 `none` 吃掉。本批 change 不處理（events.jsonl 強制寫策略避開此 foot-gun）；若日後想讓 GUI user 也能 opt-out CLI log，另開 polish change。

## 後記（apply 階段補充）

discuss 時的 3 個 open question 都已 resolved：

1. 6 條 change 序列、依賴關係 — 確認沿用。
2. A 的抽離範圍 — 最終擴大到「goal + query + fix」三個 spawn verb 一起抽（discuss 時只列了 goal + query，apply 動工讀完 CLI source 後發現 fix orchestration 跟 goal 結構同型，分批抽會二次 churn `invoke()` callback 簽名，所以一次做完）。lint 維持不抽（已是 thin wrapper）。
3. Change name `v3-goal-library` / `v3-run-log-events` — 採用。

實作階段發現 / 確定的設計細節（未進 discuss、進 spec / design）：

- `VerbEvent::Banner` 採用 owning `VerbBanner` enum + `as_banner()` 借出 `Banner<'_>`，不是 `Banner` re-export — 因為 `Banner<'a>` 帶 lifetime，在 `impl FnMut(VerbEvent)` 跨執行緒 closure（GUI Tauri event emit）下無法滿足 `'static + Send`
- `VerbError` 比 discuss 預估多一個 `KeyringMissing { source: KeyringError }` variant（exit 3）+ `ConfigParse` 加 `which: &'static str` 欄位 — 為了保留 CLI byte-equivalent stderr 訊息
- `QueryReport` / `GoalReport` 加 `agent_exit_code: Option<i32>` — CLI 需要它做 exit code propagation
- `GoalReport` 加 `fix_post_lint_issues_remain: bool` — CLI 需要它在 byte-equivalent 時機 emit `✗ fix: ...` 訊息
- `FixReport` 加 `status: FixStatus { Skipped { reason }, InitialClean, PostLintClean, PostLintIssuesRemain }` + `SkipReason { NoFixFlag, DisabledByConfig }` enum — CLI 需要它分支 stderr 訊息與 exit code

完整實作對照見 `openspec/changes/v3-goal-library/specs/verb-library/spec.md` 與 `specs/cli/spec.md`。
