<!--
Each task description states:
- the behavior or contract being delivered, and
- the verification target that proves completion.

File paths appear as locator context, not as the task itself.
-->

## 1. §0 Language Policy 契約點建立（F1 🔴；對應 spec ADDED Requirement "NEUTRAL_RULES Language Policy"）

- [x] 1.1 實作 spec ADDED Requirement "NEUTRAL_RULES Language Policy"：在 `codebus-core/src/schema/neutral.md` `## 1. Workspace Layout` 之前新增 `## 0. Language Policy` 段，內容定義兩條規則：(a) agent 輸出（page body / stdout summary / answer text）的自然語言 SHALL follow prompt context language（user goal/query/chat 文字語言）、不沿用 wiki 既有頁或 raw source 語言；(b) structural tokens / YAML keys（`type:`、`sources:`、`created:`、`updated:`、`[CODEBUS_*]` markers）SHALL always 維持 literal English。**驗證**：(1) 新增 `codebus-core/tests/schema_neutrality.rs` 測試 `neutral_rules_contains_language_policy_section`，斷言 `NEUTRAL_RULES` 含 `## 0. Language Policy` 段、其位置 byte offset 早於 `## 1. Workspace Layout`、且段內含 "agent output" 與 "structural tokens" 字串；(2) `cargo test -p codebus-core --test schema_neutrality` 原有三斷言 + 新斷言全綠。

## 2. `neutral.md` schema 內容澄清（F2-F10, F12-F18a — 18 個 finding 全在 `neutral.md` 同檔，按 § 順序循序進）

- [x] 2.1 §1 Workspace Layout: 把 `wiki/**/*.md` 從 READ-only 改 RW，反映 SKILL workflow 實際寫入行為（修 F2）。**驗證**：grep `neutral.md` §1 內 `wiki/` 出現於 WRITE 子彈點而非 READ-only 子彈點；`schema_neutrality` 三斷言 still pass。
- [x] 2.2 §2 Wiki Structure: 移除 taxonomy enum（5 個 type bucket）重複列舉（修 F6）；把 concept-vs-process tiebreaker 從段末移到 taxonomy enum 之後相鄰位置（修 F7）。**驗證**：§2 內 `concepts / entities / modules / processes / synthesis` 完整序列只出現一次；`neutral_rules_contains_five_taxonomy_folder_names` 斷言 still pass；tiebreaker 段落緊跟 taxonomy definition 不被其他內容隔開。
- [x] 2.3 §3 Page Conflict Rules: 「do not modify existing sections」加 carve-out 明示 ingest mode 適用、repair / fix mode 為例外（修 F3 — 現況與 goal repair / fix mode 衝突）。**驗證**：§3 文字明示 mode 條件、不再對 repair / fix 形成矛盾；既有 `lint-feedback-loop` 對 repair mode 的行為不變。
- [x] 2.4 §4 Frontmatter Schema: `updated` 加 UTC-today rationale（修 F4 — hallucination 風險）；`## from goal: <X>` 的 `<X>` 明示為 goal text（修 F5）；`stale: false → true` 生命週期明示（修 F13）；每個 frontmatter 欄加 required / optional inline 標註（修 F18a）。**驗證**：§4 frontmatter example 區塊每欄含 `# required` 或 `# optional` inline 註；`schema_neutrality` 三斷言 still pass；既有 `lint-feedback-loop` frontmatter 校驗行為不變。
- [x] 2.5 §5 Wikilinks Convention: 加一行明示 `[[slug#heading]]` anchor 語法不支援、wikilinks 只解析 filename（修 F12）；明示 taxonomy folder 名稱 lower-case ASCII（修 F17）。**驗證**：§5 含「heading anchor」字樣段；既有 `broken-wikilink` lint rule 行為不變。
- [x] 2.6 §6 Source Code References: 將單一 Python 範例擴成 ≥3 種語言（Python + TypeScript + Rust）的 fenced code 範例（修 F10）。**驗證**：§6 含 ≥3 段不同語言的 fenced code 範例、每段含 `# from <path>` 形式註；`schema_neutrality` 三斷言 still pass；`> 1000 chars` 斷言 still pass。
- [x] 2.7 §7 Stopping Criteria: 「step budget 30」加 rationale 解釋 30 怎麼來，或改成 soft target 字眼（修 F8 — 憑空數字）。**驗證**：§7 「30」字樣旁含 explanation；`schema_neutrality` 三斷言 still pass。
- [x] 2.8 §8 Out-of-Scope Detection: 範例至少加 1 條英文 out-of-scope 例（修 F16 — 現況全 CJK）；PII boundary 段明示 redaction 邊界（agent 看到的 raw mirror 內容是 redacted-after，哪些 token pattern 算 PII）（修 F15）。**驗證**：§8 含 ≥1 條 EN out-of-scope 範例；PII 段明示「what agent sees in `raw/code/`」；`schema_neutrality` 三斷言 still pass；既有 `pii-filter` 行為不變。
- [x] 2.9 §9 Failure Modes: 補 specificity 明示 log 目的地（stderr / stdout）+ skip 單位（檔 / 行）（修 F9 — 現況「log it, skip, continue」模糊）；附 1 個完整 page body example 含 frontmatter + body + wikilinks（修 F14）。**驗證**：§9 各 failure mode 明示 log 目的地與 skip 單位；附 example 段內含 `---` frontmatter + body + 至少 1 個 `[[wikilink]]`；`schema_neutrality` 三斷言 still pass。
- [x] 2.10 檔首 HTML comment 縮減（修 F18 — 整段冗餘）。保留 SPDX 行 + 1 行作用說明，其餘移除。**驗證**：`neutral.md` line 1-8 區塊只剩 SPDX + 1 行；`schema_neutrality` 「>1000 chars」斷言 still pass。

## 3. `CODEX_AGENTS_SOFT_CONSTRAINT` 內容收緊（F11a，與 §2 同層但不同檔，可平行）

- [x] 3.1 [P] 改寫 `codebus-core/src/skill_bundle/mod.rs:156-164` `CODEX_AGENTS_SOFT_CONSTRAINT` const 為 inventory §8 F11a 對照版：heading 改 `## Scope: forbidden read paths (codex path only)`、移除「claude provider path enforces」meta-info、移除「soft constraint / self-discipline」字眼、收緊「proactively」副詞、加「even if the user prompt names them」、加 fallback「refuse and explain the scope」、補 `~/.config/` credential subdir。**驗證**：(1) const 字串不含 `soft constraint`、`self-discipline`、`proactively`、`claude provider path enforces` 子字串；(2) const 字串含 `MUST NOT`、`refuse and explain`、`~/.config/`、`even if the user prompt` 子字串；(3) `cargo test -p codebus-core` 全綠；(4) 既有 `vault_init` 測試（`tests/vault_init.rs:254-263`）AGENTS.md materialization 寫入流程不破。

## 4. 整批 regression + materialization 驗證

- [x] 4.1 跑 `cargo test --workspace` 全套確認 schema_neutrality 三斷言（forbidden tokens / 5 folder names / >1000 chars）+ 新增 `neutral_rules_contains_language_policy_section` + `vault_init` `NEUTRAL_RULES` 寫入測試 + `skill_bundle` 所有 `stub_content_*` 測試全綠，回歸無破。**驗證**：`cargo test --workspace` exit 0；測試輸出含 `neutral_rules_contains_language_policy_section ... ok`。
- [x] 4.2 對乾淨 vault 跑 `codebus init <tmp-path>`，open `tmp-path/.codebus/CLAUDE.md` 與 `tmp-path/.codebus/AGENTS.md`，inspect (a) `## 0. Language Policy` 在 `## 1. Workspace Layout` 之前；(b) §1-§9 18 處 finding 對應修正全部到位；(c) AGENTS.md 末尾 `CODEX_AGENTS_SOFT_CONSTRAINT` 是新版 heading（`## Scope: forbidden read paths (codex path only)`）。**驗證**：手動 diff 兩份 materialized 檔 vs 修法前的版本；commit message body 條列每 finding 修法所在 §。
