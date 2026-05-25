<!--
Each task description MUST state:
- the behavior or contract being delivered, and
- the verification target that proves completion.

File paths are supporting context for locating the work, never the task
itself. Cross-ref:
- spec requirements: "Codex-Side SKILL Mode Invocation Trigger" (skill-bundles),
  "Codex Sandbox Write Enablement Override" (codex-backend)
- design ### headings: Diagnose 走三層觀察 / 實際 diagnose 結論與修法 /
  `-c windows.sandbox=unelevated` 是對的 trade-off / 不為 multi-impl 預留抽象層 /
  Diagnose 觀察必須寫成 doc
-->

## 1. Diagnose 走三層觀察、找到根因即停

- [x] 1.1 [P] **Diagnose 層 (a)**：在 isolated 環境（npx 臨時版本或 PATH 切換）安裝 codex 0.132.0，於 `/tmp/exp-vault` 跑 `codebus quiz "JWT issuance and verification" --count 3`，比對 codex 0.133.0 baseline，落實「0.132 是否 emit `[CODEBUS_QUIZ_SCOPE]`」的二元觀察。**驗證**：層 (a) 紀錄段含 codex 版本、reproducer command、stdout 首三行 + 「emit/不 emit marker」結論；若 codex 0.132 裝不上 → 紀錄安裝失敗 stderr 並標 viable=false，可進下個 task。
- [x] 1.2 [P] **Diagnose 層 (b)**：寫一個 shim binary（Rust crate 內小 helper 或 PowerShell 腳本，stdout 印「ARGV=...」「STDIN=...」、exit 0），把 `CODEBUS_CODEX_BIN` 指向它，於同 vault 跑 `codebus quiz "..."`，dump codebus 實際送給 codex 的 argv 與 prompt 字串。**驗證**：層 (b) 紀錄段含 (i) ARGV 完整列表（行對行）+ (ii) `$codebus-quiz` sigil 是否原樣保留（不是 escaped/quoted/dropped）+ (iii) `--ignore-user-config / --disable apps / --ignore-rules / -c project_root_markers=...` 是否都在 argv 內，輸出 binary anomaly=yes/no 結論。
- [x] 1.3 [P] **Diagnose 層 (c)**：取層 (b) 攔到的 argv，直接呼叫 `codex exec --json ...`（不經 codebus），重定向 stdout 到檔案，檢視 stream events 是否含 `skill_invocation` / `skill_loaded` / `skill_not_found` 字串，或僅有 generic reasoning text。**驗證**：層 (c) 紀錄段含 (i) codex 呼叫 command 全文、(ii) stream events 中與 SKILL 相關事件 grep 結果（含「無相關事件」也算結論）、(iii) 結論 codex_handles_sigil=yes/no/skill_not_found。
- [x] 1.4 **整合 diagnose 結論**：根據 1.1–1.3 三層觀察，確定 root cause 屬於 CLI regression / codebus argv bug / SKILL bundle 不符 / 無法 root-cause 四種中的哪一種，寫入 `docs/2026-05-25-codex-skill-trigger-diagnose.md` 的「Root Cause 結論」段。**驗證**：doc 該段含 (i) 結論一句話、(ii) 對應 1.1/1.2/1.3 證據引用、(iii) 對映 design.md「修法選擇依 diagnose 結果擇一」表格的哪一列。

## 2. Diagnose 觀察必須寫成 doc 而非僅在 commit message

- [x] 2.1 **建立 diagnose doc 骨架**：建立 `docs/2026-05-25-codex-skill-trigger-diagnose.md`，含 Context / Reproducer / 三層觀察小節（每節有 reproducer command + actual output snippet 兩個必填子段）/ Root Cause 結論 / 選用修法理由 / 殘餘問題 / 相關文件連結。**驗證**：doc 存在、所有小節 heading 全部到位、連回 `docs/2026-05-24-codex-provider-experiment.md` 與本 change proposal、無 TBD / TODO 等 placeholder。
- [x] 2.2 **記錄選用修法理由**：在 diagnose doc 補一節「選用修法」，說明 design.md「修法選擇依 diagnose 結果擇一，不預先承諾」表格中為何 pick 該列，以及未來 codex 版本 bump 時的回切策略（例：若選 `/` description-match，標註 codex 0.134+ 修好 native sigil 後可回切 `$` 省 token）。**驗證**：doc 該節含 (i) pick 的修法名稱、(ii) 證據引用（哪一層觀察支持）、(iii) 後續版本 bump 回顧條件。

## 3. 修法選擇依 diagnose 結果擇一，不預先承諾

- [x] 3.1 **依 diagnose 結論寫單元測試（TDD）**：在對應修法檔案旁新增 / 修改測試，covering「Codex-Side SKILL Mode Invocation Trigger」spec requirement 在 codebus 端可直接 assert 的代理條件（例：`codex_backend::build_command` 輸出的 argv 內含預期 sigil 形式、或 SKILL bundle 寫出的 frontmatter 含預期 key）。測試 SHALL 先失敗（紅）。**驗證**：`cargo test -p codebus-core` 該測試名稱可被 grep 到、且在套修法前 fail、套修法後 pass；測試名稱寫進 diagnose doc「Acceptance Criteria」段。
- [x] 3.2 **套最小修法**：依 1.4 結論套對應修法（prompt sigil 切換 / argv 修補 / SKILL.md 內容調整 三選一或組合），不為 multi-impl 預留抽象層（不新增 trait method、不新增 SpawnSpec 欄位）。**驗證**：3.1 的單元測試由紅轉綠；`cargo build` 通過；`cargo test -p codebus-core` 全綠（無 regression）。
- [x] 3.3 **不為 multi-impl 預留抽象層之 review**：對 3.2 diff 自審，確認沒引入 single-impl trait / single-impl strategy enum / unused config field。**驗證**：自審 checklist 寫進 diagnose doc 末段（「per memory feedback_dont_speculative_abstract，本 change 未新增以下：trait method / SpawnSpec 欄位 / config 欄位」附 grep / `git diff --stat` 證據）。

## 4. 修完驗證 — codex-side SKILL Mode Invocation Trigger

- [x] 4.1 [P] **Quiz scope marker scenario 驗證**：active_provider=codex、`/tmp/exp-vault` 上跑 `codebus quiz "JWT issuance and verification" --count 3`，觀察 plan spawn 首行。**驗證**：log 首行為 `[CODEBUS_QUIZ_SCOPE] ...` 或 `[CODEBUS_QUIZ_NO_MATCH] ...`，且 codebus 不印 `quiz plan spawn produced no [CODEBUS_QUIZ_SCOPE]...` 錯誤；log 路徑寫進 diagnose doc。
- [x] 4.2 [P] **Goal wiki-write scenario 驗證**：active_provider=codex、同 vault 上跑 `codebus goal "summarize how the auth and database modules interact"`，觀察 agent 是否 Write 至少一個 `.codebus/wiki/**/*.md` 檔。**驗證**：跑完後 `find .codebus/wiki -newer <baseline>` 至少回傳一個新 / 改 page；agent stream 內含 Write tool-call 事件；log 路徑寫進 diagnose doc。
- [x] 4.3 [P] **Query vault-read scenario 驗證**：active_provider=codex、同 vault 上跑 `codebus query "how does JWT verification handle invalid signatures?"`，觀察 agent stream 是否含對 `.codebus/wiki/` 下檔案的 Read/Glob/Grep tool-call。**驗證**：agent stream grep 至少一個讀 vault 檔的 tool-call；最終答案含 wiki page 名或 wikilink；log 路徑寫進 diagnose doc。
- [x] 4.4 [P] **Chat no-meta-comment scenario 驗證**：active_provider=codex、同 vault 上 `echo "what does the auth module do?" | codebus chat`，觀察 response。**驗證**：response 不含「I found this is a documentation vault rather than application source」或同義語句；log 路徑寫進 diagnose doc。
- [x] 4.5 [P] **Fix SKILL-trigger + 修 lint warning scenario 驗證**：active_provider=codex、同 vault 製造一個 broken wikilink 後跑 `codebus fix`，觀察 agent 首動作 + lint 結果。**驗證**：agent 不 emit 「treating this as a planning task for the codebus-fix project」等 generic exploration prose、第一個 tool-call 直指 lint warning 對應的 wiki page；spawn 結束後重跑 `codebus lint` 該 warning 不再出現（B cluster 修法 covering 寫權限後此驗證項已 in scope）。
- [x] 4.6 **Codex SKILL trigger failure surfacing scenario 驗證**：模擬 4.1 失敗情境（暫時將修法 revert 或注入 broken SKILL.md），確認 codebus 印 stderr error 或 exit non-zero、不會 silent success。完成後還原修法。**驗證**：模擬期間 stdout/stderr 紀錄寫進 diagnose doc「Failure surfacing 驗證」段、含 exit code 與 error 文字 snippet。

## 6. 實際 diagnose 結論與修法（2026-05-25 完成）— Codex Sandbox Write Enablement Override

- [x] 6.1 **TDD: Codex Sandbox Write Enablement Override 單元測試**：在 `codebus-core/src/agent/codex_backend.rs` 的 `#[cfg(test)] mod tests` 中新增測試 covering「Codex Sandbox Write Enablement Override」spec requirement — assert `build_command(SpawnSpec { permission: Permission::Workspace, .. })` 與 `Permission::ReadOnly` 兩種輸出的 argv 都含完整的 `-c windows.sandbox=unelevated` 對偶。測試 SHALL 先失敗（紅）。**驗證**：`cargo test -p codebus-core --lib agent::codex_backend::tests` 可 grep 到測試名稱、在套修法前 fail、套修法後 pass；測試名稱寫進 diagnose doc「Acceptance Criteria」段。
- [x] 6.2 **套 Codex Sandbox Write Enablement Override 修法 — isolation recipe 補 `-c windows.sandbox=unelevated`**：在 `codex_backend.rs::build_command` 的 isolation recipe argv 後追加 `.arg("-c").arg("windows.sandbox=unelevated")`（或等效形式），對 Workspace 與 ReadOnly 兩 permission 一視同仁加進去（per codex-backend spec 兩 scenario）；不動其他 isolation flags、不新增 trait method / SpawnSpec 欄位 / config 欄位。**驗證**：6.1 測試紅轉綠；`cargo build --workspace` 通過；`cargo test -p codebus-core` 全綠（無 regression）。
- [x] 6.3 **Codex Sandbox Write Enablement Override trade-off 自審**：對 6.2 diff 自審：(i) argv 補的這一行確實出現在 Workspace 與 ReadOnly 兩種 permission（per codex-backend spec 兩 scenario）；(ii) 沒拿掉任何既有 isolation flag；(iii) 沒新增 trait method / config 欄位。**驗證**：自審 checklist 附 `git diff codebus-core/src/agent/codex_backend.rs` 證據、寫進 diagnose doc「Self-review」段或補一節「B cluster 修法 self-review」。


## 5. Housekeeping — Diagnose 觀察必須寫成 doc

- [x] 5.1 **Update memory `project_multi_provider_driver_confirmed`**：在 memory 加 2026-05-25 段落，撤回「codex 近期可實際跑」的舊 claim、改為「codex 0.133.0 上 SKILL trigger 已修復（via change codex-skill-trigger-fix）+ 修法形狀」摘要；連結 diagnose doc。**驗證**：memory 該檔內含 2026-05-25 dated entry + 修法形狀一句話 + diagnose doc link；無刪除既有 memory 結構（只 append/edit）。
- [x] 5.2 **Update memory `todo_codex_provider_regression_2026-05-25`**：把 P1 區段標記 done、註明 commit hash / change name；P2 (`codex-fix-sandbox-write`) 保留，明確標「依賴本 change 完成才可開」。**驗證**：memory 該檔 P1 段落含 done badge + 連結；P2 段落保留並標 blocked-by codex-skill-trigger-fix。
- [x] 5.3 **Cross-link diagnose doc 進 docs index**：若 `docs/` 有 README 或 index.md，加一行連結到 `docs/2026-05-25-codex-skill-trigger-diagnose.md`；若無 index，跳過此 task 並在 diagnose doc 末端註明「無 docs index、未做 cross-link」。**驗證**：grep `docs/2026-05-25-codex-skill-trigger-diagnose.md` 出現於 `docs/README.md` 或 `docs/index.md`；或 diagnose doc 末段含「無 docs index」說明（兩者擇一即算完成）。

## 7. C cluster — codex verify-stage spawn 撞 batch-file argv 限制（多行 prompt 走 stdin）

- [x] 7.1 **驗證 Rust .cmd argv hardening 是 root cause（最小 repro）**：寫 minimal Rust binary 用 `Command::new(codex.cmd).arg(multi_line).output()`，確認 Rust 1.77+ 拒含 `\n` arg；同 string 無 `\n` 則 OK；對比 raw_arg + stdin pipe 兩條繞路看誰真實 round-trip multi-line。**驗證**：repro 結果 + 三條繞路結論寫入 diagnose doc「C cluster Diagnose」段、含 stdout / 結論。
- [x] 7.2 **TDD: codex backend 對多行 prompt SHALL pass `-` + 提供 stdin_payload**：在 `codebus-core/src/agent/codex_backend.rs::tests` 加測試 covering「Codex Multi-Line Prompt Stdin Routing」spec requirement — assert (i) multi-line prompt argv 末元素為 `-`、(ii) no argv element contains `\n`、(iii) `stdin_payload(&spec)` returns `Some(formatted_prompt)`；單行 prompt argv 末元素仍為 formatted prompt、`stdin_payload` returns `None`。測試 SHALL 先失敗（紅）。**驗證**：`cargo test -p codebus-core --lib codex_assembly_sub_mode_input_with_newlines_uses_stdin_placeholder` 紅→綠；測試名稱寫進 diagnose doc「Acceptance Criteria」。
- [x] 7.3 **套 C 修法 — trait 加 stdin_payload + codex_backend 實作 + invoke pipe stdin**：在 `codebus-core/src/agent/backend.rs` 加 `fn stdin_payload(&self, _spec: &SpawnSpec) -> Option<String> { None }`；`codex_backend.rs` 新增 `format_codex_prompt` helper、`build_command` 對多行 pass `-` argv、實作 `stdin_payload` 對多行回 `Some(...)`；`claude_cli.rs::invoke` 讀 backend stdin_payload、`Some` 走 `Stdio::piped` + write_all、`None` 維持 `Stdio::null`。**驗證**：7.2 紅轉綠、`cargo build --workspace` 通過、`cargo test -p codebus-core` 全綠。
- [x] 7.4 **E2E 驗證 quiz / goal 的 verify spawn 不再 batch-file argv 失敗**：active_provider=codex、`/tmp/exp-vault` 上跑 `codebus quiz "<topic>"` 與 `codebus goal "<task>"`，stderr / stdout grep 「`spawn agent: batch file arguments are invalid`」應為空；verify 階段應 emit `CONTENT_OK` 或 `<id> | <type> | <suggestion>` line 而非「content-verify spawn failed」warning。**驗證**：兩個 verb log 經 grep 確認、結果寫進 diagnose doc「C cluster e2e 驗證」段。
- [x] 7.5 **`agent-backend` spec MODIFIED 對齊**：本 change `specs/agent-backend/spec.md` MODIFIED「Agent Backend Trait Contract」requirement — 從「exactly three methods」鬆綁為「3 required + optional methods with safe defaults」，加 optional stdin_payload scenario。**驗證**：`spectra analyze codex-skill-trigger-fix` 無 Critical / Warning（或僅 Suggestion-level）；`spectra validate` 通過。
