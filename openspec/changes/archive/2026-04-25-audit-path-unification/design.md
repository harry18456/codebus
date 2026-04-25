## Context

`docs/reviews/2026-04-25-stage-4.md` Cat 2.5-B 決策階段已論證 (a) 統一到 `<ws>/.codebus/` 是勝出方案。本 change 是執行階段，純 mechanical path move + spec 對齊 + test fixture 更新，零 architecture 重設、零 LLM 行為改動。

開動時機選 Module 5 Generator P0 動工**之前**：避免 Generator 透過 `app.state.llm_chat_provider(ws)` factory 寫到舊 path 後再次回頭改的 ordering issue。

## Goals / Non-Goals

**Goals**

- 把 3 個 jsonl writer（`token_usage` / `llm_calls` / `reasoning_log`）從 `<ws>/` 根目錄移到 `<ws>/.codebus/` 子目錄
- 與既有 `sanitize_audit` / `tool_audit` 路徑慣例對齊，五層 workspace audit 統一在同一 subdir
- 對應 spec MODIFIED Requirements + test fixture assertion 更新
- CLAUDE.md 七層 Audit JSONL 段把位址描述改正、拿掉 latent risk paragraph
- Review tracker Cat 2.5-B 標 ✅
- 走完整 Spectra ceremony（new → in-progress → 修 + 測 → validate → archive）

**Non-Goals**（與 proposal Non-Goals 一致）

見 proposal Non-Goals 段：不動 sanitize_audit / tool_audit 路徑、不動 kb_growth、不動 authorization_audit、不引入 migration script、不重 architect logger constructor、不動 audit JSONL schema、不動 fixture baseline、不動 archive folder。

## Decisions

### Decision 1：補三個 filename 常數，杜絕 magic string

api/__init__.py 已有 `_WORKSPACE_AUDIT_SUBDIR = ".codebus"` + `_SANITIZE_AUDIT_FILENAME = "sanitize_audit.jsonl"`。本 change 補 sibling `_TOKEN_USAGE_FILENAME` / `_LLM_CALLS_FILENAME` / `_REASONING_LOG_FILENAME` 三常數。

理由：
- **避免 magic string 散播**：3 個 factory + 1 個 explore 共 6 處 string literal，集中常數一處改全
- **符合既有 convention**：M1 `_SANITIZE_AUDIT_FILENAME` 已是這個 pattern
- **未來 audit 移檔位再次發生時零摩擦**：第七層 `authorization_audit.jsonl` 上線時也用同一條 `_AUTHORIZATION_AUDIT_FILENAME` 即可（雖然那是 App-level，但常數模式相通）

**替代方案**：保留 6 處 string literal。棄用 — Cat 3 latent risk #1 `rules_version` 三點 hard-code 已是同類技術債，本 change 順便不滋生新的。

### Decision 2：ReasoningLogger 維持 caller-side mkdir，不改 constructor

`agent-core` spec L156 現規定 ReasoningLogger「MUST NOT silently create parent directories outside the workspace」。改 constructor 加 mkdir 會：
- 與 spec 衝突，需同步動 spec scenario
- 違反 single responsibility（path safety 是 caller 責任）
- 與 sibling logger `UsageTracker` / `LLMCallLogger` 不一致 — 但那兩者本來就 auto-mkdir，是另一個 inconsistency；本 change 不擴大解決

因此選 **caller-side mkdir**：在 `api/explore.py` 構造 ReasoningLogger 前加 `(workspace_root / _WORKSPACE_AUDIT_SUBDIR).mkdir(parents=True, exist_ok=True)`。

理由：
- **最小變動**：1 行 mkdir + 1 行 import 常數，spec 不需動 ReasoningLogger constructor 行為
- **保 path safety 邏輯在 caller 一處**：未來如果 Q&A Agent 也要用 ReasoningLogger，同樣要在自己 caller 加 mkdir，spec 規矩明確
- **與 spec 既有「caller MUST have rejected the path via ensure_in_workspace」對齊**：caller 已負責路徑安全 + parent 存在

**替代方案 A**：給 ReasoningLogger constructor 加 auto-mkdir。棄用 — 違反 spec L156，需動 spec scenario。
**替代方案 B**：在 ReasoningLogger 加 `ensure_parent: bool = False` opt-in flag。棄用 — flag 設計增加 surface 而本 change 只有一個 caller，過度抽象。

### Decision 3：Spec 改 path string 但 scenario 維持 `<workspace>` placeholder semantic

usage-tracking spec L11 寫 `<workspace>/token_usage.jsonl`；agent-core spec L137 寫 `{workspace_root}/reasoning_log.jsonl`。本 change 把所有 path 字面改為 `<workspace>/.codebus/<filename>` / `{workspace_root}/.codebus/<filename>`，但保留 placeholder 寫法（不寫死絕對 path）。

理由：
- **placeholder 寫法是設計約定**：spec 約束的是「相對於 workspace_root」的 path 結構，不是某個具體 workspace
- **保留既有 scenario 風格**：「One line per chat call」這類 scenario 主要 assert 行數而非 path，path 只是 setup context

### Decision 4：Test 動最小範圍 — 只動 factory-output assertion

scout 顯示 36 個 test 檔提及 `token_usage.jsonl` / `llm_calls.jsonl` / `reasoning_log.jsonl`，但其中 ~29 個是直接 construct 的（`UsageTracker(tmp_path / "token_usage.jsonl")`），這類 test 的 path 是 test 自選 arbitrary 值，與 factory 路徑慣例無關，**不動**。

只動 factory-output assertion 的 test（預估 5-7 檔），這類 test 用 `wire_kb_dependencies` / `app.state.llm_*_provider(ws)` 然後 assert `(ws / "<file>.jsonl").exists()` —— 必須改為 `(ws / ".codebus" / "<file>.jsonl").exists()`。

理由：
- **變動範圍最小化**：直接 construct 的 test 沒理由跟著動，path 只是 fixture detail
- **避免 noise diff**：scout 假設「36 檔都要改」會炸 PR review；實際 7 檔內聚焦
- **保持測試獨立性**：test 各自決定它自己的 path convention，本 change 不強推

### Decision 5：CLAUDE.md 七層 Audit JSONL 段同步改完整 + 拿掉 latent risk paragraph

Cat 2.5-A commit `06744bb` 在 CLAUDE.md L102-117 加了七層 implementation status 表 + 「Audit 路徑不一致是已知 latent risk」尾段。本 change 落地後 latent risk 解掉，要把那段 paragraph 拿掉、把 6 個 ✅ 層的位址都改成 `<ws>/.codebus/`。

理由：
- **CLAUDE.md 是 onboarding source of truth**：解掉的 risk 不能繼續寫在「已知 risk」段
- **位址改完整就一致**：六層 workspace audit 全在 `<ws>/.codebus/`，描述零歧義

## Risks / Trade-offs

- **[既有 dev workspace 有舊 path audit 殘留]** → 風險低；codebus 目前沒外部使用者，dev 環境的 workspace 都是 tmp_path（每次 fresh）或 `tests/golden/*-synthetic/` fixture（也是隔離 workspace，不寫 audit）。**Mitigation**：本 change 不引入 migration，但 CLAUDE.md 提示 dev 若 hit 殘留 audit 可手動刪除舊 path 檔。
- **[ReasoningLogger caller-side mkdir 漏加]** → 風險中；只有一個 caller (`api/explore.py:178`)，但若未來 Q&A Agent 接 ReasoningLogger，新 caller 也要自己加 mkdir。**Mitigation**：spec MODIFIED Requirement 主文 explicit 寫「caller MUST mkdir parent」。
- **[Test fixture path 漏改造成 false negative]** → 風險中；factory-output assertion 散在 5-7 檔，漏改會 silent pass（assertion 通過但檢查的是不存在的舊 path）。**Mitigation**：apply 階段 grep `(ws.*"\.codebus"\s*/\s*"token_usage|llm_calls|reasoning_log"` 確認新 path assertion 全到位；額外 cross-check `(ws_path / "token_usage.jsonl").exists()` pattern 不再存在於 factory test 檔。
- **[Magic string 常數命名衝突]** → 風險低；新增 `_TOKEN_USAGE_FILENAME` 等三常數都是 `_PRIVATE_UPPER` 模式，與既有 `_WORKSPACE_AUDIT_SUBDIR` 一致，無 namespace 衝突。
- **[使用者 `.gitignore` 沒設 `.codebus/`]** → 風險低；但本 change 不負責教使用者 git 配置。CLAUDE.md 可順便加一句 `.codebus/ → user 的 .gitignore 一行解決`。

## Migration Plan

- **Schema migration**：無 — audit JSONL 欄位完全不變
- **既有 workspace data**：無 — codebus 沒外部使用者，內部 dev 環境每次 tmp_path
- **Test fixture migration**：無 — fixture 用 tmp_path，本 change archive 後跑全測 fresh dirs
- **Spec migration**：本 change MODIFIED Requirements，main spec auto-sync 由 `spectra archive`
- **Roll-back path**：git revert 兩個 commit（work + archive），spec / code / test 一併還原

## Open Questions

無。Cat 2.5-B 決策階段已涵蓋全部設計選擇；本 change 是純執行。
