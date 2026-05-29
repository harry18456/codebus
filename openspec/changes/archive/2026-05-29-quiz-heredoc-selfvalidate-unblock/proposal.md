## Why

commit `26ba1d0`（2026-05-23、agent-hook-hardening）為修補 F4 shell-metachar bypass，把 `<` 連同其他 metachar 加進 Bash PreToolUse hook 的 denylist。副作用是把 quiz SKILL（claude path）明文教 agent 跑的自我驗證 heredoc `codebus quiz validate - <<'CBQZ'` 一起擋掉——quiz Mode B 的「自驗→修→再驗」in-session 迴圈自 2026-05-23 起**靜默失效**（agent 的 Bash tool call 被 hook 默默 block，沒有任何錯誤被 surface）。這不是 correctness break（caller-side `codebus quiz validate` 在 agent 結束後仍會跑、broken citation 仍被抓），而是 quiz 初稿品質的靜默 capability regression。需要在**不重開 F4** 的前提下恢復這條 in-session 品質迴圈。

## What Changes

- **精確放行 quiz-validate heredoc 形式**：在 PreToolUse hook 的 allow predicate 加一個結構化例外分支，僅放行「`codebus quiz validate` + 單引號 heredoc `<<'MARKER'` + stdin `-`」這個精確形狀；其餘任何含 `<` 的命令（含非 heredoc 的 input-redirect）照擋。
- **heredoc body 視為 opaque stdin**：放行邏輯不是「掃整條除了 `<<` 外無 metachar」（heredoc body 必含換行、且 quiz 內容常含 `$` `|` `(` 等），而是把命令結構化拆成「首行 / body / 收尾 MARKER / 收尾後」；body 不掃，只守首行不得 chaining、收尾 MARKER 後不得有 trailing command。
- **僅放行單引號 delimiter**：`<<'MARKER'`（body 無 shell 展開）才放行；unquoted `<<MARKER`（body 會觸發 `$()`／參數展開＝新注入面）維持 block。這比原 backlog 提案的 allow 清單更嚴格（原列了 unquoted 形式）——理由見 design.md Decision。
- **守 F4 不回退的新測試**：新增 hook 單元測試，覆蓋「放行乾淨 heredoc / 放行含 metachar 的 body」與「擋 heredoc+chaining / 擋 heredoc 後接命令 / 擋 unquoted heredoc / 擋非 heredoc input-redirect」；既有 hook 測試全數維持綠。
- **spec 同步**：`lint-feedback-loop` 的 Fix Bash Hook Installation 需求新增 heredoc 例外 precondition 與對應 scenario。
- **SKILL body 不變**：claude quiz SKILL 已教單引號 `<<'CBQZ'` 形式（codebus-core 的 skill_bundle 模組），正是放行的精確形狀；無需修改 SKILL，相關斷言測試維持綠。codex path（無 heredoc、走 NO_VALIDATE）不在此範圍。

## Non-Goals (optional)

被否決的修法（Option B 拿掉 `<`、Option C 改走非 heredoc 自驗）與完整 scope 排除，記於 design.md 的 Goals / Non-Goals 段。

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `lint-feedback-loop`: Fix Bash Hook Installation 需求的 Allow 條款新增「quiz-validate heredoc 例外」——以結構化（首行/opaque body/收尾 delimiter）規則放行單引號 heredoc 自驗形式，並維持對 chaining、substitution、write-redirect、unquoted-heredoc、非 heredoc input-redirect 的封鎖。

## Impact

- Affected specs: lint-feedback-loop（Modified — Fix Bash Hook Installation）
- Affected code:
  - Modified: codebus-cli/src/commands/hook.rs（新增 heredoc 結構化辨識 helper、接進 allow predicate、新增單元測試；既有測試不回退）
  - New: (none — 新測試與 helper 落在既有 hook.rs)
  - Removed: (none)
- 確認不需改動：codebus-core/src/skill_bundle/mod.rs（claude quiz SKILL 已教單引號 heredoc，正是放行形狀）；前端（無 UI 牽動）；codex SKILL body；caller-side validate。
