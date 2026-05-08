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

V3 走 [`legacy/v2-rust/docs/strategy/2026-05-08-skill-vs-binary-pivot.md`](../legacy/v2-rust/docs/strategy/2026-05-08-skill-vs-binary-pivot.md) §11.6「長期 pivot」path D：codebus 是 vault helper + skill installer，**不再 spawn `claude -p`**。

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
  Trigger（印給 user 貼進 Claude Code）：
    - codebus goal "..." → echo /codebus-goal "..."
    - codebus query "..." → echo /codebus-query "..."
    - codebus fix         → echo /codebus-fix

3 個 skill bundle 寫進 ~/.claude/skills/：
  codebus-goal/  ── workflow per goal（schema rules + ingest 流程）
  codebus-query/ ── workflow per query（read-only）
  codebus-fix/   ── lint loop（Bash tool 跑 codebus lint --json，agent 改 wiki，重 lint）

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

9 個 change，序列做。每個 ≤ 14 tasks。

| # | Change | CLI 完成什麼 | Skill 完成什麼 | 依賴 |
|---|---|---|---|---|
| 1 | `v3-workspace` | `codebus --help` + 5 verb routing（subcommand mode、no-arg → init at pwd）；含 `codebus-app` placeholder crate | — | — |
| 2 | `v3-init` | `codebus init [--repo X] [--no-obsidian-register]` 全功能：vault layout / raw_sync (NullScanner) / **obsidian vault register** / `.gitignore` mutation / per-repo `.codebus/CLAUDE.md` / 寫 3 個 skill bundle 骨架；含 `sanity_check::check_repo_is_not_vault` | 3 個 SKILL.md 骨架寫到 `~/.claude/skills/codebus-{goal,query,fix}/`（內容暫定，後續 change 補） | #1 |
| 3 | `v3-pii` | raw_sync 換 `RegexBasicScanner`（v2 同套 regex：AWS / Anthropic key / email / IPv4 / 自訂 patterns_extra）；init 輸出含 PII redaction count | — | #2 |
| 4 | `v3-goal` | `codebus goal "..."` 印 `/codebus-goal "..."` trigger | `codebus-goal/SKILL.md` 補完整內容（neutral.md §4 workflow per goal + frontmatter schema reference） | #2 |
| 5 | `v3-query` | `codebus query "..."` 印 trigger | `codebus-query/SKILL.md` 補完整內容（neutral.md §11 workflow per query + read-only invariant） | #2 |
| 6 | `v3-lint` | `codebus lint [--repo X] [--json]` direct 全功能（7 rules：broken_wikilink / frontmatter_integrity / page_size / duplicate_slug / orphaned_page / taxonomy_violation / pii_leak）；human + JSON 雙輸出；exit 0/1 | — | #2 |
| 7 | `v3-fix` | `codebus fix` 印 trigger | `codebus-fix/SKILL.md`：用 Bash tool 跑 `codebus lint --json` → 解析 findings → 編輯 wiki page 改正 → 重跑 lint，max 5 iterations（user 可改） | #6 |
| 8 | `v3-config` | `~/.codebus/config.yaml` 6 條 tolerance（v2 carry：missing file / parse fail / unknown key / unknown discriminator / unknown subfield / type mismatch graceful warn）；`lint` section（disabled_rules + custom_rules_dir + auto_fix.max_iterations）；`pii` section（patterns_extra + on_hit policy） | 反向打通：lint 吃 config disabled_rules；init 吃 pii patterns_extra；fix skill 從 config 讀 max_iterations 寫進 SKILL.md instruction | #3 #6 |
| 9 | `v3-render-polish` | OSC 8 hyperlink wrap `[[wikilink]]` for `codebus lint` output；terminal color 5-level emoji priority（`--emoji` flag > `--no-emoji` > `NO_EMOJI` env > config.yaml `emoji:` > TTY auto-detect）；`NO_COLOR` env 守 color | — | #6 #8 |

### 依賴圖

```
#1 ─┬─ #2 ─┬─ #3
    │      ├─ #4
    │      ├─ #5
    │      └─ #6 ─┬─ #7
    │             └─ #9 ─ (after #8)
    │
    └─ #8（其實 #3 #6 都齊後就能 propose，#9 再吃 #8）
```

實務：1 → 2 → 3 → 4 → 5 → 6 → 7 → 8 → 9 一條過。

## 5. 累積里程碑

- **#1 結束**：CLI shell 通；3-crate workspace 編譯
- **#2 結束**：可以對任何 git repo 跑 `codebus`（no-arg）→ vault 立刻成形 + Obsidian app 看得到 + `~/.claude/skills/codebus-*/` 出現 3 個 skill 骨架（內容空）
- **#5 結束**：goal / query 兩個 skill 內容完整，可以在 Claude Code 用 `/codebus-goal "..."` / `/codebus-query "..."` 對 vault 做事；`init` + 2 個有用 skill = 第一個能對外展示的版本
- **#7 結束**：`fix` skill 也通；`lint` direct CLI 配 `fix` skill = 完整 wiki 維護 loop
- **#9 結束**：UX polish 到位（color + clickable wikilink），可發 v3.0.0

## 6. Open Questions（每個 change 各自 design.md 處理）

- **#2**：per-repo `.codebus/CLAUDE.md` 寫的時候要不要 `if missing`（v2 phase 1 task 11.1）保 user 客製化？答：應該。
- **#3**：v2 PII filter 有 3 種 `on_hit` mode（Warn / Skip / Mask）。v3 default 走哪個？v2 default `Warn`（mirror file + stderr warn 每個 match）。建議 carry default。
- **#4 / #5 / #7**：3 個 skill 的 `description:` 字串怎寫（影響 Claude Code 自動 activation）？需驗證。
- **#7**：fix loop 的 max_iterations 是 hardcoded 5（v2 default）還是必須走 config？建議先 hardcoded，#8 再讓 config 覆蓋。
- **#8**：v2 config 的 `llm` / `render` / `log` section 在 path D 沒用，spec 直接 retire 還是保留 forward-compat 接收（unknown discriminator 走 graceful warn）？建議**只實作 lint / pii section**，其他 section 照 tolerance rule 「unknown top-level key 靜默忽略」處理。
- **#9**：OSC 8 hyperlink 在 `codebus lint` JSON output 模式要不要做？答：JSON 模式不要（machine-readable）；human 模式才做。

## 7. Tauri 來時要做什麼

`codebus-app` 在 #1 就是空殼。Tauri tutorial app 真動工時新開 change `tauri-app-bootstrap`，動到：

- `codebus-app/Cargo.toml` 加 `tauri` deps
- `codebus-app/src/`：Tauri command 包 `codebus-core::vault::query` / `codebus-core::wiki::lint`（這些 query API 在 path D 沒做，等需要時再開 change `vault-query-api`）
- `codebus-app/tauri.conf.json` 等 Tauri 標配

不在這 9 個 change 範圍內。
