## Summary

把 `SpawnSpec.prompt: String`（verb 層 pre-compose 的 `/codebus-<verb> ...` 字串）拆成結構化 `verb + sub_mode + input`，由各 backend（claude_backend / codex_backend）自行 assembly：claude 組 `/codebus-<verb> ...`、codex 組 `$codebus-<verb> ...` native skill trigger 形式。

## Motivation

prompt surface deep review §17 Pattern 10 + §16 F26 + §15 F86 三個 finding 同根：`SpawnSpec.prompt: String` 是 provider-neutral 預組字串，verb 層 9 處 compose 點全寫 claude 形式 `/codebus-<verb>` 字面。**對 codex 是繞路**：

- **§16 F26 實機驗證**（2026-05-23 codex-cli 0.133.0）：codex 用 `$codebus-<verb>` native trigger 比收到 `/codebus-<verb>` 字面繞 description-match implicit invocation **省 24.8% input tokens**（11k tokens × 每次 spawn）、agent_message 少 1 條（無探索繞路）、probe 立即出現（SKILL 指示嚴格遵從）
- **§15 F86 (🔴 CRITICAL)**：10 個 spawn 模板全用 `/` prefix 是 cross-cutting footprint，codex 應用 `$`
- **§16 F96**：goal `verify:` / `repair:`、quiz `plan:` / `generate:` / `validate:` mode prefix 形式不統一，缺結構化欄位

`agent-backend/spec.md:64` 既有 "SpawnSpec Provider-Neutral Intent" requirement 已寫 SpawnSpec SHALL contain `input` (user text) — **spec 既有 wording 已是 Phase 3 目標**；code (`spawn_spec.rs:79`) 仍叫 `prompt: String` 是 code drifted from spec。Phase 3 = code 對齊 spec + 補上 spec 未列的 `sub_mode` 欄位。

Phase 2 已撤回 SKILL body byte-identical invariant。Phase 3 撤回 spawn 字串 byte-identical invariant，補完 PE2 B split 全層落地。

## Proposed Solution

### SpawnSpec shape 改動

```rust
pub struct SpawnSpec {
    pub verb: Verb,                       // SKILL bundle name: Goal/Query/Fix/Chat/Quiz (NOT Verify)
    pub resolve_as: Option<Verb>,         // NEW: model-resolution override (None = use verb;
                                          //      Some(Verb::Verify) for cross-flow content-verify spawns
                                          //      that invoke the quiz/goal bundle but use Verify config)
    pub sub_mode: Option<String>,         // NEW: e.g. Some("verify"), Some("plan"); None for free-text
    pub input: String,                    // RENAMED from `prompt`: raw user text or structured body
    pub permission: Permission,
    pub command_allowance: Option<CommandPrefix>,
    pub resume_session_id: Option<String>,
}
```

**Why `resolve_as` exists**：apply 階段 grep `Verb::Verify` 揭露 `verb` 在現況有雙重身份 — 5/6 spawns 中 `verb` 同時是 SKILL bundle 名 + config 解析 key；但 goal verify spawn 跟 quiz content-verify spawn 用 `spec_verb = Verb::Verify`（verify-stage-independent-model pattern），slash 卻是 `/codebus-goal verify:` 或 `/codebus-quiz verify:`。Phase 3 backend 要組 `/codebus-<bundle> ...`，bundle 必須是 Goal/Quiz 不是 Verify。`resolve_as` 把 model-resolution override 顯式化，避免 hidden coupling（如「sub_mode=verify → 自動用 Verify config」）。

### Backend assembly

**claude_backend**（quoted free-text 保留現況慣例）：
```rust
let prompt = match &spec.sub_mode {
    Some(mode) => format!("/codebus-{verb} {mode}: {input}"),
    None => format!("/codebus-{verb} \"{input}\""),
};
cmd.arg("-p").arg(prompt);
```

**codex_backend**（native `$` trigger，無 quotes — F95 撤回確認 modern LLM 容錯）：
```rust
let prompt = match &spec.sub_mode {
    Some(mode) => format!("$codebus-{verb} {mode}: {input}"),
    None => format!("$codebus-{verb} {input}"),
};
cmd.arg(prompt);
```

### 9 compose 點 refactor — 停止 pre-compose、改傳 sub_mode + input

| Compose 點 | 現況 | 改後 SpawnSpec 欄位 |
|---|---|---|
| `verb/chat.rs:161` | `format!("/codebus-chat \"{}\"", text)` | sub_mode=None, input=text |
| `verb/goal.rs:327` (ingest) | `format!("/codebus-goal \"{}\"", text)` | sub_mode=None, input=text |
| `verb/goal.rs:459` (verify) | `format!("/codebus-goal verify: goal={}\\n\\nCHANGED PAGES:\\n{}", ...)` | sub_mode=Some("verify"), input=結構化 body |
| `verb/goal.rs:505` (repair) | `format!("/codebus-goal repair: goal={}\\n\\nCONTENT DEFECTS:...\\n\\nFLAGGED PAGES:...", ...)` | sub_mode=Some("repair"), input=結構化 body |
| `verb/query.rs:119` | `format!("/codebus-query \"{}\"", text)` | sub_mode=None, input=text |
| `verb/quiz.rs:426` (plan) | `format!("/codebus-quiz plan: {}", topic)` | sub_mode=Some("plan"), input=topic |
| `verb/quiz.rs:526` (generate initial) | `format!("/codebus-quiz generate: pages=[...] count=N", ...)` | sub_mode=Some("generate"), input=結構化 body |
| `verb/quiz.rs:587` (verify) | `format!("/codebus-quiz verify: topic={}\\n\\nQUIZ:\\n{}", ...)` | sub_mode=Some("verify"), input=多行 body |
| `verb/quiz.rs:626` (generate retry) | `format!("/codebus-quiz generate: pages=[...] count=N\\n\\n...PREVIOUS QUIZ:...\\nCONTENT DEFECTS:...", ...)` | sub_mode=Some("generate"), input=結構化 body 含 retry context |

### 6 production SpawnSpec 構造點調整

verb/chat.rs:189 / verb/goal.rs:332 / verb/goal.rs:651 / verb/query.rs:153 / verb/quiz.rs:366 / wiki/fix/mod.rs:138 — 每個改成傳 sub_mode + input 而非預組 prompt。fix verb 無 user input → sub_mode=None, input=`""`。

### 5 test SpawnSpec fixture 更新

agent/claude_backend.rs:125 (test helper) / agent/claude_cli.rs:465 (test_spec) / agent/codex_backend.rs:232 (test helper) / agent/spawn_spec.rs:105 (單元 test) / wiki/fix/mod.rs:207 (單元 test) — signature 跟著動。

### spawn_spec.rs 模組 doc 重寫（~30 行）

現況 line 12-25 + Phase 0 加的 TODO 段宣稱「SKILL bundle is double-written identically for every provider, so the same invocation string is meaningful to all of them」+ TODO 預告 Phase 2/3 撤回。Phase 3 落地後重寫成「SpawnSpec 是 provider-neutral structured intent；assembly per provider 在 backend；claude 組 slash form、codex 組 dollar form」現況描述。

## Non-Goals (optional)

- **Phase 5 codex per-command allowance spike**（F73 上半）：codex sandbox 缺 read-only-plus-one-cmd 中間態仍未解；本 change 不動 sandbox 行為
- **`Provider` enum promote 到 `crate::agent`**：Phase 2 留 `pub(crate) skill_bundle::Provider`；本 change 不 promote、backends 自己用 enum discriminant 即可（claude_backend = claude form, codex_backend = codex form, no enum check needed）
- **`SubMode` 型別改 verb-specific enum**：`sub_mode: Option<String>` 用 String 不用 enum——每 verb 自己有 sub-mode 子集（chat=none, goal=verify/repair, quiz=plan/generate/verify），enum 要 verb-specific 子型別或 nested enum 都過度抽象（per [[feedback_dont_speculative_abstract]]）
- **改 free-text 輸入 escape 規則**：claude `\"...\"` quoting 維持現況、codex 無 quotes 維持 F95 撤回的「modern LLM 容錯」結論；不引入新 escape layer
- **改 stream parser 行為**：spawn 字串改動只影響 outbound prompt；inbound stream event parsing 不變

## Alternatives Considered (optional)

- **保留 `prompt: String` 不重命名**：Phase 3 不動 SpawnSpec，只在 backend 內 detect `/codebus-<verb>` prefix 改寫成 `$codebus-<verb>`。否決：spec 已寫 `input`、code drift 該收；string-based detection 比 structured 欄位脆弱
- **`SubMode` 用 enum**（如 `enum QuizMode { Plan, Generate, Verify }` / `enum GoalMode { Verify, Repair }`）：型別安全但要 verb-specific 子 enum + 對應 dispatch。否決：3 個 verb sub-mode 集合不同，enum 抽象成本 > 收益；String 配合 verb context 已足夠精準
- **兩 provider 都 quote**：claude 既有 `\"...\"`、codex 也加。否決：codex 無慣例需要 quote、F95 實機驗證 modern LLM 容錯；額外 quote 對 codex 是噪音
- **兩 provider 都不 quote**：claude 既有 `\"...\"` 改不 quote。否決：claude 既有測試 + 既有行為穩定，無需改動；F95 撤回不等於現有 quote 是錯的、只是不必新加

## Impact

- Affected specs: `agent-backend`（MODIFIED Requirement "SpawnSpec Provider-Neutral Intent"：加 `sub_mode` 欄位、明示 backend assembly 責任 + provider-specific trigger form）
- Affected code:
  - Modified:
    - `codebus-core/src/agent/spawn_spec.rs`（struct shape 改、module doc 重寫 ~30 行、單元測試 update）
    - `codebus-core/src/agent/claude_backend.rs`（assembly logic：sub_mode + input → `/codebus-<verb>` form；test helper 跟著動）
    - `codebus-core/src/agent/codex_backend.rs`（assembly logic：sub_mode + input → `$codebus-<verb>` form；test helper 跟著動）
    - `codebus-core/src/agent/claude_cli.rs`（test_spec fixture signature update）
    - `codebus-core/src/verb/chat.rs`（chat spawn 停 pre-compose）
    - `codebus-core/src/verb/goal.rs`（ingest / verify / repair 三 compose 點 → sub_mode + input）
    - `codebus-core/src/verb/query.rs`（query spawn 停 pre-compose）
    - `codebus-core/src/verb/quiz.rs`（plan / generate-initial / verify / generate-retry 四 compose 點 → sub_mode + input）
    - `codebus-core/src/wiki/fix/mod.rs`（fix spawn 停 pre-compose，無 input → input=`""`；單元測試 update）
  - New:（無新檔）
  - Removed:（無）
- Tests:
  - 既有 `cargo test -p codebus-core` 全綠（含 spawn_spec / claude_backend / codex_backend / verb-level integration test）
  - 新增 backend assembly 單元測試：claude_backend 帶 sub_mode + input → output 含 `/codebus-<verb> <mode>: <input>` 字面；codex_backend 同 input → output 含 `$codebus-<verb> <mode>: <input>` 字面；free-text 路徑（sub_mode=None）claude 有 `\"...\"` quotes、codex 無 quotes
- 實機驗證：cargo build CLI、對 vault spawn claude 跟 codex 看 prompt 字串實際形式（claude `-p "/codebus-goal \"text\""`、codex `cmd.arg("$codebus-goal text")`）
