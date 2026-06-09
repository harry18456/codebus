# Backlog: quiz self-validate heredoc 被 Bash metachar denylist 擋

**Date:** 2026-05-28
**Surfaced during:** F4 Bash-hook adversarial re-audit（多 agent workflow）的 denylist→allowlist 評估 agent 意外發現
**Severity:** 正確性 / capability 靜默 regression（非 correctness break）
**Status:** archived（2026-05-29，change `quiz-heredoc-selfvalidate-unblock`）— 採 Option A 變體：結構化放行單引號 heredoc（首行/opaque body/收尾 MARKER/收尾後四段、僅 `<<'MARKER'`、body 不掃），unquoted / chaining / trailing / 非 heredoc redirect 仍擋；84/84 unit + 15/15 真實 binary + live CDP e2e（real goal→quiz、Mode B agent heredoc 跑兩次＝迴圈恢復、`cd && codebus` 形式仍被擋）確認。F4 未回退。
**Effort:** 輕（約半天）

---

## 一句話

commit `26ba1d0`（2026-05-23、agent-hook-hardening）把 `<` 加進 Bash PreToolUse hook 的 `SHELL_METACHARACTERS` denylist，結果把 quiz SKILL body 明文教 agent 跑的自我驗證 heredoc `codebus quiz validate - <<'CBQZ'` 一起擋掉了。quiz Mode B 的 in-session 自驗迴圈在 claude path **自 2026-05-23 起靜默失效**。

## 證據（全 grounded、自己 grep + 跑 binary 驗）

1. **`<` 在 denylist**：`codebus-cli/src/commands/hook.rs:171-173` `SHELL_METACHARACTERS = [';','&','|','$','`','>','<','(',')','\n','\r']`
2. **quiz SKILL body 教 heredoc**：`codebus-core/src/skill_bundle/mod.rs:248` + `636-643`，原文「Validate your draft via the Bash tool using a heredoc ... `codebus quiz validate - <<'CBQZ'` ... the heredoc is **the only way**」（cat-pipe 第一字是 `cat` 被擋、無 file-write tool）
3. **跑實際 binary 確認**：
   ```
   echo '{"tool_input":{"command":"codebus quiz validate - <<'"'"'CBQZ'"'"'"}}' | codebus.exe hook check-bash
   → {"decision":"block","reason":"hook: command contains forbidden shell metacharacter `<`; ..."}
   ```
4. 對應 test 仍鎖 heredoc 形式：`mod.rs:1764 stub_content_quiz_claude_mode_b_has_heredoc` 斷言 claude SKILL body 必含 `<<'CBQZ'` → 機制仍 live、非已棄

## 後果 / Severity 校準

- ❌ **不是** correctness break——caller-side（codebus CLI）在 agent 結束後仍會跑 `codebus quiz validate`、broken citation 仍被抓、user 仍看到 `content_review: flagged`
- ✅ **是** quality / capability **靜默** regression——agent 失去 in-session「自驗→修→再驗」迴圈、quiz 初稿品質下降、且沒有任何錯誤被 surface（agent 的 Bash tool call 被 hook 默默 block）
- 對應 memory `quiz_agent_self_validate_heredoc`（heredoc 是當初為繞 hook「只放行 argv[0]=codebus」設計的 workaround、現在 workaround 自己被新 hook 擋）
- self-inconsistency：同一個 commit `26ba1d0` 改了 `skill_bundle/mod.rs`（86 行）卻沒拿掉 heredoc 指示，design 前提「agent 無 metachar 合法用途」（design.md:41）被它自己的 quiz SKILL body 打臉

## 範圍限定

- **claude path only**——codex 的 quiz SKILL body 不含 heredoc（PowerShell 無 heredoc、`mod.rs:1784` 斷言 codex body 不得有 `<<'CBQZ'`）；codex 走 `[CODEBUS_QUIZ_NO_VALIDATE]` 誠實 surface（per prompt-surface Phase 5 spike 結論）

## Proposed fix（三選一、apply 階段定）

| 選項 | 內容 | 風險 |
|---|---|---|
| **A. denylist 對 heredoc 特例放行** | `is_allowed_bash_command` 對「quiz validate 形式 + 開頭 `<<'MARKER'` heredoc + `-` stdin」放行（其餘 metachar 仍擋） | 要加 heredoc 形狀 parse、複雜度上升、denylist 不再純 char-scan |
| **B. 從 denylist 拿掉 `<`** | `<` 只是 input redirect（讀導向）、`>` 寫導向仍擋；靠 argv[0]/argv[1] + Claude `--allowedTools Bash(prefix *)` 兜底 | 削弱對 `<file` input-redirect 的防禦（但威脅模型主要是 chaining/substitution/write-redirect、那些 char 仍在）|
| **C. 改 SKILL body 走非 heredoc 自驗** | 找一條不含 metachar 的自驗路徑 | memory 說 cat-pipe（第一字非 codebus）+ temp-file（無 write tool）都不可用、heredoc 是唯一路——C 很可能走不通；除非改 codebus CLI 給 quiz validate 一個無 stdin 的 in-arg 形式 |

→ 初步傾向 **A**（精準、不削弱其他防禦）或 **B**（最簡、`<` 的威脅其實低）。apply 第一步先確認：實機 quiz-generate 跑時 agent 的 heredoc 是否真的撞到 hook（vs 被 `--allowedTools` 先放行），grounded 後再定。

## 驗收

1. 修完後跑實際 binary：`codebus quiz validate - <<'CBQZ'` → 不再 block（或走 A 只對此形式放行）
2. 確認其他 metachar 形式（`codebus lint; rm`、`codebus lint $(x)`）仍 block（不能因為放行 heredoc 開了 chaining 後門）
3. 真實 quiz-generate CDP smoke：agent in-session 自驗迴圈恢復、events 看得到 validate 被跑
4. spec `lint-feedback-loop` Fix Bash Hook Installation 段同步（若走 A、加 heredoc 例外 scenario）

## Out of scope

- 全面 denylist → allowlist 遷移（已決定不做、見 BACKLOG「已決定不做」表）
- codex path quiz 自驗（codex 無 heredoc、by-design 走 NO_VALIDATE）

## 何時動

不阻塞（靜默 degrade 已久、caller-side 兜底）。可跟其他 hook / quiz 議題 batch。若 user 常用 quiz 想恢復 in-session 自驗品質、可優先。
