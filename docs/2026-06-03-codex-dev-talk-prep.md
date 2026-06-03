# Codex 開發者來訪 — Talk-Prep 簡報（2026-06-03）

> 對象：親手 build codex 的 OpenAI 開發者。語氣：認真的整合者，給精準、有實證的回饋與尖銳問題，同時誠實給予肯定。
> 來源：codebus 自身程式碼 + sibling research repo `agent-cli-research/` 的 PoC + 官方 docs/changelog 查證 + **2026-06-03 在本機 codex 0.136.0 上重跑全部 sandbox PoC**。

---

## 0. 環境前提（先講清楚）

- **codex-cli 0.136.0**（本機 `codex.cmd` = `C:\Users\harry\AppData\Roaming\npm\codex.cmd`，`.cmd → node → codex.exe` shim 屬實）
- **claude-code 2.1.161**
- **Windows 11 Home、非管理員（non-admin）、`windows.sandbox=unelevated`**
- macOS / Linux **未實測** —— 別讓對方以為下面是跨平台結論（Seatbelt / Landlock 行為可能完全不同）

---

## 1. TL;DR + 最重要的 caveat

**codebus 在 Windows 上把 codex 當成可隔離、可程式化驅動的 agent backend，整體封裝乾淨；codex 在 Windows 的 sandbox「寫入 / 命令 / HTTPS egress」面做得誠實且大致到位，但「讀取」面在 unelevated（非管理員）模式下基本不設防 —— 這是 OpenAI 自己有文件揭露的設計取捨，不是 bug。**

> ✅ **caveat 已大幅降級**：我們所有 sandbox PoC 原本測於 0.135（read PoC 原始 0.134），對方在 0.136+。**2026-06-03 已在本機 0.136.0 重跑全部五支 PoC，0.135 的每一條判決逐格重現**（見 §2 對照表）。所以 read/write/egress/subagent 現在都可以當「0.136 today-verified」講，**唯一還沒在新版重量測的只剩 skill 的 `$` vs implicit token 比**（那是 token 計量、不是 sandbox 邊界）。

---

## 2. 2026-06-03 在 0.136.0 上的重驗結果（逐項對照 0.135）

全部用 **mock Responses provider 驅動真 codex.exe 跑在真 OS sandbox**（無 API spend）。

| 探針 | 模式 | 0.135 判決 | **0.136 重跑** | 一致? |
|---|---|---|---|---|
| 讀 workspace 外合成憑證 | read-only | LEAKED, exit 0 | **LEAKED, exit 0, blocked_by_policy=false** | ✅ |
| 讀 workspace 外合成憑證 | workspace-write | LEAKED | **LEAKED** | ✅ |
| 讀 `%USERPROFILE%` 下合成憑證 | workspace-write | LEAKED | **LEAKED** | ✅ |
| 讀 workspace 內檔（控制組） | read-only | reads（無 marker） | **reads（無 marker）** | ✅ |
| 寫 workspace 外（normal ACL） | workspace-write | DENIED（Access is denied） | **DENIED（未寫入）** | ✅ |
| 寫 `%USERPROFILE%`（normal ACL） | workspace-write | DENIED | **DENIED（未寫入）** | ✅ |
| 寫 Everyone-writable（`*S-1-1-0`）dir | workspace-write | LEAK（寫入） | **LEAK（wrote=true, exit 0）** | ✅ |
| 寫（任意目標） | read-only | 全 blocked pre-exec | **全 blocked** | ✅ |
| egress loopback 127.0.0.1 | workspace-write | allowed（真 hit） | **allowed（egress_hits 收到）** | ✅ |
| egress 外部 HTTP/80 | workspace-write | allowed | **allowed_external** | ✅ |
| egress 外部 HTTPS/443 | workspace-write | blocked（conn reset） | **blocked_no_connection** | ✅ |
| egress（任意） | read-only | 全 blocked pre-exec | **全 blocked_pre_exec** | ✅ |
| subagent：main read-only → worker 寫 | — | bounded（worker 寫被拒） | **w_in/w_out 皆未 land** | ✅ |
| subagent：main workspace-write → worker 寫 | — | bounded（界在 workspace） | **w_in landed / w_out denied** | ✅ |
| subagent：能否 spawn_agent | — | 能（multi_agent on） | **spawn_agent_issued=true** | ✅ |

**重點**：0.136 release notes 有「deny read rules stay enforced for safe commands」與「cancel Windows sandbox on network denial / cleanup after denied network」這些網路/讀取相關改動，但 **都沒關掉**我們量到的 unelevated read-anywhere 與 HTTP/80+loopback egress。最新版上這兩個面向仍開。

底層機制旁證：0.136 ACL dump 顯示沙箱目錄掛著 `CodexSandboxUsers:(I)(OI)(CI)(RX)` 這個 codex 專屬群組 —— 佐證 write 是真 OS token-ACL 強制。

---

## 3. 🔴 先前對外講過、本輪查證後修正的地方（誠實清單）

明天**不要**照舊版講以下幾條，已修正：

| 原本說法 | 修正後 | 依據 |
|---|---|---|
| 「naively **draining** the pipe deadlocks」 | **誤歸因**。production code 正常 pipe+drain；真正卡死的是「**只 kill 單一 PID** 孤兒化占 stdout pipe 的孫程序，reader 收不到 EOF」。且占 pipe 的 leaf 是 native **`codex.exe`**，不是 node.exe（codebus 自家 `process_kill.rs:6-15` 註解寫錯成 node.exe，待修） | 程式碼 + 0.136 live；本輪 read PoC 用 pipe 時也觀察到殘留 codex.exe |
| 「`/skill-name` 走 description-match、多 ~25% token」 | ① `24.8%` 是 **n=1、跑在 0.133.0**，落後 3 個 minor，別當現值；② 對 **skills** 而言 `/skill-name` 其實是 **`Unrecognized command`**（issue #11817），真正貴的是 **implicit（無前綴）description-matching**，不是 `/`；`/prompts:name` 屬已 deprecated 的 custom-prompts | 官方 docs + #11817 |
| 「0.134→0.135 sandbox 行為漂移」 | **過度陳述**。那次唯一明確的 read 重驗是 **不變**；write「flip」被釐清為 Everyone-writable 的 **ACL artifact**（穩定，非漂移）。且 codebus **沒 pin codex 版本**，只有被動「升版重跑 spike」紀律 | research repo git log |
| 「`--permissions-profile` 是新隔離旗標」 | 它只存在於 `codex sandbox`，**不是** `codex exec` 的旗標 → 不觸及也不取代 codebus recipe | 0.136 live `unexpected argument` |

---

## 4. Top 5 要當面講／問的（已排序）

### #1 — Windows 非管理員 read leak：`read-only` 不是讀取邊界（0.136 today-verified）
- **Claim**：Win11 非管理員 / `windows.sandbox=unelevated` 下，`-s read-only` 與 `-s workspace-write` **都能讀 workspace 外**的檔案（含 `%USERPROFILE%` 下合成憑證，同 `~/.ssh`/`~/.aws` 的 ACL/位置類別），`exit 0`、無警告、`blocked_by_policy=false`。
- **證據**：`read-poc-0136.json` 三 case `marker_seen=true`；OpenAI PR #18202（0.131.0）自陳 unelevated WRITE_RESTRICTED token「cannot safely enforce deny-read」故 fail-closed。**Issue #23459（unelevated workspace-write 讀整顆磁碟含 user profile）截至上月仍 OPEN**。
- **要問**：「#23459 仍開著 —— 在 unelevated restricted-token backend 上限制讀取範圍在 roadmap 上嗎？還是 read-anywhere 是非管理員的永久設計、唯一真正的讀取邊界就是 alpha 的 elevated backend？0.136 的『deny read rules stay enforced』有打算延伸到這塊嗎？」

### #2 — 外部 orchestrator 必須自己 kill 整棵 process tree
- **Claim**：Windows 上 codex 是 `codex.cmd → node.exe → codex.exe`，stdio inherit 到 leaf；只 kill 單一 PID 會留下 **native codex.exe** 孫程序占住 stdout pipe（reader 收不到 EOF → invoke 卡死）。cancel/timeout 必須殺整棵樹（Win32 Job Object `KILL_ON_JOB_CLOSE` / Unix `killpg`）。多行 prompt 走 stdin（`codex exec -`）。
- **0.136 利多（可肯定）**：release notes 主動改善 codex 自己的清理（sandboxed-command cleanup after interruptions/denied-network #22729,#19880,#23943；PR #19211 終止 stray `codex-command-runner.exe`）—— 平台在收斂這個失敗類別。
- **要問**：「有沒有給外部 orchestrator 的正式契約來終止 codex process tree？例如乾淨取得 native `codex.exe` 的 leaf PID，或保證它在 `codex.cmd` stdin 關閉／SIGTERM forward 時自行結束 —— 還是我們得永遠用 Job Object 自己包？」

### #3 — egress 半開：HTTPS/443 擋、HTTP/80 + loopback 漏（0.136 today-verified）
- **Claim**：unelevated workspace-write **不是 network-off**：外部 HTTP/80 與 loopback 都成功（live listener 真回 hit），只有 HTTPS/443 連線無法完成；`-s read-only` 對所有網路 pre-exec by policy 拒絕。
- **證據**：`egress-0136.json` loopback `allowed` + egress_hits 收到、HTTP/80 `allowed_external`、HTTPS/443 `blocked_no_connection`。**0.136 的「cancel on network denial」沒關掉這洞。**
- **要問**：「0.136 unelevated workspace-write 上，HTTP/80 + loopback egress 仍開（我們今天剛驗）—— HTTPS 失敗但 HTTP 成功的不對稱是刻意（TLS-specific）還是 env-based offline controls 被 raw socket 繞過的副作用？這在 unelevated 是永久的 best-effort 限制嗎？」

### #4 — `$skill` native 顯式觸發便宜；非顯式走 description-match 較貴；SKILL body 選中後才讀
- **Claim**：codex 上 `$skill-name` 是 native 顯式觸發、跳過 description 評估；非顯式 fallback 到 implicit description-matching（較貴）。SKILL.md body 是選中後 on-demand 讀（codex 上是可見的 `command_execution` tool call），**非** system 上前注入。
- **修正（見 §3）**：別引 24.8% 舊數字；貴的是 implicit 無前綴、不是 `/`。
- **要問**：「對 skills 而言 `/skill-name` 現在是否一律 `Unrecognized command`（#11817），唯二入口是顯式 `$skill` 與 implicit description-match？有沒有受支援的方式讓 user 看到 implicit 觸發每 turn 實際花掉多少 description-evaluation token？」

### #5 — `codex exec --json` schema 與 sandbox flag 跨 minor 的穩定性承諾？
- **Claim**：exec JSONL schema 與 sandbox/isolation flag 無公開 back-compat 保證；codebus 採「每次升版重跑隔離 spike」紀律（**非** pin 版本）。JSONL 有過 undocumented break（#4776：`item_type`→`type`、`assistant_message`→`agent_message`）。
- **0.136 現況（today-verified）**：codebus parser 依賴的欄位 `type` / `item.completed` / `agent_message` / `turn.completed.usage{input_tokens,cached_input_tokens,output_tokens,reasoning_output_tokens}` 全部健在；整條隔離 recipe parse + 跑 → exit 0、無 trust prompt。
- **要問**：「`codex exec --json` event-stream schema 與 sandbox/isolation flag（`windows.sandbox` enum、deny-read 強制）有任何 stability / semver 承諾嗎？還是整合者應視為 experimental、每個 minor bump 都重新驗證？`--experimental-json` 何時會 stabilize？」

---

## 5. 完整 claim 清單（依主題分組）

圖示：✅ 0.136 today-verified · ⚠️ 已修正 over-claim（仍可用、需聲明邊界） · ❓ 待重量測

### A. Windows 整合（process / pipe / shim）

**⚠️→✅ A1 — shim pipe-orphan + process-tree kill + multiline-via-stdin**
- 證據：`process_kill.rs:6-15`、`terminate_tree`（Win `TerminateJobObject` :198-211 / Unix `killpg` :182-196）、`codex_backend.rs:279-300`（newline→`-`+stdin）；0.136 live `codex exec --help`：「If not provided as an argument (or if `-` is used), instructions are read from stdin」。
- Freshness：shim 形狀 + grandchild-holds-pipe 是 npm 打包 + OS pipe 繼承事實，版本穩定；`-` reads stdin 已 0.136 live 驗。
- 修正：見 §3（draining 誤歸因；leaf 是 codex.exe 非 node.exe）。
- 要問：見 Top 5 #2。

### B. Sandbox（read / write / egress / isolation recipe）

**✅ B1 — read leak：`read-only` 不是讀取邊界**（見 Top 5 #1，0.136 today-verified）

**✅ B2 — write ACL-gated：normal-ACL 擋、Everyone-writable 漏**
- 0.136：`acl-0136.json` normal-ACL → DENIED（access_denied_seen=true）；`Everyone:(OI)(CI)(W)` → LEAK（wrote=true）。只有 ACL 不同。
- 要問：「unelevated workspace-write 下，唯一擋住寫 `C:\Windows\Temp` 的只是 Everyone 在 restricted SID list 裡 —— issue #14006 顯示你們可對 workspace 內 world-writable 路徑加 deny-ACE；有沒有計畫延伸到 workspace **外**？還是關掉 Everyone leak 根本得靠 elevated/admin backend？」

**✅ B3 — egress 半開（HTTPS 擋 / HTTP+loopback 漏）**（見 Top 5 #3，0.136 today-verified）

**⚠️→✅ B4 — per-spawn 隔離 recipe + `--disable` set + `--permissions-profile` 位置修正**
- recipe：`codex exec --json --ignore-user-config --disable {apps,plugins,hooks,browser_use,browser_use_external,computer_use,in_app_browser} --ignore-rules --skip-git-repo-check -c project_root_markers=[...] -c windows.sandbox=unelevated -c web_search=disabled --ephemeral -s <mode>`。
- 0.136 live：整條 parse clean、exit 0、stderr 空、**無 trust prompt**；7 個 `--disable` 目標 `features list` 全 `stable true`；`codex exec --permissions-profile foo` → `unexpected argument`（該旗標只在 `codex sandbox`）。`codex_backend.rs:146-192` 逐字相符。
- 注意：`experimental_windows_sandbox`/`elevated_windows_sandbox`/`plugin_hooks` 現標 `removed`、`use_legacy_landlock` `deprecated`、`web_search` 預設改 `cached`（但 `disabled` 仍移除工具，recipe 不受影響）。
- 要問：見 Top 5 #5。

### C. Token

**✅ C1 — `turn.completed.usage` 是 cumulative-replace，不是 per-turn delta**
- 證據：`codex_backend.rs:311-315`（`TokenUsageSemantics::Cumulative`）、`sink.rs:296`（`*acc = addend.clone()` last-wins）、unit test `apply_cumulative_takes_latest_not_sum`（`sink.rs:308-326`）；codex issue #17539 原文「turn.completed... reports cumulative session token totals... only `.total` is emitted, `.last` is discarded」。
- Freshness：低風險；0.135/0.136 notes 零 token-usage 變更；#17539 仍 OPEN；`reasoning_output_tokens`(PR #19308) 純加法、codebus 已 map（`codex_parser.rs:79`）。
- 殘餘風險（結構性）：parser 硬綁字串，未來改名會讓 `g()` 回 None、`unwrap_or(0)` 在 cumulative last-wins 下用 0 靜默覆蓋好的舊總量。
- 要問：「#17539 的 per-call `last` 若出，是在 cumulative total 旁 **新增** 欄位，還是改變現有 `usage` 語意 —— 也就是『對 cumulative total 做 last-wins』的 consumer 會不會靜默壞掉？」

### D. Skill + Subagent

**⚠️ D1 — `$skill` native 便宜 / 非顯式較貴 / body 選中後讀**（見 Top 5 #4；token 比待重量測）

**✅ D2 — `multi_agent`/`spawn_agent`：subagent 繼承 session `-s`（write/cmd 受界、read 仍 soft）**
- 0.136 live：`features list` = `multi_agent stable true`（codebus 7-feature `--disable` 刻意不含它，`codex_backend.rs:122-124`）；`subagent-escape-0136.json`：read-only→worker 寫全未 land、workspace-write→界在 workspace（w_in land / w_out denied）、兩模式 `spawn_agent_issued=true`、無逃逸。
- 已知風險：spawn_agent 無 sandbox/cwd 參數（#20077 參數集 `{agent_type,task_name,message,model,reasoning_effort,fork_turns}`）；inheritance 有 regression class（#15305 review-mode subagent 退回 config-default）→「inherits `-s`」是文件意圖、非保證。read 面與 main 一樣 soft。
- 要問：「Windows unelevated 下，`spawn_agent` 出的 subagent 是否 **保證** 繼承 parent turn 的 **live runtime** `-s` override（含 read 面），還是可能像 #15305 退回 config-default？」

### E. 版本穩定

**⚠️ E1 — schema/flag 跨 minor 無 back-compat 保證 → 升版重跑 spike**（見 Top 5 #5，含 §3 兩點修正）

---

## 6. 可現場 demo 的項目（全部今天已實跑過、可重現）

> **Windows 鐵律（CLAUDE.md invariant）**：所有 ad-hoc codex probe 輸出 **一律重導到檔案、絕不 pipe 進 demo shell** —— `.cmd→node→codex.exe` 孫程序占 stdout pipe，naive drain 會卡死。PoC script 多數已內建檔案重導 + `taskkill /F /T` 收尾（read PoC 例外、用 pipe）。zh-TW Windows 跑 PoC 要 `PYTHONUTF8=1`（cp950 解碼會崩）。

| # | Demo | 怎麼跑 | 注意 |
|---|------|--------|------|
| 1 | **隔離 recipe + `--permissions-profile` 修正** | (a) 整條 recipe `-s read-only "say hi"` → exit 0、JSONL clean、無 trust prompt；(b) `codex exec --permissions-profile foo "hi"` → `unexpected argument` vs `codex sandbox --help` 列出該旗標 | 一張投影片同框「拒絕錯誤 + recipe exit=0」 |
| 2 | **read leak / 0.136**（殺手鐧） | `python scripts/codex_sandbox_read_poc.py --json`（codebus repo）→ 4 case `marker_seen` | 讀 **合成** marker、絕不讀真 `~/.ssh`；codex 在真 OS sandbox 下執行 read，mock 只是 deterministic Responses oracle |
| 3 | **write ACL A/B** | `python codex_write_acl_poc.py`（research repo poc/codex-sandbox）→ normal-ACL DENIED vs Everyone-writable LEAK | **看 inner 寫入結果 / 檔案存在**，不看 process exit（codex exec 即使 inner 被拒仍回 exit 0） |
| 4 | **egress 半開**（loopback 最乾淨） | `python codex_write_egress_poc.py`：起本地 listener，workspace-write loopback 真回 hit | 同 unelevated 非管理員 profile 才可比 |
| 5 | **token cumulative** | `codex exec --json "<prompt>"` 看 `turn.completed.usage` 的 `cached_input_tokens` 占比；指 unit test `apply_cumulative_takes_latest_not_sum` | live token 抵達時機是 n=1 empirical，demo wire-format/code 事實、別把 timing 當保證 |
| 6 | **multiline-stdin**（30s 安全） | `printf 'line one\nline two\n' \| codex exec -` 顯示 `-` 從 stdin 讀多行 | help 文件已載明 `-` 行為 |
| 7 | **tree-kill / orphan**（視覺） | 起長 `codex exec` 占 stdout、只 kill `codex.cmd` PID → `tasklist` 顯示 `codex.exe` 仍活著占 pipe vs codebus `terminate_tree` 回收；regression test `terminate_tree_kills_grandchild`(`process_kill.rs:315-377`) 無 spend 重現 | — |
| 8 | **subagent bounding** | `python scripts/codex_subagent_mock_escape.py` + `codex_subagent_write_oracle_control.py`（無 spend、write-oracle anchored） | **明說「mock-driven、write-oracle-anchored、0.135+0.136 皆未見逃逸」**，非 live real-agent |
| 9 | **skill `$` vs implicit 重量** | 同 prompt 跑兩次（輸出重導檔案）：`$codebus-chat <q>` 顯式 vs bare prompt 強迫 implicit；比 `input_tokens` 與 tool-call 數；live show `/codebus-chat` 回 `Unrecognized command` | 數字會偏離 24.8%（n=1、新版），呈現為 live data point、不引舊數字 |

---

## 7. 可以正面肯定 codex 的點（真心話）

- **Windows process-tree 衛生正在收斂（A1）**：0.136 notes 主動改善 codex 自己的清理（#22729,#19880,#23943；PR #19211）—— 正是 codebus 當初要在外部防的失敗類別，平台往對的方向走。
- **read scope 是誠實設計而非馬虎（B1）**：read-only 文件即載明允許讀任意處（它界的是 write/command/network，不是 read）；PR #18202 讓 codex **fail-closed** —— unelevated token 無法誠實 honor deny-read overlay 時直接早早拒絕該 override，而非假裝強制。這正是你想要的誠實 sandbox 姿態；缺口純粹是 unelevated backend 上未強制的 read 軸。
- **write 是真 OS 邊界（B2）**：unelevated 下對 normal-ACL 路徑（含 home dir）以 restricted-token ACL 檢查真擋 —— 對照 Claude Code **無 OS sandbox**。且 OpenAI 誠實把 Everyone-writable 記為「the biggest caveat」並有 runtime 警告，而非 over-claim 完全 confinement。
- **egress 是有揭露的分層模型（B3）**：OpenAI 自家 Windows doc 明說 unelevated 用「environment-level offline controls」「weaker network isolation」「processes could ignore those settings or open sockets directly」，並把 robust 邊界搬進 Windows（offline-user firewall rule、sandbox-user）給 elevated/proxy 路徑（PR #12220）。半開 egress 是已知、已揭露的 no-admin best-effort tier 限制。
- **per-spawn token 帳目乾淨（C1）**：usage 是 `turn.completed` 上一級結構化物件、具名欄位、cache-read 明確 surface；`reasoning_output_tokens`(PR #19308) 純加法的乾淨 schema 演進。
- **顯式 skill 觸發乾淨（D1）**：`$skill` 是真正 native bypass description 評估（docs 親口承認省 token）；progressive disclosure 誠實 —— 前期只載 name+description+path（capped ~2% context / 8000 chars），body 在選中時經 **可見的** `command_execution` tool call 取得，成本可在 event stream 觀察。
- **subagent 的 write/command 面做得好（D2）**：`-s` 是 process-level policy、subagent 是同 process thread，worker 騎同一條 ACL-gated 邊界、零額外 plumbing；docs 明確承諾「Subagents inherit your current sandbox policy」。`--disable multi_agent` 提供俐落關閉桿。
- **per-spawn 可組合隔離（B4）**：單一非互動 `codex exec` 就能剝掉 user config/trust list、per-project execpolicy、7 個 stable feature 表面，**零 trust/requirements prompt**、不需 global trust-the-folder 狀態 —— 這正是讓 codebus 能安全跑「不可信 codebase 分析」的關鍵。

---

## 8. 還沒在 0.136 重量測的（唯一殘留 stale-risk）

| 項目 | 測量版本 | 為何沒重跑 | 風險 |
|---|---|---|---|
| skill `$` vs implicit 的 token 比（D1） | 0.133.0、**n=1** | 是 token 計量、非 sandbox 邊界；要真 API 才精準 | **MEDIUM —— 數字勿引舊 24.8%**，要嘛現場重量測、要嘛只講方向（顯式 `$` 較便宜） |

> 其餘 sandbox 邊界（read / write / egress / subagent）已於 2026-06-03 全部在 0.136.0 重跑確認，可當定論講。

---

## 9. 相關檔案（絕對路徑）

codebus repo：
- `D:\side_project\codebus\codebus-core\src\agent\process_kill.rs`（tree-kill；自家 doc 註解仍誤寫 node.exe、待修）
- `D:\side_project\codebus\codebus-core\src\agent\codex_backend.rs`（recipe :146-192、stdin :279-300、Cumulative :311-315、multi_agent 不 disable :122-124）
- `D:\side_project\codebus\codebus-core\src\log\sink.rs`（last-wins :296、test :308-326）
- `D:\side_project\codebus\codebus-core\src\stream\codex_parser.rs`（reasoning_output_tokens :79）
- PoC：`scripts/codex_sandbox_read_poc.py`、`scripts/codex_subagent_mock_escape.py`、`scripts/codex_subagent_write_oracle_control.py`

research repo（`agent-cli-research/poc/codex-sandbox/`）：
- `codex_write_egress_poc.py`、`codex_write_acl_poc.py`、`codex_hooks_poc.py`
- 0.135 基準 JSON：`rerun-0.135.0.json`、`write-egress-0.135.0.json`、`write-acl-0.135.0.json`、`SUMMARY.md`
- **0.136 重跑 JSON（本輪產出）**：`egress-0136.json`、`acl-0136.json`，codebus repo `target/read-poc-0136.json`、`target/subagent-control-0136.json`、`target/subagent-escape-0136.json`
