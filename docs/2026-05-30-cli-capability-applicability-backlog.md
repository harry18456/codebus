# CLI 能力研究回收 — codebus opportunity 盤點（2026-05-30）

把 claude/codex CLI 能力研究（外部 `agent-cli-research/FINDINGS.md`，A–I 八組、Windows 11 non-admin / codex-cli 0.135.0）反向對照 codebus 實際程式碼，找出的 opportunity。

- **產出方式**：44-agent dynamic workflow，每條經對抗式 verify（實際 grep codebus + 讀 FINDINGS）；28 條 → 20 confirmed / 8 revised / 0 rejected，完整性審查補 8。
- **完整分析（含每條 verdict grounding）**：外部 `D:/side_project/agent-cli-research/CODEBUS-APPLICABILITY.md`（commit `4cee064`）+ workflow 原始 JSON。本檔是 codebus-local 的行動版，grounding 用 codebus 自己的 `file:line`。
- **適用範圍**：codex 沙箱結論皆 Windows non-admin / 0.135.0；macOS/Linux 未測，不外推。

> **兩個改變嚴重度的反框架（先讀）**
> 1. codebus 第一道讀取防線不是沙箱、是 `pii/` scanner + `raw_sync` 的 **PII-redacted mirror**（agent 讀 `raw/code/` 鏡像、非 live repo）→ 框住既有 row 23（codex %USERPROFILE% 讀漏）的實際嚴重度。security.md 揭露 codex read-leak 時應同段提這道架構防線。
> 2. codebus 早已用 `content_verify` 迴圈做多 spawn 角色分工編排（spawn 唯讀 verify agent `resolve_as=Verify` + repair agent、`CONTENT_VERIFY_CAP=3`、provider-neutral）→ 既有 row 18「加 subagent 委派」的核心價值其實已有；真機會是泛化它。

---

## Tier 1 — Quick wins（小工程、值得直接做）

### T1-1 · per-run wall-clock timeout（hang / 無人值守兜底）
- **觀察**：`invoke()` 主迴圈用阻塞 `BufReader::lines()` 讀 stdout，cancel 只在 caller 主動翻 cancel flag 時觸發 tree-kill；**沒有任何 wall-clock 計時器**。watcher thread 只輪詢 `cancel`/`done` 兩 flag。claude/codex CLI 本身都不提供 per-run timer（FINDINGS group F；官方建議外部 wrapper，codebus 就是那個 wrapper）。
- **Grounding**：`codebus-core/src/agent/claude_cli.rs:96-261, 293-314`（無 timeout 分支）；`spawn_spec.rs:94-115`（無 timeout 欄位）。
- **Proposed fix**：watcher 加第三分支 `started_at.elapsed() > limit → KillHandle.terminate_tree()`（tree-kill 機制已現成），標 `outcome=failed` + 新增 `interrupt_reason=Timeout` variant（接進既有 `RunLog.interrupt_reason` taxonomy）。limit 走 config（如 `lifecycle.run_timeout_sec`，預設 None=不限、維持現狀）。
- **工程量**：輕（小）。**value: high**（provider-neutral circuit breaker；turn-count cap 不追—claude max_turns 是 SDK-only、codex 無）。

### T1-2 · codex 內層 `is_error` 餵進 outcome（頂層 exit 0 遮蔽 sandbox denial）
- **觀察**：codex `exec` 頂層 process exit 即使內層 shell 指令被 sandbox 拒絕仍可能是 0（與 read-leak-exit-0 同遮蔽模式，PoC-verified 0.135.0）。codebus 對 codex 路徑**只信頂層 exit code** 判 outcome；`codex_parser` 雖已把 per-item `exit_code` 解成 `ToolResult.is_error`，但只進 stream 渲染、**沒回饋 outcome**。→ 被擋下的 run 會誤標 succeeded。
- **Grounding**：`codebus-core/src/verb/goal.rs:589-601`（outcome 只讀 `invoke_report.exit.code()`）；`codebus-core/src/stream/codex_parser.rs:33-55`（`is_error` 已解出）；`codebus-core/src/render/stream_event.rs:78-99`（只進渲染）。
- **Proposed fix**：codex 路徑聚合 per-item `is_error`（或非零 inner exit）成一個訊號餵進 outcome——至少在 RunLog 記「inner-command denials: N」計數，或對「期望寫入卻 mutation 指令 is_error」降級為 failed。避免把正常的 grep-no-match 誤判。claude 路徑無此問題（`permission_denials` 走 stream）。
- **工程量**：中（需定義哪些 inner failure 該影響 outcome）。**value: medium**。訊號已在手、只差接線。

### T1-3 · token 累加器對 codex 改 cumulative-replace（潛伏 double-count）
- **觀察**：`invoke()` 對每個 `StreamEvent::Usage` 用 `saturating_add` 求和；但 codex `turn.completed.usage` 是 **cumulative-replace 非 per-turn delta**（FINDINGS A + codebus 既有 PoC）。claude `result.usage` 一次性、求和 OK；codex 若一次 spawn emit >1 `turn.completed` 就 double-count（目前只觀察到 1 筆 → 潛伏）。
- **Grounding**：`codebus-core/src/log/sink.rs:203-212`（saturating_add）；`codebus-core/src/agent/claude_cli.rs:211-214`（每筆都 accumulate）；`codebus-core/src/stream/codex_parser.rs:69-82`。呼應 memory `project_codex_usage_cumulative_not_per_turn_delta`（前次只標 frontend）。
- **Proposed fix**：accumulation 改 provider-aware——codex Usage 用 cumulative-replace（覆寫成最後一筆）、claude 維持 sum；或 parser 給 Usage event 標 delta vs cumulative。加 unit test 餵兩筆 codex `turn.completed` 鎖 last-cumulative。**先在 0.135.0 重驗 cumulative-replace 仍成立**（前次量在 0.133-0.134）。
- **工程量**：輕（小）。**value: medium**。

### T1-4 · claude 非 chat verb 加 `--no-session-persistence`（orphan session）
- **觀察**：codex 端非 chat verb 都帶 `--ephemeral` 避免留 state；claude 端 `compose_claude_cmd` **無對應**，每次 goal/query/fix/quiz 都在 user claude 目錄留 session rollout（無界磁碟成長 + prompt/wiki 內容快取的隱私足跡），而那些 verb 從不 resume。
- **Grounding**：`codebus-core/src/agent/claude_cli.rs:352-412`（argv 組裝、無此 flag）；對照 `codebus-core/src/agent/codex_backend.rs:144-146`（`if !matches!(spec.verb, Verb::Chat)` gate）。FINDINGS B：`--no-session-persistence` PoC-verified（--help，print 模式限定）。
- **Proposed fix**：claude argv 對非 chat verb 鏡像 codex 的 gate 加 `--no-session-persistence`；chat 維持持久化讓 `--resume` 可用。一行 conditional（backend 已有 verb 資訊）。
- **工程量**：輕（小）。**value: medium**。

### T1-5 · Rust loader 補 effort enum 校驗（CLI / 手改 yaml 漏閘）
- **觀察**：claude effort 是恰好 5 級閉集（low/medium/high/xhigh/max）。codebus 只在 frontend Save 閘 enum；走 CLI 入口或手改 `~/.codebus/config.yaml` 時 Rust loader 不擋非法 effort，要等 spawn 後 CLI 自拒才失敗。
- **Grounding**：`codebus-core/src/config/endpoint.rs:74-79`（`effort: String`、註解明寫交給 CLI 驗）；`codebus-core/src/config/codex.rs:24-30`；frontend `codebus-app/src/lib/ipc.ts:384-407`（只 GUI Save 閘）。
- **Proposed fix**：在 `validate_system_profile`/`validate_azure_profile` 加可選 effort 閉集校驗，把 GUI 閉集語意下移到 core。**只對 claude 做**（codex effort full enum 未列全）；model 名維持寬鬆 forward-compat 不連帶閉集化。
- **工程量**：輕（小）。**value: low**（現況最終仍會失敗、非靜默成功）。

---

## Tier 2 — 先 spike / verify（安全或正確性缺口）

### T2-1 · skill bundle drift 守門（codex body 由 6× str.replace 衍生、靜默留 stale claude 文字）
- **觀察**：codex SKILL body 不是獨立維護，是用 `claude_to_codex_translate` 的 **6 個逐字 `str.replace`** 從 claude body 衍生（`CLAUDE.md→AGENTS.md` + 5 段整段替換）。改了 claude 段落、對應 `replace` 不再 match → codex body **靜默保留過時的 claude 文字**，包含 claude-only 機制描述（`--tools`/PreToolUse hook/heredoc）——這些在 codex 路徑是**假的**。無任何測試斷言 replace 真的 fire。
- **Grounding**（已自驗）：`codebus-core/src/skill_bundle/mod.rs:211`（`claude_to_codex_translate`）、`:215/220/226/236/247/256`（6× replace）、`:329`（dispatch）。
- **Proposed fix**：加測試斷言每個 `replace` 確實命中（替換前後不同 / target 子字串存在）+ 共用段在 `.claude` vs `.codex` SKILL、`CLAUDE.md` vs `AGENTS.md` 之間除白名單 delta 外一致；或改 single-source 產生器。drift 時 fail loud。
- **工程量**：中。**value: high**（失敗模式是「靜默餵錯 provider 指令」）。

### T2-2 · codex `[agents]` subagent 隔離 spike（單一受限 agent 保證只 claude 端驗過）
- **觀察**：codebus「每 spawn 單一受限 agent」的安全保證只在 claude 端驗過（2026-05-21 subagent-sandbox-control：`--tools` 排除 `Task`、子 agent 被 parent `--tools` 天花板雙重 bound）。codex 隔離是 OS sandbox（`-s`）、**不依 tool 名 gate**，且 FINDINGS 記 codex 原生 `config.toml [agents]`（max_threads 6 / max_depth 1）。→ codex **可能** spawn 出不受 codebus 控制天花板約束的子 agent，codebus 從未驗這條路。
- **Grounding**：`codebus-core/src/verb/goal.rs:58-66`（toolset 無 Task，claude-only 保證）；`codebus-core/src/agent/codex_backend.rs:219-225`（command_allowance 只 warn）；既有「已決定不做」row（subagent-sandbox-control 是 claude-only 結論）。
- **Proposed fix**：在 codebus 確切 codex flag（`--ignore-user-config --disable apps --ignore-rules -c project_root_markers -s read-only`）下做 ground-truth spike：codex agent 能否 spawn 子 agent、子 agent 是否仍受 `-s`/root-pin 約束。能逃逸則 security.md §5 記 caveat、或加 config 關掉 `[agents]`。
- **工程量**：中。**value: high**（provider-abstraction 安全保證漏接）。關聯 row 23。

### T2-3 · codex chat resume Windows live round-trip 驗證
- **觀察**：codex 多輪 = 三件耦合（turn1 不帶 `--ephemeral` 留 rollout + resume 改 `-c sandbox_mode=` 因 `codex exec resume` 拒 `-s` + sniff `thread.started` thread_id），目前只逐件 unit-test argv，**沒整條跑過**。任一腿 regress 都會靜默壞掉多輪而其他 test 仍綠。
- **Grounding**：`codebus-core/src/agent/codex_backend.rs:96-159`（resume argv）；`codebus-core/src/verb/chat.rs:67-90, 189-199`。⚠ FINDINGS B 標 codex Windows session-file path 為 Inferred、無 live round-trip。
- **Proposed fix**：跑一次 live 兩輪 codex chat：turn1（fresh `-s`）→ 抓 `thread.started` thread_id → turn2（`exec resume <id>` + `-c sandbox_mode=`）確認 rollout 找得到（無 "no rollout found"）且歷史接上。是 **verification task 非 fix**（code 已對）。
- **工程量**：輕（小）。**value: medium**。

### T2-4 · claude `--bare` / `--setting-sources` 隔離 spike
- **觀察**：claude spawn 目前只隔離 MCP（`--strict-mcp-config` + 空 `--mcp-config`），user 全域 `~/.claude/CLAUDE.md`、全域 settings hooks、ambient skills、keychain 登入**仍可能注入**——跟 codex 路的 `--ignore-user-config` 明顯不對稱。FINDINGS：`--bare` 一次 strip keychain + CLAUDE.md + hooks（auth 降成 API-key only），是更強隔離。
- **Grounding**：`codebus-core/src/agent/claude_cli.rs:388-397`（只 MCP 隔離）；`codebus-core/src/agent/env_overrides.rs:50-53`（Azure scoped env）。
- **Proposed fix**：先 spike——`--bare` 會連 vault 層 `.codebus/.claude/` 的 CLAUDE.md autodiscovery + 自家 hook gate（check-bash/check-read）一起剝，會打掉防護。驗 `--bare` 與自家 hook 相容性；不相容改 `--setting-sources` 精準只保留 project 層、剝 user/全域層。
- **工程量**：中。**value: medium**。

---

## Tier 3 — 新能力（有具體需求再加、不投機抽象）

### T3-1 · structured output（`--json-schema` / `--output-schema`）
少數真正跨 provider 對等的 A 組能力（claude inline / codex file）。哪天有 verb 需要保證 shape 的結果（quiz plan scope / drift report / structured fix summary），再在 SpawnSpec 加 optional schema 欄位、per-backend 映射、解析 `structured_output` 取代 regex/stream scraping。**現在無 consumer 別先抽**。Grounding：`spawn_spec.rs:93-115`（無 schema 欄位）。工程量中、value medium、deferred。

### T3-2 · codex `--oss --local-provider`（本地模型 profile）
跟 codebus「sandbox-bounded、不外洩 source」契約超契合（離線 + 省 API）。codex provider 目前只 system/azure 兩 profile。⚠ `--oss` 只 --help 驗過、`codex exec`（非互動）相容性 + Windows 未測 → **採用前先 spike** `codex exec --oss --local-provider ollama` 跑 goal verb。Grounding：`codebus-core/src/config/codex.rs:58-63`、`codebus-core/src/agent/codex_backend.rs:168-217`。工程量重、value medium、deferred。

### T3-3 · 泛化既有 `content_verify` orchestrator
不是加 CLI subagent，而是把現有多 spawn 角色分工迴圈（verify/repair、`resolve_as`、降權 toolset、bounded loop）抽成可重用 pattern。承反框架 2。工程量中、value medium、deferred。

---

## Tier 4 — 風險揭露 / 增補既有項（多半 no-action，升級時 re-verify）

- **增補 row 23（codex 隔離）**：(a) PII raw_sync mirror 框住讀漏嚴重度（反框架 1，security.md 揭露時同段提）；(b) `CODEX_AGENTS_SOFT_CONSTRAINT`（禁讀 `~/.ssh` 等的 AGENTS.md 段落）efficacy **2026-05-30 對抗式 workflow 測過＝conditional**（real codex gpt-5.4 Azure、合成 marker、8 框法 × 有/無約束）：with-constraint **leak 0/8**；唯一乾淨 A/B（良性檔名 `project-notes.txt`、由 codex 自主列舉 home 撈到）**由 leak 翻成 scope-refuse**、但 **n=1**。**意外發現**：codex 對 `id_rsa` 類檔名有**內建 credential guard**（無約束也拒、與 soft constraint 無關）形成第二層、但**良性檔名的 home secret 只剩 soft constraint 獨守**。`-s read-only` 本身不擋讀、soft constraint 做 100% 讀取限制工作。→ **solo-dev 建議：接受殘餘風險 + security.md 誠實記（別寫成 hard boundary）**；AppContainer/LowBox 留升級路徑，觸發＝外部/不可信 prompt 來源出現、或良性檔名 probe 打穿。Caveats：樣本小、id_rsa confound、單 model、Windows only。harness：`agent-cli-research/poc/codex-soft-constraint/run_probe.ps1`、verdict：workflow `wh3qk2a1v`。(c) codex subagent 隔離=T2-2。
- **codex PreToolUse hook 在 `exec` 不 fire**（C-1）：已 document，**別在 `.codebus/.codex/` 放假 hook 製造防護假象**。
- **`--ignore-rules` 關掉 codex 唯一 per-command gate**（C-2）：document 成有意識取捨。⚠ `.rules` 是 docs 非 PoC、且 `-s` 仍是一層 gate。
- **metachar denylist 脆弱**（C-3）：document 真正 load-bearing 的是 `--allowedTools Bash(<prefix> *)`、denylist 是次層；加 regression test（含 heredoc 形狀），別反應式擴 denylist（已害 quiz heredoc 一次）。關聯 row 24（Bash hook integration test）。
- **codex `$skill` ~25% token 省 + `.codex/skills` 接受度**（H-2/H-3）：量在 0.133.0、未在 0.135.0 重跑 → 升級時 re-verify，別把 24.8% 當現值引用。
- **env-var 注入契約 parity + 未公開 flag version-watch**（補 a）：claude `ANTHROPIC_API_KEY` vs codex `CODEBUS_CODEX_AZURE_KEY` 無中性抽象；`CLAUDE_CODE_DISABLE_ADVISOR_TOOL=1` 未公開但 load-bearing（Azure 沒它 400）→ Claude 升級可能無聲壞掉、無 test 接。Grounding：`env_overrides.rs:50-53`。
- **git auto-commit partial-failure**（補 b）：agent 中途 crash → working tree dirty 但無 commit → 下次 `changed_paths_under` baseline 錯。Grounding：`codebus-core/src/git/`。

## 誠實標記（對抗式 verify 下修）
B-2 / C-2 / I-4 的「capability 宣稱」只有 `--help`/docs、非完整 PoC → 是 verification task 非 fix。完整性審查（critic）跑在 verify 之後、本身未被對抗式驗，其細節 claim（如 T2-1 的 str.replace）已自 grep 驗過才入此檔。

## No-action（記錄成已知 asymmetry）
bidirectional stream input（claude-only、用不到）；caller-supplied session id（無 consumer）；read-only researcher subagent（content_verify 已涵蓋核心）；body materialization 成本不對稱（純揭露、數 tool call 的測試要 provider-aware）。
