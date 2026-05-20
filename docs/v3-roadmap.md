# codebus v3 Roadmap

> 起草：2026-05-08（path D pivot 起點）。最後更新：2026-05-20。
>
> 涵蓋 CLI + app 兩條主線。每個 `/spectra-propose` 動工前先 reread。

## 1. Context

v3 第一次嘗試（commit `640de61 feat: v3 skeleton ...` + `762541e feat: init auto-mutates ...`）已 `git reset --hard e877adc` 回退。原因：方向偏差。

具體偏差：

- 沒讀 [`legacy/v2-rust/Cargo.toml`](../legacy/v2-rust/Cargo.toml) workspace 結構，做成 single binary crate
- 沒讀 [`legacy/v2-rust/codebus-core/src/config/`](../legacy/v2-rust/codebus-core/src/config/)（v2 投入最大的模組之一，1006 行嚴格 tolerance），做了 150 行平凡 config
- Spec 寫滿「day-1 vendor neutral trait surface / `AgenticProvider` / `AgentEvent` / `vault::query` 4-fn」抽象，**zero second-impl 驗證**
- Goal/query/fix subcommand 自己 spawn `claude -p`（v2 模式），跟 path D 「skill mode」願景背道而馳
- Schema 在 SKILL.md 跟 inline prompt 雙投遞，source-of-truth 模糊

四條 anti-pattern 從中萃出，至今仍生效（見 §3）。

## 2. Vision

v3 走 [`legacy/v2-rust/docs/strategy/2026-05-08-skill-vs-binary-pivot.md`](../legacy/v2-rust/docs/strategy/2026-05-08-skill-vs-binary-pivot.md) §11.6「長期 pivot」path D：codebus 是 vault helper + skill installer。verb 走 **spawn `claude -p` 帶 slash command 觸發對應 skill bundle**，由 SKILL.md 內容指揮 agent 流程；schema 是 SKILL.md 唯一交付，binary 不再 inline。

### Architecture

```
codebus binary（cargo workspace）
├─ codebus-core（lib）── vault primitives / wiki lint / config / schema / verb library
├─ codebus-cli（bin codebus）── 7 verb subcommands（init / goal / query / lint / fix / chat / quiz）
└─ codebus-app（bin codebus-app）── Tauri desktop tutorial app（v1 進行中）

CLI 行為分兩類：
  Direct（binary 自跑、deterministic）：
    - codebus init / no-arg → init at pwd
    - codebus lint [--json]
    - codebus quiz validate <file>
  Spawn（binary fork claude -p、slash command 觸發 skill bundle）：
    - codebus goal "..."  → spawn `claude -p "/codebus-goal \"...\""`
    - codebus query "..." → spawn `claude -p "/codebus-query \"...\""`
    - codebus fix         → spawn `claude -p "/codebus-fix"`
    - codebus chat        → multi-turn REPL，每 turn spawn `claude -p` 帶 transcript
    - codebus quiz "..."  → two-shot（plan → generate），每 shot spawn `claude -p`

  cwd = <repo>/.codebus；spawn 同時下 triple-flag（v2 iter-9 lesson）：
    --tools <whitelist>          # hard gate
    --allowedTools <same>        # auto-approval
    --permission-mode acceptEdits # -p mode 無 terminal 必需

Provider 模組：codebus-core/src/agent/claude_cli.rs，single impl，不寫 trait。
trait surface 等到 codex / gemini 等 second impl 真進來再開 change。

每個 vault：
  <repo>/.codebus/CLAUDE.md   ── per-repo schema（user 可改）
  <repo>/.codebus/wiki/       ── 5-folder taxonomy
  <repo>/.codebus/raw/code/   ── PII-filtered source mirror
  <repo>/.codebus/quiz/       ── 生成的 quiz md + per-attempt sidecar progress
  <repo>/.codebus/log/        ── runs-<date>.jsonl + events-<run>.jsonl
  <repo>/.gitignore           ── auto append .codebus/
```

### Lint = CLI（不是 skill）

Lint 邏輯純 deterministic（7 條 rule pattern match）。CI / pre-commit 可直接用、fix skill 透過 Bash tool 跑 `codebus lint --json` 拿 findings，不需 skill-call-skill。

## 3. Anti-Patterns（一次都別再犯）

1. **Spec 不寫 single-impl 抽象**：trait / API surface / enum variant 沒有 2+ impl 或 consumer 驗證的，寫進 design.md open questions 就好；不寫 normative Requirements
2. **Schema 不雙投遞**：每份 schema 內容**只有一個 source of truth**。SKILL.md 跟 inline prompt 不可同份內容。path D 下 SKILL.md 是唯一交付，binary 不再 inline schema
3. **Carry over v2 之前先 grep v2**：每個 change 動工前讀 v2 對應 module / spec，不靠記憶猜行為
4. **`/spectra-apply` 不亂 checkpoint**：一路跑完 tasks.md，除非設計 / 環境真卡

## 4. Stages

v3 走到今天可分四個階段。Stage 1 是 ship 主線；Stage 2 / 3 是 CLI 側補基建；Stage 4 是 app 進場 + 語意層深化（兩條交織）。

### Stage 1 — CLI v3.0.0 ship（2026-05-08 → 2026-05-10，13 條 ✅）

10 條主序列 + 3 條 follow-up，於 2026-05-10 `chore(release): v3.0.0`（commit `6936902`）收口。

| Status | Change | 內容 |
|---|---|---|
| ✅ | `v3-workspace` | 3-crate workspace + 5-verb skeleton |
| ✅ | `v3-init` | vault layout / raw_sync / Obsidian register / 3 skill 骨架 |
| ✅ | `v3-pii` | `PiiScanner` trait + `RegexBasicScanner`（AWS / Anthropic key / email / IPv4） |
| ✅ | `v3-vault-history` | nested git auto-init + auto_commit API |
| ✅ | `v3-goal` | `codebus goal` spawn `claude -p` + triple-flag sandbox + source-signal drift |
| ✅ | `v3-query` | read-only spawn 同模式（無 auto_commit） |
| ✅ | `v3-lint` | 7 rules + human / JSON 雙輸出 |
| ✅ | `v3-fix-trust-agent` | spawn fix loop + Bash 接 `codebus lint --json` |
| ✅ | `v3-config` | 6 條 tolerance + lint section + pii section |
| ✅ | `v3-render-polish` | OSC 8 wikilinks + 5-level emoji priority + NO_COLOR |
| ✅ | `v3-run-log` | `Stdio::piped()` + parse `stream-json --verbose` + jsonl 持久化 |
| ✅ | `v3-bug-fixes` | init→goal 不該 re-sync / lint --repo vault-root error 提示 |
| ✅ | `v3-pii-severity-dispatch` | Critical 強制 mask、Warn 走 user-config（uv 驗收 672 false-positive 後得） |

UV repo 完整 e2e 驗證寫在 [`docs/v3-uv-verification-2026-05-10.md`](v3-uv-verification-2026-05-10.md)。

### Stage 2 — CLI 側基建補完（2026-05-11 → 2026-05-14，7 條 ✅）

進 app 之前先把 CLI core 改成「app 可重用」型態：verb 邏輯從 CLI thin wrapper 搬進 `codebus_core::verb::*`，stream rendering 跟 invoke 解綁、加 cancel signal、events 持久化。同期補上 endpoint config / SKILL.md 純化等周邊整頓。

| Status | Change | 內容 |
|---|---|---|
| ✅ | `claude-code-endpoint-profiles` | `~/.codebus/config.yaml` 加 endpoint profiles（base_url / model / effort 三組合） |
| ✅ | `fail-loud-on-config-parse-error` | config parse 錯誤從 silent fallback 改 fail-loud |
| ✅ | `v3-goal-library` | 3 spawn verb orchestration 搬進 `codebus_core::verb::*`；invoke 加 `on_event` callback + `AtomicBool` cancel；CLI 變 thin wrapper byte-equivalent |
| ✅ | `v3-run-log-events` | RunLog 加 `outcome`；per-run events.jsonl 持久化；GUI runs 強制寫 |
| ✅ | `v3-chat-verb` | 新 verb `codebus chat`（multi-turn read-only REPL）+ `chat::run_chat_turn` + `codebus-chat/SKILL.md` + `/goal` in-REPL promote |
| ✅ | `endpoint-effort-dropdown` | settings UI 後端的 effort 列表來自 spec |
| ✅ | `v3-skill-bundles-vault-only` | SKILL.md 僅寫進 vault 自己 `.claude/skills/`，不污染 `~/.claude/skills/` |

### Stage 3 — codebus-app v1（2026-05-11 → 進行中，10 條，9 ✅ / 1 ⏳）

CLI 主線 ship 後 app 層 v1 切成主序列 8 條（foundation + A + B + chat + C + D + E + F），每一條都假設前一條已 archive；不是平行可換序。一路上加 2 條額外補洞（fix-app-quiz / fix-quiz-ux-wiring）、1 條進度持久化 redesign（quiz-attempt-progress）、1 條 settings 前端（settings-config-frontend）。

| Status | Change | 內容 |
|---|---|---|
| ✅ | `v3-app-foundation` | Tauri shell + IPC bridge（5 commands） + Lobby + Settings modal stub + Workspace stub + Tailwind v4 / shadcn token |
| ✅ | `stage-b-app-endpoint-settings` | foundation follow-up — settings 把 endpoint 三組合接通 |
| ✅ | `v3-init-nav-stubs` | Sidebar Goals / Wiki / Quiz tabs stub |
| ✅ | `v3-app-workspace-goal` | Vault Workspace 真內容：sidebar tabs + Wiki preview (Milkdown) + Goal flow（modal / mini-stream / running / done / cancelled / interrupted + `[Retry with same goal]`） |
| ✅ | `v3-app-chat-cmdk` | Cmd+K spotlight chat 抽屜（multi-turn streaming + 引用 + `[Promote to goal]`） |
| ✅ | `v3-app-quiz` | Quiz flow（pending / reviewing 兩態 + md 持久化）+ wiki page 觸發 quiz / 答題評分 / frontmatter |
| ✅ | `fix-app-quiz` | v3-app-quiz 7 個 GUI defect TDD 修正（header 碰撞 / +New 無反應 / plan-marker 脆 / preamble 漏檔 / live render / view-log modal / quiz 內 +New 隱藏） |
| ✅ | `quiz-attempt-progress` | Quiz 進度持久化 redesign：不可變 attempt md + sibling `<id>.progress.json` sidecar；history 徽章 / 路由；completed → QuizReview 取代 raw md；重做此份 |
| ✅ | `fix-quiz-ux-wiring` | 5 項既有缺口（答題/summary 返回鈕 / 已 active Quiz tab 點回 history / 啟動載入 config / 出題數接 `quiz.default_length` clamp / plan-marker 行內前言容忍） |
| ✅ | `settings-config-frontend` | Settings 把 pii / lint / quiz / goal / log 各 config knob 接到 UI |
| ⏳ | `v3-app-polish-ship` | Release build / installer / auto-update / icon / E2E test infra / 跨平台 macOS+Linux 驗收 sweep。**2026-05-20 user 明確 deprioritize**：solo dev、無外部 user 階段 release-gate 衛生工作回報低，本條優先序排在 `v3-multi-agentic-provider` 跟其他 feature backlog 之後；待具體 release-blocking 訊號出現（要對外發、要給別人試、deadline）再動 |

### Stage 4 — 語意層深化（2026-05-19 → 進行中，3 條 ✅）

Goal / quiz 的 trust-agent 模式 ship 後實機看到「agent 自己 validate 自己」的盲區：plan 不符規範、generate 偏離 plan、寫出的 wiki 本身 hallucination。三條獨立 model 驗證 + bounded repair 補上這層。

| Status | Change | 內容 |
|---|---|---|
| ✅ | `quiz-validate-repair` | Deterministic quiz validation + trust-agent self-repair loop |
| ✅ | `quiz-content-verify` | Independent model content verify + bounded repair（CLI + GUI） |
| ✅ | `goal-content-verify` | Independent model content verify + bounded repair（goal + shared core） |

## 5. Cross-platform policy

開發階段一律以 **Windows MSVC** 為主，每條 change 的 acceptance checklist 只在 Windows 上必跑必過。macOS / Linux 的手動回歸驗證集中到 `v3-app-polish-ship` 一次掃完，作為 release gate 的一部分。

理由：
1. 主要開發機是 Windows，每條 change 都要求三平台驗證 dev velocity 損失過大
2. 跨平台 build artifact / installer 本來就排在 polish-ship，順手把手動驗收一起做才不會驗兩次
3. polish-ship 才會建 E2E test infra，到時候 cross-platform 也可能變部分自動

各 change 的 tasks.md 不另列 macOS / Linux acceptance 條目；polish-ship 屆時負責統整。

### Deferred acceptance registry

各 change 在此登記延後到 `v3-app-polish-ship` 的 macOS / Linux 手動驗收範圍：

- **`v3-app-quiz`** — polish-ship 需在 macOS + Linux 重跑五區塊：(1) CLI `codebus quiz "<topic>"` 端到端（plan→generate→落檔 `<vault>/.codebus/quiz/<slug>/<id>.md`；no-match exit 0 不落檔；retry 非破壞兩檔）；(2) GUI Quiz tab plan-confirm-generate flow；(3) wiki preview `[Quiz me on this]` Page flow；(4) Quiz history（掃 `.codebus/quiz/` 依 slug group、retry 兩 row、`[看過程]` events.jsonl）；(5) 共用 `quiz.default_length` config 與 `app.*` namespace isolation。Windows MSVC 上述皆已必跑必過。
- **`fix-app-quiz`** — macOS / Linux 手動驗收仍 deferred，沿用上述 v3-app-quiz 五區塊範圍含本 change 的修正。Windows 已由 user 實機 sweep pass（含 quiz-attempt-progress + fix-quiz-ux-wiring 合併驗收 2026-05-19）。
- **`quiz-attempt-progress`** — macOS / Linux deferred。sidecar atomic write 的 `fs::rename` 覆寫語意在 Windows 已測試覆蓋，macOS/Linux 需於 polish-ship 一併實機確認。Windows GUI sweep 2026-05-19 全 pass（答題中途離開非破壞 / history 接續未答題 / QuizReview 取代 raw md / 解釋 wikilink 跳 wiki / 看過程 modal / 重做此份不 spawn）。
- **`fix-quiz-ux-wiring`** — macOS / Linux deferred。Windows GUI sweep 2026-05-19 Journey A–D 全 pass（D4 出題數 / D1 返回非破壞 / D2 已 active tab 點回 history / D3 啟動載入 config 即生效 / D5 plan-marker 行內前言容忍 + no-match 不落檔）。

## 6. Out of scope（v3 範圍以外）

下列 item 在 v3 主線**皆不做**，未來走獨立 change 評估：

- 多 AI provider 選擇 UI（Claude CLI 是唯一選項，trait surface 等 second-impl 真進來再開）
- Light theme / theme toggle（hard-coded dark）
- Language switcher UI（auto-detect system locale）
- Per-vault settings override
- Quest banner / progress bar / "graduated" / "mastered" / "learned" 任何 page-level state
- Tutorial slideshow / 投影片模式 / 教學 md 生成
- Telemetry / analytics / crash reporting
- Quiz 歷史圖表 / 間隔重複（spaced repetition）
- 多 goal 並行（v1 always at most 1 running goal）
- 分享 / 匯出 / public wiki publish

## 7. Tauri 之後可能做的（v3.x → v4 idea）

| Status | Idea | 觸發點 |
|---|---|---|
| 💭 | `v3-fix-path-inject` | init 自動注 PATH 進 `.claude/settings.json`，免 user 手動 `cargo install`。等真有 user 抱怨 fix 跑不起來再開 |
| 🟢 | `v3-multi-agentic-provider` | **2026-05-20 unblocked** — codex CLI 0.132.0 spike 確認 contract 完整，second-impl 對標條件滿足。**Codex 對映**：`codex exec` ≈ `claude -p`、`--json` ≈ `stream-json`、`--sandbox read-only/workspace-write/danger-full-access` 比 Claude `acceptEdits` 更乾淨、`~/.codex/skills/<name>/SKILL.md` **完全相同 yaml frontmatter + md 格式** 跟 Claude 共用、`resume`/`fork` ≈ `--resume`、有 `.rules` execpolicy + hook system、`--output-schema` 額外加分、token usage 含 `reasoning_output_tokens` 直接對應 codebus 既有 `TokenUsage.reasoning_tokens`。**同日 `agy` 1.0.0 spike 結論不適合**（缺 `--tools` / 無 stream-json / `-p` mode 看不到 agentic tool loop / 帶自己預設 system prompt 打架）。**工程量重估**：約 1-2 週 — 加 `CodexBackend` + `parse_codex_stream_line` + skill bundle 雙寫（`.codebus/.claude/skills/` + `.codebus/.codex/skills/`）+ config schema 加 codex profile + `agent::invoke` routing。校準：之前說「codebus 在 v3-run-log-events 已 normalized、卡的是對方缺 contract」是對的，agy spike 印證；codex spike 找到合格 second-impl，整合 viable。詳見 [`docs/2026-05-14-multi-provider-agent-backend-backlog.md`](2026-05-14-multi-provider-agent-backend-backlog.md) 2026-05-20 更新段 |
| 💭 | `v3-multi-pii-provider` | 補強現有 regex_basic：Microsoft Presidio HTTP / AWS Comprehend Detect-PII / 自訂 ML scanner |
| 💭 | `v3-embedded-search` | 對 wiki pages 跑 embedding / vector index 提供 semantic search（補強 `query` 或開新 `codebus search` verb） |
| 💭 | `v3-first-run-wizard` | 第一次跑偵測 `~/.codebus/config.yaml` 不存在 → 互動引導選 AI / PII / log 設定。依賴上面四條都有實際選項可選才有意義 |

## 8. Open Questions（每個 change 各自 design.md 處理）

- **#2**：per-repo `.codebus/CLAUDE.md` 寫的時候要不要 `if missing`（v2 phase 1 task 11.1）保 user 客製化？答：應該（已實作）。
- **#7**：fix loop 的 max_iterations 是 hardcoded 5 還是必須走 config？答：hardcoded 5 + config 覆蓋（#8 接通）。
- **#9**：OSC 8 hyperlink 在 `codebus lint` JSON output 模式要不要做？答：JSON 模式不要（machine-readable）；human 模式才做。
