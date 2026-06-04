## Context

兩個缺口同屬「agent run 完整性」，但落在不同層、彼此獨立：

- **(A)** `agent::invoke`（provider-agnostic 主迴圈）目前把 child stderr 整條 `io::copy` 進 `io::sink()`、從不分類；`sandbox_denial_count` 只在主執行緒從 stdout 解析出的 `StreamEvent::ToolResult { is_error }` 命中 `is_sandbox_denial` 時累加。偵測器 `is_sandbox_denial`（`stream/sandbox_signal.rs`）是現成純函式。codex `exec` 在 Windows 常見「頂層 process exit 0、但內層 shell 被 OS 沙箱擋、拒絕字串只走 stderr」→ 計 0、run 標 succeeded、稽核看不見。
- **(B)** 裝 `check-bash` / `check-read` 硬閘的 vault 設定檔（vault 內 .claude/settings.json）由 `vault::settings`（write-if-missing、保留 user 客製）在 init 時寫一次；spawn 路徑不重寫。它落在 goal/fix agent 的 Write 可及 cwd 內、無完整性偵測。`codebus lint` 目前規則集（`wiki/lint`）依 `lint-feedback-loop` spec 限定**只掃 `wiki/` subtree**、`VaultContext` 也只由 `wiki_root` 建。

現有約束（必須尊重，不可違反）：
- `vault::settings` 的 write-if-missing / byte-identical 保留契約（spec `lint-feedback-loop` 的 Fix Bash Hook Installation requirement + 多次 archived change 確立）。
- `sandbox_denial_count` 的「best-effort observability、SHALL NOT 改變 outcome」契約（spec `run-log` + `verb-library` Sandbox Denial Signal Observability）。
- lint 的 read-only invariant（lint 不得寫 vault）。

## Goals / Non-Goals

**Goals:**
- (A) 讓 child stderr 出現的 sandbox/permission denial 也計入 `sandbox_denial_count`，補上 codex-Windows-exit-0 的稽核盲點。
- (B) 讓 `codebus lint` 能偵測 vault gate 設定檔的兩條必要 hook 被移除/改空，回報一條 error 級 issue；`fix` 既有 lint precheck/final 自動帶到。
- 兩者皆走偵測/可觀測性，零行為破壞、零既有 vault migration。

**Non-Goals:**
- 不 per-spawn 重寫 settings.json（違反 write-if-missing 契約）。
- 不加 Write/Edit deny-hook（需既有 vault migration、成本高）。
- (B) 偵測非預防：竄改當下不擋、下次 lint/fix 才報。
- (B) 不變成通用 vault 結構 linter；只驗那兩條 hook。
- (A) 不改 outcome 語意、不預設轉發 stderr 到終端。
- 不碰 codex OS sandbox、claude read denylist、env scrub、PII pattern（各自 backlog）。

## Decisions

**D1 — (A) stderr 改逐行分類、denial 計數經 JoinHandle 合流（不用共享 atomic）。**
stderr 背景執行緒從 `io::copy` 改成 `BufRead` 逐行：每行跑 `is_sandbox_denial`，命中則累加一個 thread-local 計數；`forward_stderr` 為真時該行仍寫到終端、為假時丟棄（等同既有 sink 行為，只是多看一眼）。執行緒結束時把 denial 計數作為 `JoinHandle` 回傳值；主迴圈在既有的 join 點取得它、加總到 stdout 來源的 `sandbox_denial_count` 後再放上 `InvokeReport`。
- 為何不用 `Arc<AtomicUsize>`：計數只需在 join 後彙整一次，回傳值最單純、無共享可變狀態。
- 去重：stdout 的 `ToolResult` 與 stderr 行可能描述同一次 denial → 採「兩來源相加、可能高估」而非試圖跨流去重（計數本就是 best-effort 訊號、寧可多報；spec 明列此語意）。

**D2 — (B) 檢查放進 `lint-feedback-loop`、以新 requirement 表達，而非塞進 wiki Rule Set。**
wiki Rule Set 的「只掃 wiki/、SHALL NOT scan 其他 subtree」契約保持不動；新增獨立 requirement「Vault Gate Integrity Check」明確、狹窄地授權 lint 額外讀**單一檔** .claude/settings.json。Purpose 微調點名此例外。
- 為何不放 `vault` spec：偵測的**輸出表面**是 lint（user 跑 lint / fix 自動帶到）；把檢查邏輯與報告格式留在 lint spec 較內聚。
- 為何不擴 wiki Rule Set 那條 requirement：避免污染「wiki-only」這個清楚契約。

**D3 — (B) 期望 hook 值單一來源、由 `vault::settings` 匯出。**
`vault::settings` 既有 `DEFAULT_SETTINGS_JSON`。新增匯出「必要 hook 期望集」(matcher → command 對：`Bash`→`codebus hook check-bash`、`Read`→`codebus hook check-read`)，DEFAULT_SETTINGS_JSON 與新規則都引用它，避免兩邊 drift。

**D4 — (B) 只驗「必要 hook 存在」、不驗 byte-identical。**
規則解析 settings.json 的 `hooks.PreToolUse`，確認兩條必要 hook 各自存在（matcher + command 對得上）。user 在 settings.json 額外加的東西不觸發 issue → 尊重 write-if-missing/客製契約。檔案不存在或 JSON 解析失敗或缺任一必要 hook → error issue。

**D5 — (B) `VaultContext` 加 `vault_root`；issue path 用 vault-relative 的設定檔路徑、formatter 不前綴 `wiki/`。**
`VaultContext::build` 額外帶入 vault_root（wiki_root 的父）。gate issue 的 path 以 vault-relative `.claude/settings.json` 表示（text 格式逐字呈現、不套 `wiki/<rel>` 前綴）、JSON 格式用其絕對路徑（延續 JSON 絕對路徑契約）。需在輸出層加一個「非 wiki-subtree issue 不前綴 wiki/」的分支。

**D6 — rule id 與 severity。**
rule id（kebab-case，與既有 `broken-wikilink` 等一致）：`vault-gate-integrity`；severity：`error`（硬閘失效是高優先）。

## Implementation Contract

**(A) 觀察行為**：一次 codex（或任何 provider）invocation 中，若 OS 沙箱拒絕訊息只出現在 child stderr，`InvokeReport.sandbox_denial_count` 與寫入的 `RunLog.sandbox_denial_count` 反映該次 denial（> 0）；既有 stdout 來源的計數不變。當 count > 0 時，既有的 `warning: sandbox-denial` stderr 一行照常發出。`outcome` 不因此改變。
- **介面/資料**：`InvokeReport.sandbox_denial_count: usize`（既有欄位、語意加寬來源）；stderr 背景執行緒的 `JoinHandle` 回傳 `usize`（該流命中數）。`is_sandbox_denial(&str) -> bool` 簽章不變、套用對象從「僅 ToolResult.output」擴為「ToolResult.output 與 stderr 每行」。
- **失敗模式**：stderr 讀取錯誤維持 best-effort（忽略、計數以已讀行為準），不讓 invoke 失敗；非 denial 行依 `forward_stderr` 決定轉發或丟棄。
- **驗收**：unit test 模擬「stdout 無 denial、stderr 含一條 curated marker」→ 計數為 1、outcome 不變；「stdout 與 stderr 各含 denial」→ 相加（容許高估）；`forward_stderr=false` 時非 denial 行不外洩到測試捕捉的終端。

**(B) 觀察行為**：對一個 settings.json 已被改空 hook 的 vault 跑 `codebus lint`，輸出含一條 `error` issue、rule=`vault-gate-integrity`、path 指向設定檔；對 hook 完整的 vault 跑則無此 issue；user 自行在 settings.json 加額外設定但兩條必要 hook 仍在 → 無此 issue。`codebus fix` 因既有 precheck/final 會自動偵測到此 issue。
- **介面/資料**：新 `LintRule` 實作 `VaultGateIntegrityRule`；`VaultContext { ..., vault_root }`；`vault::settings` 匯出必要 hook 期望集；JSON issue 沿用既有 `{path, severity, rule, message}` 形狀、`rule` = `vault-gate-integrity`。
- **失敗模式**：settings.json 不存在 / 非合法 JSON / `hooks.PreToolUse` 缺任一必要 hook → 各回一條 error issue、message 點名缺哪條或檔案問題。lint read-only invariant 維持（只讀不寫）。
- **驗收**：unit test 覆蓋「完整→0 issue」「缺 Bash hook→1 error」「缺 Read hook→1 error」「整個 PreToolUse 清空→error」「user 額外鍵但兩 hook 在→0 issue」「檔案缺→error」「JSON 損毀→error」；輸出 test 確認 text 格式不對此 path 前綴 `wiki/`、JSON 格式 path 為絕對路徑。
- **Scope 邊界（in）**：僅 `agent::invoke` 的 stderr 分類合流；僅一條讀 `.claude/settings.json` 的 lint 規則 + `VaultContext.vault_root` + 輸出層非-wiki path 分支 + `vault::settings` 期望集匯出 + 兩份 spec delta（verb-library MODIFIED、lint-feedback-loop ADDED；run-log 欄位契約不變故不動）。**（out）**：deny-hook、per-spawn 重建、outcome 改變、其他 vault 結構檢查、其餘安全 backlog。

## Risks / Trade-offs

- [(B) 偵測非預防：竄改下輪才生效、偵測在下次 lint] → 對 MEDIUM 有界威脅可接受；vault git diff 亦可見可還原；Non-Goal 明列。
- [(B) 輸出層長期假設所有 issue 在 wiki/ 之下，加非-wiki path 分支可能與既有 OSC8/relative 呈現邏輯交互] → 以 output unit test 釘住 text/JSON 兩格式對 gate issue 的呈現；gate issue 的 path 呈現契約由新 Vault Gate Integrity Check requirement 自帶 scenario 釘住（不改既有 Lint Output Formats requirement）。
- [(A) stdout+stderr 相加可能對同一 denial 高估] → 計數本為 best-effort 訊號（spec 明示不精確、不改 outcome）；高估只影響觀測、不誤判成功為失敗。
- [(B) 把 lint 從「純 wiki 內容」擴成「含一項 vault-gate 檢查」是契約面擴張] → 以獨立 requirement + Purpose 點名限縮、Non-Goal 防止滑坡成通用 vault linter。

## Migration Plan

無資料/格式 migration。(A) 純執行期行為；(B) 規則邏輯在 binary，對所有既有 vault 立即生效、不需重寫任何 vault 檔（這正是相對 deny-hook 方案的優勢）。回滾＝還原本 change 的 commit 即可。
