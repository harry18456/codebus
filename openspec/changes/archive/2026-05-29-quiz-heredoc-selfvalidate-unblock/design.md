## Context

PreToolUse Bash hook（codebus-cli/src/commands/hook.rs）對 codebus agent 沙箱的 Bash 工具做硬性閘控：`is_allowed_bash_command` 先以 `find_shell_metacharacter` 對原始命令字串做 byte-level metachar 掃描（任何命中即 false），再以 argv tokenize 檢查是否為 `codebus lint *` 或 `codebus quiz validate *`。`SHELL_METACHARACTERS` 集合涵蓋 semicolon、ampersand、pipe、dollar、backtick、greater-than、less-than、open-paren、close-paren、LF、CR。

commit `26ba1d0`（2026-05-23、agent-hook-hardening）為修補 F4 shell-metachar bypass 把 less-than（`<`）加入該集合。副作用：claude quiz SKILL（codebus-core 的 skill_bundle 模組 Mode B 段）明文教 agent 跑的自驗 heredoc `codebus quiz validate - <<'CBQZ' ... CBQZ` 含 `<`，自此被 hook 靜默 block，quiz in-session「自驗→修→再驗」迴圈失效。caller-side（agent 結束後 CLI 跑 `codebus quiz validate`）仍兜底，故非 correctness break，而是品質的靜默 capability regression。

**關鍵約束**：本 change 在動剛經 adversarial re-audit 確認修好的 F4 安全 hook。任何放寬都不得讓 chaining / command-substitution / write-redirect 的 bypass 後門復活。既有 hook 單元測試必須全綠，並加新測試守「放行 heredoc 但仍擋 chaining/substitution/unquoted/非 heredoc redirect」。

**propose 階段技術發現（影響 parse 設計）**：Claude Bash tool 送出 heredoc 時，`tool_input.command` 是**整段多行字串**（含 quiz draft body）。backlog 的 binary 復現只送第一行故只觸發 `<`，但真實命令還含多個換行（heredoc body 必有）、且 quiz 內容常含 dollar、pipe、paren 等 metachar。因此「放行 heredoc 運算子但其餘照 char-scan」會被 body 擋死——放行必須以**結構化 parse**處理，body 視為 opaque stdin。

## Goals / Non-Goals

**Goals:**

- 恢復 claude path quiz Mode B 的 in-session 自驗 heredoc（`codebus quiz validate - <<'CBQZ' ... CBQZ`）能通過 hook。
- 在不重開 F4 的前提下達成：放寬面最小、僅針對精確的單引號 quiz-validate heredoc 形狀，其餘含 `<` 的命令（含非 heredoc input-redirect）照擋。
- 為每一種 evasion（heredoc+chaining、heredoc 後接命令、unquoted heredoc、here-string、非 heredoc redirect）留對應 block 測試，既有測試零回退。

**Non-Goals:**

- **Option B（從 denylist 整個拿掉 `<`）**：被否決。會讓 `codebus quiz validate < ~/.ssh/id_rsa`、`codebus lint < /etc/passwd` 這類 input-redirect 復活；`<` 雖只是讀導向，但放寬面遠大於 heredoc 特例。僅在 Option A 證實不可行且回報 user 重新對齊時才考慮。
- **Option C（改 SKILL body 走非 heredoc 自驗）**：被否決。cat-pipe（首字 `cat` 非 codebus 被擋）、temp-file（agent 無 file-write tool）都不可用，heredoc 是唯一路；除非改 codebus CLI 給 quiz validate 一個無 stdin 的 in-arg 形式（scope 擴大），否則走不通。
- 不改 claude/codex 的 SKILL body：claude 已教單引號 `<<'CBQZ'`（正是放行形狀），codex 無 heredoc、by-design 走 `[CODEBUS_QUIZ_NO_VALIDATE]`，皆不動。
- 不改 caller-side `codebus quiz validate`（correctness backstop，本 change 只恢復 in-session quality 迴圈）。
- 不改 quiz validate CLI 的 stdin 介面。
- 不做全面 denylist 轉 allowlist 遷移（已決定不做）。

## Decisions

### Decision 1：以結構化 heredoc parse 放行，而非 char-scan 例外

放行邏輯**不是**「掃整條命令、除 heredoc 運算子外無 metachar」（heredoc body 必含換行、且常含其他 metachar，會被擋死）。改為把命令字串結構化拆解：

1. 以換行切行，每行去除尾隨 CR（跨平台行尾）。
2. **首行**（line 0）必須是：`codebus quiz validate` 形式 + 緊接 heredoc 運算子 `<<'MARKER'`。具體：
   - 找首個 `<<`，切成 prefix（`<<` 之前）與 op_rest（`<<` 之後）。
   - op_rest 必須完全匹配「單引號包住的 MARKER 後僅餘空白」——即單引號 + 一段 word 字元 + 單引號 + 選擇性尾隨空白，**之後不得有任何字元**（擋首行 chaining，如 `<<'X'; rm`）。
   - prefix 必須：(a) 經 `find_shell_metacharacter` 掃描為 None（無其他 metachar）；(b) 滿足 `is_codebus_quiz_validate_command`（argv[0] 為 codebus binary、argv[1] 為 quiz、argv[2] 為 validate）。SKILL 形式含 stdin `-`，但 `-` 非安全承載點，不強制要求其存在。
3. **heredoc body**（首行之後到收尾 MARKER 之間的行）視為 **opaque stdin，不做 metachar 掃描**。
4. **收尾**：line 1 之後的行中必須存在一行（去 CR 後）**完全等於** MARKER（heredoc 結束標記，需在行首、無前導空白，對應 SKILL 教的非 `<<-` 形式）。
5. **收尾之後**：收尾 MARKER 行之後的所有行必須為空或純空白；任何非空白內容即拒絕（擋收尾後 trailing command，如收尾標記後再接 `rm -rf ~`）。
6. 全部通過則放行；任一不符則回退到既有 metachar 轉 allow-form 路徑（多半因 `<` 或換行被 block）。

**理由**：body 含換行與任意 quiz 內容，唯一安全且不誤殺的做法是把 body 當不可信 stdin 隔離，安全保證集中在「首行不可 chaining」「收尾後不可有命令」「delimiter 須單引號」三道結構規則上，而非逐字元掃描。

替代方案：「strip 掉 body 後只驗首行 tokens」——等價但仍需處理收尾後 trailing 與 unquoted，複雜度相同，故採完整結構化 parse。

### Decision 2：僅放行單引號 delimiter `<<'MARKER'`，unquoted `<<MARKER` 維持 block

只有單引號（或語意等價的 quoted）delimiter 才放行。unquoted `<<MARKER` **維持 block**。

**理由（安全關鍵）**：body 被視為 opaque、不掃描。在 unquoted heredoc 下，shell 會對 body 做參數／命令／算術展開——body 內若含命令替換（例如 dollar-paren 包住的 curl 管到 sh）會在 shell eval 時執行。若放行 unquoted 等於把一個不掃描的區段交給 shell 展開＝重開注入面。單引號 delimiter 下 body 為字面值、無展開，傳給 codebus stdin 是惰性的，故安全。

**與原 backlog/提案 allow 清單的差異（明確標注）**：原提案把 `codebus quiz validate - <<CBQZ`（unquoted）列為 allow case。本 design 將其改為 **block**，因為在 opaque-body 設計下 unquoted 是真實注入向量，且 SKILL 實際只 emit 單引號 `<<'CBQZ'`，故 quoted-only 不損失任何真實功能。此為「不重開 F4」這個主約束的忠實落實。雙引號 delimiter `<<"MARKER"` 在技術上同樣禁展開（安全），但 SKILL 從不 emit，為最小放寬面本 change 不放行（未來如需可再放寬）。

### Decision 3：抽純函式 is_quiz_validate_heredoc helper，find_shell_metacharacter 語意不變

新增純函式 `is_quiz_validate_heredoc(cmd: &str) -> bool` 封裝 Decision 1 的結構化規則，可獨立單測。`is_allowed_bash_command` 改為：**先**呼叫 `is_quiz_validate_heredoc`（命中即放行），未命中才走既有「`find_shell_metacharacter` 命中即 false，否則 `is_codebus_lint_command` 或 `is_codebus_quiz_validate_command`」。

`find_shell_metacharacter` 與 `SHELL_METACHARACTERS` 的語意、內容**完全不動**——`<`、換行仍在集合內，只是對「已被 `is_quiz_validate_heredoc` 確認為合法 heredoc 形狀」的命令先行放行。非 heredoc 形狀的 `<` 命令（含 input-redirect）一律照舊被擋。

**理由**：把放寬封裝成一個可單測、邊界明確的結構化 predicate，不污染既有 metachar 集合的純 char-scan 語意；回退路徑保證任何不符 heredoc 形狀者落回原有防線。

### Decision 4：SKILL body 與 codex 路徑不動

claude quiz SKILL 已教單引號 `codebus quiz validate - <<'CBQZ'`（skill_bundle 模組 Mode B 段），正是放行形狀；既有斷言測試 `stub_content_quiz_claude_mode_b_has_heredoc`（必含 `<<'CBQZ'`）維持綠、無需修改。codex 路徑無 heredoc、走 `[CODEBUS_QUIZ_NO_VALIDATE]`，斷言測試 `stub_content_quiz_codex_mode_b_no_validate_marker`（必不含 `<<'CBQZ'`）亦不受影響。故本 change 不觸及 codebus-core/src/skill_bundle/mod.rs。

## Implementation Contract

**Behavior（可觀察）**：
- claude quiz Mode B agent 送出的 `codebus quiz validate - <<'CBQZ'`（接 draft body 後以 `CBQZ` 收尾，含 `--json` 變體）Bash 呼叫被 hook 放行：`codebus hook check-bash` exit 0、stdout 無 decision JSON；agent in-session 自驗迴圈恢復。
- 下列一律維持 block（exit 0 加 block decision JSON）：heredoc 首行夾帶 chaining（收尾標記後接 semicolon-rm、ampersand-ampersand-curl）、收尾 delimiter 後接命令、unquoted heredoc、here-string（三個 `<`）、非 heredoc input-redirect（`codebus quiz validate < ~/.ssh/id_rsa`、`codebus lint < /etc/passwd`）、以及所有既有 F4 向量（`codebus lint; echo PWNED`、`codebus lint` 接命令替換 等）。

**Interface / data shape**：
- 新增純函式 `is_quiz_validate_heredoc(cmd: &str) -> bool`（hook 模組內，可在測試 module 單測）。
- `is_allowed_bash_command(cmd: &str) -> bool` 行為擴充：新 heredoc 例外在最前、未命中回退既有邏輯。
- 無 CLI flag 變更、無 stdin/stdout JSON 契約變更、無前端牽動。

**Failure modes**：
- 任何不符合 heredoc 結構規則者一律回退到既有 block 路徑（fail-closed 不變）。
- 對「形狀近似 heredoc 但被拒」者，block reason 由 `find_shell_metacharacter` 取首個 metachar（多為 `<`），訊息可能未精確指出真正拒因（如 trailing semicolon）；此為 cosmetic、仍是 block、非安全承載，可接受。

**Acceptance criteria**：
1. `cargo test -p codebus-cli --bins hook::` 全綠（既有 hook 測試加新增 heredoc allow/block 矩陣測試）。
2. 實際 binary：餵 full heredoc（含 body 加換行）`codebus quiz validate - <<'CBQZ' ... CBQZ` 得 allow；`codebus quiz validate - <<'X'` 接 `; rm -rf ~` 得 block；unquoted heredoc 得 block；`codebus lint; echo PWNED` 得 block；`codebus quiz validate < ~/.ssh/id_rsa` 得 block。
3. 真實 claude path quiz generate CDP smoke：events / hook log 看得到 `codebus quiz validate` 被跑且 agent 依 findings 修正，不再靜默 block。
4. `pnpm tsc` 加 `pnpm test` 綠（預期無前端改動，僅確認未誤觸）。

**Scope boundaries**：
- In scope：codebus-cli/src/commands/hook.rs 的 predicate（新 helper 加接線）加其單元測試；lint-feedback-loop spec delta（Fix Bash Hook Installation 的 Allow 條款加新 scenario）。
- Out of scope：SKILL body（已正確）、codex 路徑、caller-side validate、quiz validate CLI stdin 介面、denylist 轉 allowlist 遷移、前端。

## Pre-apply 校準（apply 第一步、ground truth 後回填）

per 累積教訓（grounded debugging / 同 bug 同 change / propose 假設要 reproduce），apply 第一步須實機 ground，發現與本 design 假設的差異回填於此段：

1. Read codebus-cli/src/commands/hook.rs 完整 `is_allowed_bash_command` / `find_shell_metacharacter` / `is_codebus_quiz_validate_command` / `SHELL_METACHARACTERS` 加既有 metachar 測試區。
2. Read skill_bundle 模組 Mode B heredoc 指示原文加 claude/codex heredoc 斷言測試，確認 SKILL 仍 emit 單引號 `<<'CBQZ'`。
3. Read 本 spec lint-feedback-loop 的 Fix Bash Hook Installation 段（allow/block 條款加既有 scenario）。
4. **實機 ground**：跑一次 claude path quiz generate（CDP smoke 或 CLI），確認 (a) agent 真的 emit `codebus quiz validate - <<'CBQZ'` heredoc 並被 hook block（events/hook log 看得到 block decision）；(b) `tool_input.command` 的**實際形狀**——是否如假設含完整 body 加多個換行、delimiter 是否確為單引號、是否含 `--json`；(c) agent 被 block 後的行為（改寫成別的形式／放棄自驗）。
5. 若實機形狀與本 design 假設不符（例如命令只含首行、或 delimiter 非單引號），於此段記錄並調整 Decision 1 的 parse 規則後再進 RED 測試。

**Findings（apply 2026-05-29 實機 ground）**：

- **(b) 命令實際形狀 — 假設成立。** 用真實安裝的 `codebus`（即 vault `.claude/settings.json` PreToolUse hook 實際呼叫的同一支 binary）餵入「SKILL 明文教 agent emit」的完整多行 heredoc（`codebus quiz validate - <<'CBQZ'` + 多行 body + 收尾 `CBQZ`），確認 `tool_input.command` 為單一多行字串、含多個 LF、且 body 行含 `|` `$` `(` `)` `;` `>` 等 metachar。delimiter 確為單引號。→ 印證 Decision 1：char-scan 會被 body 擋死，**結構化 parse（body opaque）為必要**；Decision 1 parse 規則無需調整。
- **(a) hook 確實 block — 假設成立。** 上述命令得 `{"decision":"block","reason":"...forbidden shell metacharacter `<`..."}`、exit 0。`<` 為字串中首個命中的 metachar（故 reason 指 `<` 而非 body 內其他符號），對齊 Failure modes 段的 cosmetic 說明。
- **PreToolUse 獨立於 --allowedTools — 架構確認。** PreToolUse hook 在工具執行前獨立觸發、可 block，與 `--allowedTools` 的許可提示分離；故 heredoc 必撞 hook（非被 allowedTools 先放行）。此為 Claude Code hook 架構（查 docs + code，非黑箱猜測）。
- **(c) agent 端到端 — Task 5.1 已 live 確認（2026-05-29、真實 GUI + CDP）**：切 provider 為 claude、re_init `.codebus` vault（裝當前 quiz skill + hook）、跑真實 goal 建 5 頁 wiki，再跑真實 quiz generate（Mode B）。CDP activity stream 觀察到：agent 先試 `cd "...\.codebus" && codebus quiz validate`（含 `&&` → hook 正確 block），隨即改用純單引號 heredoc `codebus quiz validate - <<'CBQZ' ## Q1...`（**被放行、實際跑了兩次** = validate→fix→re-validate 迴圈），其後重讀 wiki 頁修正，最終產出 10 題結構正確的 quiz。同時證明：heredoc 放行（修法生效）+ chaining 仍擋（F4 未回退）。對照修法前（裝舊 binary 時 heredoc 靜默 block）成立。

## Risks / Trade-offs

- **[heredoc body 不掃描可能藏東西]** → body 僅流向 `codebus quiz validate` 的 stdin（read-only 驗證器、不 exec）；單引號 delimiter 禁止 shell 展開；收尾 delimiter 後不得有命令。三重保證下 body 無法逃逸到 shell 執行。
- **[結構化 parse 引入新 bypass]** → 對每種 evasion（首行 chaining、收尾後 trailing、unquoted、here-string、多重 heredoc 運算子）各留 block 測試；既有 hook 測試守回退不破。
- **[實機命令形狀與假設不符]** → Pre-apply 校準 Task 先實機 ground 真實 agent-emitted 命令，再依確認形狀定 parse；不符即回填調整。
- **[block reason 對被拒 heredoc 指 less-than 而非真正拒因]** → cosmetic、仍 block、非安全承載，接受。
- **[工時]** → 上限半天；若 heredoc-shape parse 比預期 tricky、或實機發現 agent 根本沒走 heredoc，stop 找 user 對齊。

## Open Questions

- 實機 `tool_input.command` 是否確含完整 heredoc body 加換行（影響 parse 必要性）——apply Task 1 ground 後即定，預期為「是」。
