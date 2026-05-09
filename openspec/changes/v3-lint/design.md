# v3-lint 設計

## Context

v3 架構已選 agentic AI product 定位 — `goal` 與 `query` 都是 CLI spawn `claude -p` agent process 的模式。但 cli/spec.md 的 Stub Verb Exit Behavior 仍把 `lint` 與 `fix` 列為 stub，實際只印 `not yet implemented` 然後 exit 1。

對比歷史：

- legacy v1（TS）有完整 lint 規則（`legacy/ts-src/src/core/wiki/lint.ts` 268 行），但無 fix loop。
- legacy v2（Rust）archived 的 `lint-feedback-loop` change 引入 `wiki/fix/{mod,prompt,memory}.rs` 共 829 行的 CLI 機械式 loop，使用 `LlmProvider::invoke` trait（in-process）。
- v3「fresh start」commit 砍掉 v2 的 LlmProvider trait，全面改用 `claude -p` spawn-process pattern；lint/fix 兩個 stub 留待本 change 補完。

外部驗證：

- `claude --help` 確認 `--allowedTools "Bash(git *)"` 細粒度語法直接生效（v2.1.137 實測）。
- 實測 `claude -p "..." --session-id <uuid>` 後 `claude -p "..." --resume <uuid>` 能正確 recall 前文，session 在 `-p` mode 完整支援。

衝突點：v3-init 既有 `skill-bundles` spec 明定「SHALL NOT write skill bundles into `<repo>/.claude/skills/`」，本 change 必須 patch 該 requirement 為雙位址寫入。

## Goals and Non-Goals

**Goals**：

- `lint` 與 `fix` 兩個 verb 從 stub 移除，落地可用。
- fix loop 採 agent self-driven 為主、CLI 外層 ping 為強保證的補救機制。
- 三個 skill bundle (goal/query/fix) 雙位址寫入，使用者直觸 skill 與 CLI spawn 都能找得到。
- goal flow 整合 lint→fix loop，commit 摺單顆。
- lint 自動偵測 vault root，CLI 直跑與 agent 自呼共用同一規則集。
- JSON 輸出絕對路徑，agent 跨 cwd 一致解析。

**Non-Goals**：

- 不處理 `codebus-app`（Tauri）整合，留給 app 動工再評估（破口 3、4 標到 v3 roadmap §7）。
- 不引入 MCP server / sidecar lint tool 給 fix agent — 用 `Bash(codebus lint *)` whitelist 已足。
- 不復活 v2 的 `LlmProvider` trait 抽象，v3 維持 spawn-process 模型。
- 不寫「使用者要 cd 到 .codebus/」UX 教學 — 雙位址 skill 寫入直接解掉。
- fix loop 不加 oscillation guard（沿用 v2 lint-feedback-loop 決策；靠 outer_ping_max 自然終止）。
- 不保留 v2 config key alias（`lint.auto_fix.*`）— v3 clean break，新命名為 `lint.fix.*`。

## Decisions

### Agent-driven self-loop

fix agent session 內自跑 `codebus lint --format json` → 修檔 → 再跑 lint，直到 lint 乾淨或自宣告改不動。loop 的主要動力是 agent 自己。

替代 A：CLI-driven loop（v2 模式）— CLI 每輪 spawn 新 agent process 帶當前 issues + 上輪 git diff。**否決理由**：v2 模式依賴 in-process `LlmProvider`，v3 改 spawn-process 後每輪重啟代價高（context 重 load + token cost）；且 v3 沒 trait 抽象可用。

替代 B：agent 內建驗證（不跑 lint，agent 自己驗 frontmatter 規則）— **否決理由**：lint 規則複雜（slug catalog、跨 folder 撞名、code region strip），agent 自驗會 drift；deterministic 規則用機率方案解。

### CLI outer ping

agent 結束後 CLI 跑 final `lint_wiki()` 校驗。若仍有 issue 且 ping budget > 0，CLI 用 `--resume <uuid>` 帶 follow-up prompt（含剩餘 issues）喚起同一 session。

預設 `outer_ping_max=2`。v2 的 `max_iterations=5` 是「整段 loop」的上限；v3 agent 自己已 self-loop，外層 ping 純補救，2 次足。

### Bash whitelist 細粒度

fix agent sandbox 為 `Read,Glob,Grep,Write,Edit,Bash(codebus lint *)`。

替代 A：`Bash(codebus *)` — 否決，agent 可能誤呼叫 `codebus init/goal/fix` 鬼打牆。
替代 B：`Bash` 全開 — 否決，破壞 v3 minimal sandbox 慣例。
替代 C：自製 MCP / sidecar lint tool — 否決，工程量超出本 change 範圍。

CLI flag 字面寫法：`--allowedTools "Read,Glob,Grep,Write,Edit,Bash(codebus lint *)"`（claude --help v2.1.137 example 確認支援）。

### Vault 自動偵測

`codebus lint` 啟動時優先序：

1. `<cwd>/wiki/` 存在 → cwd 即 vault root（agent 從 `.codebus/` cwd 自呼場景）
2. `<cwd>/.codebus/wiki/` 存在 → cwd 是 source repo root（使用者直跑場景）
3. `--repo <PATH>` 顯式指定 → 覆寫前兩條
4. 都沒有 → exit 2 + hint 「run `codebus init` first」

封裝在 `codebus-core/src/wiki/lint/locate.rs`。`init`/`goal`/`query` 不採此偵測，維持既有「明確 repo_root + append .codebus」慣例（這些 verb 不會被 agent 自呼）。

替代：lint 只認 `--repo` flag，agent 自己加 `--repo ..` — 否決，SKILL.md 要教 agent 怪招且容易出錯。

### 雙格式輸出

`codebus lint`（text，預設）：給人看，issue path 印 vault-relative（`concepts/auth.md`）。
`codebus lint --format json`：給 agent 看，issue path 印絕對路徑（`/abs/path/to/.codebus/wiki/concepts/auth.md`）。

替代：JSON 也用 vault-relative + 一個獨立 `vault_root` 欄位 — 否決，agent 多一層字串組合容易錯。

### Skill bundle 雙位址寫入

init 寫到兩個位置：
- `<repo>/.codebus/.claude/skills/codebus-{goal,query,fix}/`（既有，CLI spawn 用 cwd=vault root）
- `<repo>/.claude/skills/codebus-{goal,query,fix}/`（新增，使用者在 source repo root 開 Claude Code 直觸）

兩位置內容相同（相同 SKILL.md）。SKILL.md 維持 cwd-relative 路徑（既有設計），靠 lint vault 自動偵測解掉 cwd 差異。

替代 A：寫到 `~/.claude/skills/`（user-global）— 否決，跨 repo 共用一份，多 repo 不同 codebus 版本會打架。
替代 B：只 `codebus-fix` 寫雙位址，goal/query 維持單位址 — 否決，使用者面對「哪些 skill 哪裡找」不對稱認知。

source repo 的 `.claude/skills/codebus-*/` 加進 `.gitignore`（沿用 init 既有 gitignore mutation 機制）。

### Goal commit 摺單顆

goal flow 改寫為：goal agent 結束後 **不立即 commit**，先跑 lint → fix loop，全部結束才 `auto_commit` 一次，commit message `wiki: <goal-text>`。fix loop 結束結果不影響 commit 是否發生（沿用 v2「失敗也提交部分修復」）。

替代：兩顆 commit（goal commit + fix commit）— 否決，git log 噪音雙倍且關聯性不明。

**BREAKING**：v3-goal 既有 spec「Goal subcommand auto-commits on agent success/failure」要 patch 為「auto-commits AFTER lint-fix loop」。

### Session continuity

CLI 為每次 fix loop 生成 UUID，第一次 spawn `claude -p "/codebus-fix" --session-id <uuid>`；外層 ping 用 `claude -p "<follow-up>" --resume <uuid>` 喚起同一 session。

agent 在同一 session 內保有完整 context（前輪修了什麼、lint 還剩什麼），無需 v2 的 git diff snapshot 跨輪記憶機制。

### codebus-fix 使用者直觸

使用者在 Claude Code 互動 session 直接打 `/codebus-fix` 觸發 SKILL.md。此模式下：

- agent self-loop 跑（同 CLI spawn 場景）
- 但**無 CLI 外層 ping、無 final lint 校驗、無 auto_commit**
- agent 結束就結束；使用者自己決定是否再執行 / 手動 commit

SKILL.md 內容必須在兩種觸發模式下都成立 — atomic 契約（拿 issues、修檔、結束）不含 loop 邏輯，由 caller（CLI 或使用者）持有 loop 控制。

## Risks / Trade-offs

- [Agent 宣告 done 但 lint 仍有 issue] → CLI 外層 ping 強保證；ping 用完仍未過則 commit + exit 1 讓使用者知道 fix 不完整。
- [Agent oscillate（多輪改不動 / 來回改）] → 沒 oscillation guard，靠 outer_ping_max 上限自然終止。極端 case agent 可能浪費 token，personal dev tool 可接受。
- [雙位址 skill 寫入 vs `.gitignore` 衝突] → init gitignore mutation 已負責；新增的 `<repo>/.claude/skills/codebus-*/` 也加進 ignore 清單。
- [`Bash(codebus lint *)` 跨平台 binary 解析] → v3-init 已假設 codebus 在 PATH（cargo install 後）；fix 沿用同假設，未在 PATH 時 fix fail-fast 報錯。
- [BREAKING goal commit timing] → archived v3-goal 留歷史 spec；新 spec 取代既有 requirement，不影響使用者既有 vault 的 git 狀態。
- [BREAKING skill-bundles 路徑] → 既有 vault re-run init 會把第二位址補上（write-if-missing 慣例）；既有第一位址不動。
- [`/codebus-fix` 互動模式無 CLI 強保證] → 設計取捨；使用者直觸是手動模式，由使用者自己負責終止與 commit。SKILL.md 在文末說明這個邊界。
- [v2 → v3 config key 不相容（`lint.auto_fix.*` → `lint.fix.*`）] → v3 是 fresh start，不保留 backward compat；若使用者有 v2 config 會被忽略，新 config 用預設值。

## Migration Plan

- **既有 v3 vault**：re-run `codebus init` 把第二位址 skill bundle 補上；vault 內容不變、git 歷史不變。
- **既有 v3-goal 使用者**：本 change 落地後新 goal 用新 commit timing；歷史 commit 不受影響。
- **無 vault 使用者**：`codebus init` 一次到位寫雙位址。
- **v2 vault 使用者**：v2 → v3 是新工具，不負責 migrate v2 config（v2 config key 名與 v3 不同）；vault 結構同（.codebus/wiki/...），既有內容可被 v3 lint 認得。

無資料 migration（vault 內容格式不變）。

## Open Questions

- 使用者直觸 `/codebus-fix` 時，agent 怎麼跟使用者報告「修了什麼 / 改不動什麼」的最佳格式？SKILL.md 撰寫時定（不是 spec 範圍）。
- `--no-fix` flag 是 goal 跟 fix 共用，還是只 goal 用？目前傾向兩個 verb 都接受（goal 用是「跳過自動 fix」，fix 用是 noop 但允許不報錯）。spec 階段定。
