## Why

來自 agent-run 完整性 review 的 3c 組（BACKLOG row 31、2026-06-03 解法 discuss 定案）。兩個已 ground 的缺口：

- **(A) stderr 稽核黑洞**：invoke loop 把 child stderr 預設倒進 io::sink、從不分類；`sandbox_denial_count` 只從 stdout 的 tool result（is_error + 標記）累加。codex 在 Windows 頂層 exit 0、但某次工具被 OS 沙箱擋住、拒絕訊息只出現在 stderr 的情況下，denial 計 0、run 被標成 succeeded —— 稽核完全看不見被擋的事（可能是良性故障，也可能是探測/注入）。
- **(B) vault gate 可被 agent 自改**：裝 check-bash / check-read 硬閘的 vault 設定檔（.codebus/.claude/settings.json）落在 goal / fix agent 的 Write 可及範圍內，且沒有任何完整性偵測。被 prompt injection 的 agent 可把它改空，下一次 spawn 起兩道 hook 就失效。

Why now：解法已在 discuss 定案、脈絡新鮮；兩者皆為 detection / observability 補強、低風險、無需動 outcome 語意。

## What Changes

- **(A)** invoke loop 把 child stderr 改成逐行讀、每行也過 is_sandbox_denial 偵測器，命中就計入同一個 sandbox_denial_count（既有的 stdout tool-result 來源保留並存）。outcome 語意不變（denial 不翻 outcome，維持既有 best-effort observability 契約）；非 denial 的 stderr 行仍依現況丟棄（io::sink 預設、CODEBUS_FORWARD_AGENT_STDERR escape hatch 行為不變）。
- **(B)** codebus lint 新增一條 vault gate 完整性檢查：讀 vault 的 .codebus/.claude/settings.json，驗其 hooks.PreToolUse 仍含必要的 Bash→check-bash 與 Read→check-read 兩條 hook；缺失或被改空時報一條 error 級 lint issue（rule id：vault-gate-integrity）。fix 既有的 lint precheck / final 驗證會自動帶到此檢查。
- **(B)** lint 的 VaultContext 增加 vault_root 欄位，讓規則能讀 wiki/ 以外、精確限定的 .claude/settings.json（不擴張成通用 vault 結構 linter）。

## Non-Goals

- 不做 per-spawn 重新 materialize / 覆寫 settings.json —— 違反 settings.rs 既有的 write-if-missing / 保留 user 客製契約。
- (B) 是偵測、不是預防：不在被改的當下擋住，只在下次 lint / fix 時報出來（竄改下輪才生效 + vault git diff 可還原，對此 MEDIUM 有界威脅足夠）。
- 不新增針對 Write / Edit 的 PreToolUse deny-hook 方案（因既有 vault 需手動 migration、成本較高而否決）。
- (B) 不演變成通用 vault 結構檢查；只驗 settings.json 的那兩條 hook 在不在。
- (A) 不把 sandbox denial 變成會改變 run outcome 的訊號；不預設把 stderr 轉發到終端。
- codex path 的 OS sandbox、claude read denylist（F1/F2）、agent env scrub、PII pattern 擴充等其餘缺口各自獨立 backlog，不在本 change。

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `verb-library`: Sandbox Denial Signal Observability requirement 擴張 —— 偵測器除 stdout 的 ToolResult 事件外，也套用到 child stderr 的每一行。`run-log` 的 sandbox_denial_count 欄位契約（型別/序列化/不改變 outcome）不變、計數語意本就 defer 給此 requirement，故不另開 run-log delta。
- `lint-feedback-loop`: 新增 Vault Gate Integrity Check requirement —— lint 額外讀單一檔 .claude/settings.json、驗兩條必要 hook 在不在；精確限定（只讀該檔、不掃其他 subtree），wiki-only 掃描範圍不變。

## Impact

- Affected specs: verb-library (modified), lint-feedback-loop (modified)
- Affected code:
  - Modified:
    - codebus-core/src/agent/claude_cli.rs
    - codebus-core/src/stream/sandbox_signal.rs
    - codebus-core/src/wiki/lint/rule.rs
    - codebus-core/src/wiki/lint/factory.rs
    - codebus-core/src/wiki/lint/mod.rs
    - codebus-core/src/vault/settings.rs
  - New:
    - codebus-core/src/wiki/lint/rules/vault_gate_integrity.rs
  - Removed: (none)
