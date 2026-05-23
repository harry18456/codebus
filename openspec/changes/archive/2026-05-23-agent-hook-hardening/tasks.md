<!--
Each task description states the behavior delivered AND the verification target.
File paths appear only as supporting locator context, never as the task itself.
-->

## 1. Bash hook metachar rejection

- [x] 1.1 RED：為 `Fix Bash Hook Installation` 要求的「metacharacter rejection」failure modes 在 `codebus-cli/src/commands/hook.rs` 既有 unit test 模組新增 block 案例（涵蓋 `&&`、`;`、`$()`、backtick、`|`、`>`、`<`、`(`、`)`、`\n`、`\r`；含「引號內 metachar 仍 block」一條）。完成行為：6+ 個新測試 FAIL（實作未動）。驗證方式：`cargo test -p codebus-cli hook::tests` 輸出顯示新增測試名稱為 failed，既有 27 條測試仍 pass。
- [x] 1.2 GREEN：實作「Bash hook 用字面字符黑名單，不引入 shell parser」決議——`is_allowed_bash_command` 在 token-prefix 比對前先做 raw command 的 metacharacter byte-level rejection；observable behavior 為 block decision JSON 含命中字元名稱（per design.md `Observable behavior` 表第二、三、四列）。完成行為：1.1 所有新測試 + 既有 27 條測試 pass。驗證方式：`cargo test -p codebus-cli hook` 全綠（約 41 條）。
- [x] 1.3 Scope boundaries 對齊：確認改動限定在 `is_allowed_bash_command` 與相關 helper，不動 `check_bash` 子命令整體流程、不擴 allow 子命令範圍、不改 `~/.claude/settings.json` 模板（per design.md `Scope boundaries`）。完成行為：除預定函式外無其他 source code 變動。驗證方式：`git diff` 範圍肉眼 review + cross-platform sanity（拒絕集合為三 shell 共同高危集，無 OS-specific 分支，Windows local test 通過即可推論跨 OS 一致）。

## 2. Read hook sensitive path blocklist

- [x] 2.1 RED：為 `PII Image Read Hook Installation` 要求的「Read hook 敏感路徑黑名單用「字面字串前綴匹配 + 副檔名 glob」混合」決議在既有 unit test 模組新增測試（涵蓋 `<home>/.ssh/`、`<home>/.aws/`、`<home>/.gnupg/`、`<home>/.config/gh/` 各一條；basename glob `*id_rsa*`、`*.pem`、`*.key` 各一條；含「路徑在 home 外但 basename glob 命中」一條；含「`~` 開頭被擴展」一條）。完成行為：8+ 個新測試 FAIL。驗證方式：`cargo test -p codebus-cli hook::tests` 顯示新測試 failed。
- [x] 2.2 RED：為「Home 目錄解析失敗時 fail-closed（block）」決議的 failure mode 新增測試（兩條：home 不可解析 + 路徑需要 home 解析 → block；home 不可解析 + basename glob 可單獨判斷 → 仍走 basename glob 不卡）。完成行為：2 個新測試 FAIL。驗證方式：同 2.1。
- [x] 2.3 GREEN（含 interface / data shape 變更）：在 `check_read_inner` 既有 image extension 檢查之後、allow 之前新增 sensitive path 檢查；分兩階段（normalize 路徑分隔 + 展開 `~` 跑 prefix 匹配 → basename glob 匹配）；home 解析失敗且需要時 fail-closed。函式 interface 加 home 參數以利 unit test 注入（per design.md `Interface / data shape`）。完成行為：2.1 + 2.2 所有新測試 + 既有 read hook 測試全綠。驗證方式：`cargo test -p codebus-cli hook` 全綠（約 50+ 條）。
- [x] 2.4 Cross-platform path separator 驗證：手動驗證 Windows path（`C:\Users\harry\.ssh\config`）與 Unix path（`/home/x/.ssh/config`）均能 block；observable behavior 在兩種 separator 一致（per design.md `Observable behavior` Read hook 表第二列）。完成行為：path-separator normalize 在兩種寫法下命中同一條規則。驗證方式：local test 中 cross-platform case 全綠 + task notes 記錄已涵蓋。

## 3. Codex AGENTS.md soft constraint

- [x] 3.1 [P] RED：為 `Codex Instruction Materialization` 要求的 sensitive-read soft constraint 在 `codebus-core/src/skill_bundle/mod.rs` 既有 test 模組新增測試（assert 生成的 AGENTS.md 內容包含 `~/.ssh/`、`~/.aws/`、`~/.gnupg/` 三個 literal 字串，且包含「workspace-write」與「vault」字樣）。完成行為：新測試 FAIL。驗證方式：`cargo test -p codebus-core skill_bundle::tests` 顯示新測試 failed，既有測試仍 pass。
- [x] 3.2 [P] GREEN：實作「Codex AGENTS.md 用 hard-coded literal 段落嵌入 skill_bundle 模板」決議——在既有 AGENTS.md 模板字串加入 soft constraint 段落，內容涵蓋 spec scenario 要求的 (a) 三個 literal 路徑、(b) 承認 codex workspace-write 允許讀外、(c) 指示 agent 留在 vault scope。observable behavior 為 `init` 寫出的 `.codex/AGENTS.md` 包含該段落（per design.md `Observable behavior` AGENTS.md 章節）。完成行為：3.1 測試 pass，既有所有 skill_bundle 測試 pass。驗證方式：`cargo test -p codebus-core skill_bundle` 全綠。
- [x] 3.3 [P] 手動 sanity 與 scope 確認：在臨時目錄跑 `codebus init` 後打開 `.codebus/AGENTS.md` 肉眼確認段落措辭通順、與既有段落視覺一致；確認改動沒擴及 SKILL.md 內容或 codex backend 旗標（per design.md `Scope boundaries` Out of scope 列表）。完成行為：手動 review 通過 + scope 守住。驗證方式：截圖或 paste 段落到 PR 描述 + `git diff` 範圍 review。

## 4. 整合與最終驗證（acceptance criteria）

- [x] 4.1 全工作區測試（acceptance criteria 第一條）：`cargo test --workspace` 全綠，含 codebus-cli + codebus-core + 任何下游依賴測試。完成行為：整個 workspace 通過。驗證方式：`cargo test --workspace` 輸出 0 failures。
- [x] 4.2 spectra validate（acceptance criteria 第二條）：`spectra validate agent-hook-hardening` 通過——spec/design/tasks 一致性、無 forbidden words、Scenario 格式皆正確。完成行為：validate 0 errors 0 warnings。驗證方式：CLI 輸出。
- [x] 4.3 跨平台 reasoning 與 scope boundaries 留底：在 PR 描述記錄「所有 hook 改動為字串級 predicate，無 OS-specific syscall，三 OS 行為一致；目前 Windows local test 通過，Mac/Linux 待 CI matrix 上線後補驗」與「實際變更檔案僅限 `codebus-cli/src/commands/hook.rs` 與 `codebus-core/src/skill_bundle/mod.rs` + 兩條 spec」（per design.md `Scope boundaries`）。完成行為：跨 OS 推理與 scope 守住兩件事留底，future 維護者讀 PR 即知範圍。驗證方式：PR description 含此段落。
