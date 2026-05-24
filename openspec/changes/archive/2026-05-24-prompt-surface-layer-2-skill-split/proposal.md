## Summary

把 SKILL bundle materialization 從現況「single body byte-identical between claude/codex paths」改成 provider-aware split — `.claude/skills/<verb>/SKILL.md` 與 `.codex/skills/<verb>/SKILL.md` 各自寫該 provider 準確的機制描述、觸發語法、shell 形式。

## Motivation

prompt surface deep review §17 浮 4 條 cross-cutting pattern 集中於 Layer 2 SKILL body，**根因相同**：`stub_content(verb: &str)` 不知道 provider，bytes 對 claude/codex 都一樣。實機證實的代價：

- **Pattern 3（8 finding）Claude 機制描述失準** — SKILL body 多處寫死 `PreToolUse hook` / `--tools Read,Glob,Grep` / `Read hook` / `mcp_*` family / `CLAUDE.md` 檔名。codex agent 載入這份 SKILL 看到對它無效的描述（F19/F67 實機證實 codex 找不到 `CLAUDE.md`，檔名是 `AGENTS.md`）
- **Pattern 9（F73 下半）Bash heredoc 跨 OS 不通用** — quiz Mode B 寫 bash heredoc，codex on Windows 走 PowerShell 失敗
- **Pattern 11（F21/F68/F80，3 verb）Trigger 框架對 codex 失準** — SKILL 寫「user types `/codebus-<verb>`」對 codex native 是 `$codebus-<verb>`
- **Pattern 1 跨層 Layer 2 部分（F32 + F45）** — goal/query SKILL workflow 內重複列 taxonomy enum，與 `neutral.md` §2 drift 風險

PE2 方案 B（拆 claude/codex SKILL 兩份）已在 prompt-surface-layer-1-batch 之前的 review 鎖定為核心決策。本 change 是 PE2 B 的 Layer 2 落地。

## Proposed Solution

引入 `pub(crate) enum Provider { Claude, Codex }` scoped 於 `codebus-core/src/skill_bundle/`，改寫 `stub_content(verb)` 為 `stub_content(verb, provider)`，在 10-12 leak 點用 inline `match provider` 切換 provider-specific 文字。`.claude/` materialization 帶 `Provider::Claude`、`.codex/` materialization 帶 `Provider::Codex`。

leak 點分佈：

| Surface | Finding | claude 文字 | codex 文字 |
|---|---|---|---|
| `stub_content` shared head trigger 行 | F21/F68/F80（Pattern 11） | 兩 provider 共用 semantic 句「Activate when the user requests <verb action>」，去掉 syntax 細節 | 同左 |
| `stub_content` shared head Schema rules 行 | F19/F67（Pattern 3） | "Read `CLAUDE.md` here" | "Read `AGENTS.md` here" |
| `FIX_WORKFLOW` Step 1 hook 描述 | F49（Pattern 3） | PreToolUse hook 機制 | sandbox `-s read-only` 規則 |
| `FIX_WORKFLOW` Read-Only Invariant | F40 | `--tools Read,Glob,Grep` | sandbox `-s read-only` |
| `CHAT_SKILL_CONTENT` Read-Only Invariant | F65 | 同上 | 同上 |
| `CHAT_SKILL_CONTENT` `mcp_*` family | F66 | 保留 mcp 排除段 | 整段移除（codex 無 mcp tool naming） |
| `CHAT_SKILL_CONTENT` Schema rules | F67 | "CLAUDE.md" | "AGENTS.md" |
| `QUIZ_SKILL_CONTENT` Read-Only Invariant | F72 | 同 F40 | 同 F40 |
| `QUIZ_SKILL_CONTENT` Schema rules | F79 | "CLAUDE.md" | "AGENTS.md" |
| `QUIZ_SKILL_CONTENT` Mode B self-validate | F73 下半（Pattern 9） | bash heredoc 維持 | 整段 Mode B 改 emit `[CODEBUS_QUIZ_NO_VALIDATE]` warning + skip（best-effort，Mode B per-command allowance 是 Phase 5 spike） |
| `GOAL_WORKFLOW` Step 2 taxonomy | F32（Pattern 1 Layer 2） | 改 "see §2 in cwd CLAUDE.md" 移除 enum 列舉 | 改 "see §2 in cwd AGENTS.md" 同上 |
| `QUERY_WORKFLOW` Step 1 taxonomy | F45（Pattern 1 Layer 2） | 同 F32 | 同 F32 |

測試：既有 11 個 `stub_content_*` test 全部 parameterize 兩 provider（11 → 22 個 test case），加新 provider-specific 斷言（"claude body 含 `Read hook` 字串"、"codex body 不含 `Read hook` / 含 `AGENTS.md`"、"codex quiz body 含 `CODEBUS_QUIZ_NO_VALIDATE` marker"）。

## Non-Goals (optional)

- **Phase 3 SpawnSpec 重構**（`SpawnSpec.prompt: String` → `verb + sub_mode + input`、backend 各自組 `/codebus` vs `$codebus`）：另一條 change。本 change 只動 SKILL body 內容，spawn 字串組裝層維持現況
- **Phase 5 F73 上半 codex per-command allowance spike**：codex sandbox 缺中間態（`read-only` / `workspace-write` / `danger-full-access` 三級），不能像 claude PreToolUse hook 精準放行單行 `codebus quiz validate`。本 change 對 codex Mode B 選擇 best-effort skip + emit warning，把架構解法留 spike
- **`Provider` enum promote 到 `crate::agent`**：Phase 3 backends 用 trait dispatch（`AgentBackend` impl）未必需要這個 enum。`Provider` 先 scoped 於 `skill_bundle` 內 `pub(crate)`；若 Phase 3 真要共用、那時再 promote（純機械改動）
- **SKILL bundle 路徑 layout 改動**：`.claude/skills/codebus-<verb>/SKILL.md` 與 `.codex/skills/codebus-<verb>/SKILL.md` 路徑維持現況，只 body 內容改

## Alternatives Considered (optional)

- **Option B（template engine + var substitution）**：引入 template engine、body 用 `{trigger}` `{read_hook_lang}` 等 placeholder、provider 提供替換值。否決：10-12 leak 點不到需要 template engine 的 threshold；多引入一層 indirection 反讓 grep / review 變難
- **Option D（10 個獨立 .md 檔，每 verb × 每 provider 一個 include_str!）**：拒絕。5 verb body 95% 共享，10 檔重複高、sync drift 風險大；inline match 在 5-15 leak 點仍可讀
- **`Provider` 用 `&str`（"claude" / "codex"）而非 enum**：拒絕。enum 編譯時保證窮舉、新 provider 加入時 compiler 提醒所有 match site
- **F73 codex Mode B 用 cross-shell 形式重寫（temp file 取代 heredoc）**：拒絕。shell 問題雖解，sandbox 仍會擋 `codebus quiz validate` 跑（F73 上半），結果還是 fail。乾脆 best-effort skip 公開行為差

## Impact

- Affected specs: `skill-bundles`（MODIFIED Requirement：byte-identical invariant 撤回；ADDED Requirement：provider-aware materialization）
- Affected code:
  - Modified:
    - `codebus-core/src/skill_bundle/mod.rs`（新增 `Provider` enum、`stub_content` 加 provider 參數、`workflow_section` 同；shared head + 5 verb workflow consts 內 ~10-12 處 inline match；`write_bundle_if_missing` / `write_codex_materialization_if_missing` 帶對應 provider；11 個 `stub_content_*` test 改 parameterized + 新增 provider-specific 斷言）
  - New:（無新檔；`Provider` enum 在既有 `mod.rs` 內）
  - Removed:（無）
- Tests: `codebus-core/tests/schema_neutrality.rs` 4 斷言全 still pass；`codebus-core/tests/vault_init.rs` AGENTS.md materialization 流程不破；新增 22 個 `stub_content_*` test case（11 × 2 provider）+ provider-specific 斷言（claude 含特定 hook 字眼、codex 不含 / 含 AGENTS.md / 含 NO_VALIDATE marker）
