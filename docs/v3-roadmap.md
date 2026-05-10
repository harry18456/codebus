# codebus v3 — Path D Redesign Roadmap

> 起草：2026-05-08。本檔是後續所有 v3 change 的指引。每個 `/spectra-propose` 動工前先 reread。

## 1. Context

V3 第一次嘗試（commit `640de61 feat: v3 skeleton ...` + `762541e feat: init auto-mutates ...`）已 `git reset --hard e877adc` 回退。原因：方向偏差。

具體偏差：

- 沒讀 [`legacy/v2-rust/Cargo.toml`](../legacy/v2-rust/Cargo.toml) workspace 結構，做成 single binary crate
- 沒讀 [`legacy/v2-rust/codebus-core/src/config/`](../legacy/v2-rust/codebus-core/src/config/)（v2 投入最大的模組之一，1006 行嚴格 tolerance），做了 150 行平凡 config
- Spec 寫滿「day-1 vendor neutral trait surface / `AgenticProvider` / `AgentEvent` / `vault::query` 4-fn」抽象，**zero second-impl 驗證**
- Goal/query/fix subcommand 自己 spawn `claude -p`（v2 模式），跟 path D 「skill mode」願景背道而馳
- Schema 在 SKILL.md 跟 inline prompt 雙投遞，source-of-truth 模糊

戳穿記錄留在 4 條 memory feedback：

- [feedback_dont_speculative_abstract.md](file:///C:/Users/harry/.claude/projects/D--side-project-codebus/memory/feedback_dont_speculative_abstract.md) — single-impl trait 不寫進 Requirements
- [feedback_no_artificial_checkpoint.md](file:///C:/Users/harry/.claude/projects/D--side-project-codebus/memory/feedback_no_artificial_checkpoint.md) — `/spectra-apply` 一路跑完
- [feedback_dev_tool_claude_only.md](file:///C:/Users/harry/.claude/projects/D--side-project-codebus/memory/feedback_dev_tool_claude_only.md) — codebus repo root 不放 `AGENTS.md`
- [feedback_grounded_debugging.md](file:///C:/Users/harry/.claude/projects/D--side-project-codebus/memory/feedback_grounded_debugging.md) — bug 連續猜 3 次停下別模擬

## 2. Vision

V3 走 [`legacy/v2-rust/docs/strategy/2026-05-08-skill-vs-binary-pivot.md`](../legacy/v2-rust/docs/strategy/2026-05-08-skill-vs-binary-pivot.md) §11.6「長期 pivot」path D：codebus 是 vault helper + skill installer。三個動詞 verb（goal / query / fix）走 **spawn `claude -p` 帶 slash command 觸發對應 skill bundle**，由 SKILL.md 內容指揮 agent 流程；不再像 v2 把整份 schema inline 進 prompt。schema 是 SKILL.md 唯一交付，binary 不再 inline。

### Architecture

```
codebus binary（cargo workspace）
├─ codebus-core（lib）── vault primitives / wiki lint / config / schema 內容
├─ codebus-cli（bin codebus）── 5 verb subcommands（init / goal / query / lint / fix）
└─ codebus-app（bin codebus-app, placeholder）── Tauri 預留

CLI 行為分兩類：
  Direct（binary 自跑、deterministic）：
    - codebus init / no-arg → init at pwd
    - codebus lint [--json]
  Spawn（binary fork claude -p、slash command 觸發 skill bundle）：
    - codebus goal "..."  → spawn `claude -p "/codebus-goal \"...\""`
    - codebus query "..." → spawn `claude -p "/codebus-query \"...\""`
    - codebus fix         → spawn `claude -p "/codebus-fix"`

  cwd = <repo>/.codebus；spawn 同時下 --tools + --allowedTools 雙旗（v2
  iter-9 lesson，見 legacy/v2-rust/docs/superpowers/specs/2026-05-04-codebus-v2-phase1-design.md
  §3.2.4）。各 verb toolset：
    - query：Read,Glob,Grep（read-only）
    - goal ：Read,Glob,Grep,Write,Edit
    - fix  ：Read,Glob,Grep,Write,Edit,Bash（fix skill 跑 codebus lint --json）
    - 永遠擋：WebFetch / WebSearch / AskUserQuestion / Task / NotebookEdit / MCP / 未來新增
  不下 --add-dir → agent 出不去 vault。

3 個 skill bundle 寫進 ~/.claude/skills/：
  codebus-goal/  ── workflow per goal（schema rules + ingest 流程）
  codebus-query/ ── workflow per query（read-only）
  codebus-fix/   ── lint loop（Bash tool 跑 codebus lint --json，agent 改 wiki，重 lint）

Provider 模組：codebus-core/src/agent/claude_cli.rs，single impl，**不寫 trait**。
InvokeOptions struct 直接吃 verb 需要的參數（slash_command / toolset / cwd /
model / effort）。trait surface 等到 codex / gemini 等 second impl 真的進來
再開 change 設計（依 §3 anti-pattern #1：no single-impl abstraction in spec）。

每個 vault：
  <repo>/.codebus/CLAUDE.md  ── per-repo schema（user 可改、agent 在進 vault 時讀）
  <repo>/.codebus/wiki/      ── 5-folder taxonomy
  <repo>/.codebus/raw/code/  ── PII-filtered source mirror
  <repo>/.gitignore          ── auto append .codebus/（init 階段做）
```

### Lint = CLI（不是 skill）

Lint 邏輯純 deterministic（7 條 rule pattern match）。優點：

- CI / pre-commit hook 可直接用，不依賴 Claude Code
- `codebus-fix` skill 透過 Bash tool 跑 `codebus lint --json` 拿 findings，不需 skill-call-skill
- Skill 只 3 個（goal/query/fix），維護面縮小

## 3. Anti-Patterns（一次都別再犯）

引用 memory：

1. **Spec 不寫 single-impl 抽象**：trait / API surface / enum variant 沒有 2+ impl 或 consumer 驗證的，寫進 design.md open questions 就好；不寫 normative Requirements
2. **Schema 不雙投遞**：每份 schema 內容**只有一個 source of truth**。SKILL.md 跟 inline prompt 不可同份內容。Path D 下 SKILL.md 是唯一交付，binary 不再 inline schema
3. **Carry over v2 之前先 grep v2**：每個 change 動工前讀 v2 對應 module / spec，不靠記憶猜行為
4. **`/spectra-apply` 不亂 checkpoint**：一路跑完 tasks.md，除非設計 / 環境真卡

## 4. Change Decomposition

10 個 change，序列做。每個 ≤ 14 tasks。**全 10 條已於 2026-05-10 ship v3.0.0 完成**（commit `6936902 chore(release): v3.0.0`）— 表格保留供歷史對照。

| # | Status | Change | CLI 完成什麼 | Skill 完成什麼 | 依賴 |
|---|---|---|---|---|---|
| 1 | ✅ | `v3-workspace` | `codebus --help` + 5 verb routing（subcommand mode、no-arg → init at pwd）；含 `codebus-app` placeholder crate | — | — |
| 2 | ✅ | `v3-init` | `codebus init [--repo X] [--no-obsidian-register]` 全功能：vault layout / raw_sync (NullScanner) / **obsidian vault register** / `.gitignore` mutation / per-repo `.codebus/CLAUDE.md` / 寫 3 個 skill bundle 骨架；含 `sanity_check::check_repo_is_not_vault` | 3 個 SKILL.md 骨架寫到 `~/.claude/skills/codebus-{goal,query,fix}/`（內容暫定，後續 change 補） | #1 |
| 3 | ✅ | `v3-pii` | 接 `pii::PiiScanner` trait + `RegexBasicScanner`（v2 builtin regex：AWS / Anthropic key / email / IPv4）+ `NullScanner`（test fixture）；raw_sync default 切 RegexBasic with `OnHit::Warn`（mirror file + stderr warn 每個 match）；init 輸出含 PII match count。**Hardcode default rules 不開 config 入口** — `patterns_extra` / `on_hit` 覆蓋是 #9 的事 | — | #2 |
| 4 | ✅ | `v3-vault-history` | 接 `core::git::nested_repo` 模組（v2 carry：`legacy/v2-rust/codebus-core/src/git/nested_repo.rs`）；init.rs 在 raw_sync 之後 obsidian-register 之前 init nested repo + 收尾 `auto_commit "init: codebus vault"`；`.codebus/.gitignore` 內含 `.lock` / `raw/code/` / `**/.obsidian/` / `logs/`（v2 carry）；公開 `auto_commit` API 給 #5 #8 wire；vault spec 反轉「SHALL NOT create `.codebus/.git/`」requirement | — | #3 |
| 5 | ✅ | `v3-goal` | `codebus goal "..." [--force-resync] [--no-obsidian-register]` spawn `claude -p` 帶 slash command；首次寫 `codebus-core/src/agent/claude_cli.rs` single impl + `--tools/--allowedTools/--permission-mode acceptEdits` 三旗 sandbox（[2026-05-09 spike verified](#)）；vault 不存在 → auto-init（v2 carry）；source-signal detection 偵測 source drift 才 re-sync（manifest.git_head/file_count/total_bytes 比對）；spawn 收尾 `auto_commit "wiki: {goal}"` | `codebus-goal/SKILL.md` 補完整內容（neutral.md §4 workflow per goal + frontmatter schema reference；schema 仍 by-reference 引用 cwd `CLAUDE.md` 不 inline 重複） | #4 |
| 6 | ✅ | `v3-query` | `codebus query "..."` spawn 同 #5 模式，read-only toolset（Read/Glob/Grep）；不 auto_commit | `codebus-query/SKILL.md` 補完整內容（neutral.md §11 workflow per query + read-only invariant） | #2 |
| 7 | ✅ | `v3-lint` | `codebus lint [--repo X] [--json]` direct 全功能（7 rules：broken_wikilink / frontmatter_integrity / page_size / duplicate_slug / orphaned_page / taxonomy_violation / pii_leak）；human + JSON 雙輸出；exit 0/1 | — | #2 |
| 8 | ✅ | `v3-fix` | `codebus fix` spawn 同 #5 模式，toolset 含 Bash（給 fix skill 跑 `codebus lint --json`）；spawn 收尾 `auto_commit "wiki: lint fix loop"` | `codebus-fix/SKILL.md`：用 Bash tool 跑 `codebus lint --json` → 解析 findings → 編輯 wiki page 改正 → 重跑 lint，max 5 iterations（user 可改） | #4 #7 |
| 9 | ✅ | `v3-config` | `~/.codebus/config.yaml` 6 條 tolerance（v2 carry：missing file / parse fail / unknown key / unknown discriminator / unknown subfield / type mismatch graceful warn）；`lint` section（disabled_rules + custom_rules_dir + auto_fix.max_iterations）；`pii` section（`patterns_extra` append 到 #3 builtin rules + `on_hit` 覆蓋 #3 default `Warn`） | 反向打通：lint 吃 config disabled_rules；init 用 config 覆蓋 #3 PII default（`patterns_extra` append、`on_hit` 覆蓋）；fix skill 從 config 讀 max_iterations 寫進 SKILL.md instruction | #3 #7 |
| 10 | ✅ | `v3-render-polish` | OSC 8 hyperlink wrap `[[wikilink]]` for `codebus lint` output；terminal color 5-level emoji priority（`--emoji` flag > `--no-emoji` > `NO_EMOJI` env > config.yaml `emoji:` > TTY auto-detect）；`NO_COLOR` env 守 color | — | #7 #9 |

### 依賴圖

```
#1 ─┬─ #2 ─┬─ #3 ─ #4 (vault-history) ─┬─ #5 (goal)
    │      │                            └─ #8 (fix) ← + #7
    │      ├─ #6 (query)
    │      └─ #7 (lint) ─┬─ #8 (fix)
    │                    └─ #10 (render-polish) ← + #9
    │
    └─ #9 (config，吃 #3 #7)
```

實務：1 → 2 → 3 → 4 → 5 → 6 → 7 → 8 → 9 → 10 一條過。

### Follow-up changes（非主 10 條序列）

下面這些 nice-to-have 不在 ship-blocking path 上，等主序列推進到對應 trigger 點再開。**狀態欄是事實 source-of-truth，動工前先看這欄不要再憑印象**：

| Status | Change | 觸發點 | 內容 / 結果 |
|---|---|---|---|
| ⏳ Pending | `v3-multi-agentic-provider` | §9 trigger（real user 反映 / Anthropic 出事 / 贊助 / Tauri demo 想 multi-vendor） | 第二個 provider impl（codex / gemini / 其他）真的要進來時，先 spike：對方 CLI 有沒有 user-invocable slash command 機制？toolset gate 機制是什麼（Claude=`--tools`、Codex=docker/chroot 等）？驗完才設計 trait surface 或 enum dispatch。在那之前 provider 模組保持 single impl。|
| ✅ Done 2026-05-10 | `v3-run-log` (commits `9187da5` + `adb88c2`) | user 想看每個 verb 跑了多少 token、費了多少時間（v2 carry：`<vault>/.codebus/log/runs-<date>.jsonl` 含 goal / mode / model+effort / 時戳 / token usage / wiki_changed / lint counts） | 走 (A) 完整 v2 stream renderer port：`Stdio::piped()` + parse `--output-format stream-json --verbose` → render Thought/ToolUse/ToolResult 即時到 stdout + accumulate Usage 入 RunLog；jsonl date-rotate 一檔 / `sink: none` opt-out / sink 失敗 stderr warning + verb exit code 不變。Banner 系統如預期正交未動。Manual e2e 對 `D:/side_project/uv` 4 條 scenario 全綠（見 `docs/v3-uv-verification-2026-05-10.md` 附錄）。|
| ✅ Done 2026-05-10 | `v3-bug-fixes` (commit `87e9b0c`) | v3-render-polish 後 UV repo 驗收（`docs/v3-uv-verification-2026-05-10.md`）暴露的 2 個非 BREAKING bug | (a) `init` 緊接 `goal` 不該觸發 re-sync — `walk_source_for_signal()` 與 `sync_with_scanner()` filter rule 對齊；(b) `codebus lint --repo <vault-root>` silently 回 `0 pages, no issues` — 偵測到 vault root 時 emit error 提示。|
| ✅ Done 2026-05-10 | `v3-pii-severity-dispatch` (commit `c4f3d30`) | UV repo 驗收暴露：default `on_hit: mask` 對 docs/test 的 `127.0.0.1` / example email 過於激進（uv 觸發 672 hits 多為 false-positive），降低 wiki agent 對源碼可讀性 | severity-dispatched on_hit + Critical floor — Critical (AWS / Anthropic key) 強制 mask、Warn (email / ipv4) 走 user-config，wiki agent 對源碼可讀性回升。|

另外有一條**純 docs 工作不需要開 spectra change**，**直接 commit 到 README**：

- ✅ Done 2026-05-10 — **`docs(quickstart): require codebus on PATH for fix loop`** (commit `e4e8bfa`) — UV repo 驗收暴露：fix flow spawned agent 內部跑 `Bash(codebus lint *)` 時找不到 binary（CLI 最終 check 仍 OK）。已在 README quickstart 補一條 `cargo install --path codebus-cli` 與「為何 fix 需要 PATH 上有 codebus」的說明。如果之後想自動化（init 寫 `.claude/settings.json` 注 PATH），再開 `v3-fix-path-inject` change。

### v3.x 再來的可能 follow-up（v3.0.0 ship 後新冒出的）

| Status | Change | 內容 |
|---|---|---|
| 💭 Idea | `v3-fix-path-inject` | init 自動注 PATH 進 `.claude/settings.json`，免 user 手動 `cargo install`。等真的有 user 抱怨 fix 跑不起來再開。|
| 💭 Idea | bump v3.1.0 | v3-run-log 是 v3.0.0 ship 後新加的 backwards-compatible feature（stream rendering + RunLog 持久化），按 semver 是 minor bump。決定前先看是否還有 v3.x 想攢一起。|

## 5. 累積里程碑

- **#1 結束**：CLI shell 通；3-crate workspace 編譯
- **#2 結束**：可以對任何 git repo 跑 `codebus`（no-arg）→ vault 立刻成形 + Obsidian app 看得到 + `~/.claude/skills/codebus-*/` 出現 3 個 skill 骨架（內容空）
- **#4 結束**：`.codebus/` 含 nested git；init 結束有第一個 commit「init: codebus vault」；`auto_commit` API 公開可被後續 verb 收尾 wire
- **#6 結束**：goal / query 兩個 skill 內容完整，可以在 Claude Code 用 `/codebus-goal "..."` / `/codebus-query "..."` 對 vault 做事；goal 收尾 commit 進 nested git；`init` + 2 個有用 skill = 第一個能對外展示的版本
- **#8 結束**：`fix` skill 也通；`lint` direct CLI 配 `fix` skill = 完整 wiki 維護 loop；fix 收尾 commit 進 nested git
- **#10 結束**：UX polish 到位（color + clickable wikilink），可發 v3.0.0

## 6. Open Questions（每個 change 各自 design.md 處理）

- **#2**：per-repo `.codebus/CLAUDE.md` 寫的時候要不要 `if missing`（v2 phase 1 task 11.1）保 user 客製化？答：應該。
- ~~**#3**：v2 PII filter 有 3 種 `on_hit` mode（Warn / Skip / Mask）。v3 default 走哪個？~~ ✅ resolved 2026-05-09：#3 hardcode `OnHit::Warn` default（v2 carry：mirror file + stderr warn 每個 match）；Skip / Mask 切換要等 #8 開 `pii.on_hit` config 入口。
- **#4 / #5 / #7**：3 個 skill 的 `description:` 字串怎寫（影響 Claude Code 自動 activation）？需驗證。
- ~~**#4 propose 前剩餘 spike**~~ ✅ resolved 2026-05-09：5 組對照 spike（`/_pii-toolgate-spike` test skill 強迫 Write）證實 v2 iter-9 lesson 在 `-p + slash + skill activation` 場景下完整有效。**Triple flag** 都要下 — `--tools <list>` (toolset whitelist hard gate) + `--allowedTools <same list>` (auto-approval) + `--permission-mode acceptEdits` (-p mode 沒 terminal 必須 bypass prompt)。關鍵 case：`--tools` 不含 Write 但 `--allowedTools` 含 Write + acceptEdits → **Write 仍被 hard-gate 擋**（file 未建）。spike 細節保留於 git commit history。
- **#7**：fix loop 的 max_iterations 是 hardcoded 5（v2 default）還是必須走 config？建議先 hardcoded，#8 再讓 config 覆蓋。
- **#8**：v2 config 的 `llm` / `render` / `log` section 在 path D 沒用，spec 直接 retire 還是保留 forward-compat 接收（unknown discriminator 走 graceful warn）？建議**只實作 lint / pii section**，其他 section 照 tolerance rule 「unknown top-level key 靜默忽略」處理。
- **#9**：OSC 8 hyperlink 在 `codebus lint` JSON output 模式要不要做？答：JSON 模式不要（machine-readable）；human 模式才做。

## 7. Tauri 來時要做什麼

`codebus-app` 在 #1 就是空殼。Tauri tutorial app 真動工時新開 change `tauri-app-bootstrap`，動到：

- `codebus-app/Cargo.toml` 加 `tauri` deps
- `codebus-app/src/`：Tauri command 包 `codebus-core::vault::query` / `codebus-core::wiki::lint`（這些 query API 在 path D 沒做，等需要時再開 change `vault-query-api`）
- `codebus-app/tauri.conf.json` 等 Tauri 標配

不在這 9 個 change 範圍內。
