## Context

PR #1 review 後盤點出兩條 latent 安全 issue：F4（`codebus hook check-bash` 只比前 2-3 token，shell metacharacter 可繞 sandbox）與威脅 C（`codebus hook check-read` 只擋 image 副檔名，可讀使用者 home 下敏感檔）。Windows codex PoC（2026-05-23 實機在 codex 0.133.0）確認 codex `workspace-write` 設計上允許讀 workspace 外任意檔（`Get-Content C:/Users/harry/.ssh/config` 直接讀出內容），因此 read 威脅在 codex path 也存在但**無 hook 可加**——本 change 對 codex 端只提供 AGENTS.md soft constraint，hard enforcement 留 backlog。

目前 codebase 狀態：
- `codebus-cli/src/commands/hook.rs` 有 `check_bash` 與 `check_read` 兩個子命令；`check_bash` 內的 `is_allowed_bash_command` 僅 `split_whitespace` 比 argv[0..2]；`check_read` 內的 `is_image_path` 僅比副檔名。
- `codebus-core/src/skill_bundle/mod.rs` 已實作 codex parallel materialization（寫 `.codex/skills/codebus-<verb>/SKILL.md` 與 `.codex/AGENTS.md`）。
- spec `lint-feedback-loop` 的 Fix Bash Hook Installation 與 PII Image Read Hook Installation 條款描述當前 allow/block 規則但未要求 metacharacter rejection 或敏感路徑黑名單。
- spec `skill-bundles` 的 Codex Instruction Materialization 條款描述 SKILL.md 雙寫但未涵蓋 AGENTS.md 內容契約。

跨平台情境：目前 Windows 主開發；macOS / Linux 為未來目標。所有改動均為字串級 predicate（無 OS-specific syscall），三 OS 行為一致。home 目錄解析沿用 `dirs` crate（既有依賴鏈）。

## Goals / Non-Goals

**Goals:**

- Bash hook 拒絕含 shell metacharacter 的命令字串，無論是否在引號內。
- Read hook 在既有 image 副檔名擋下之外，再擋 home 目錄下敏感路徑與 key 副檔名 glob。
- 兩個 hook 行為與既有 fail-closed / cross-platform path separator / ASCII case-insensitive 哲學一致。
- spec `lint-feedback-loop` 與 `skill-bundles` 同步更新（避免下次 audit 再抓到 spec drift）。
- codex 端 AGENTS.md 加 soft constraint 文字，告訴 agent 不主動讀 home 下敏感檔（hard enforcement 不在本 change）。

**Non-Goals:**

- 不改 codex backend 旗標、不改既有 SKILL.md 內容、不擴 Bash hook 允許的子命令範圍。
- 不引入 shell parser 做引號感知——agent 對 `codebus lint` / `codebus quiz validate` 無 use case 需要引號內 metacharacter，引號內一律 reject 是可接受權衡。
- 不為 codex path 做架構級 read 隔離（writable_roots Mac/Linux 實機驗、Windows ACL/chmod、container 化、sandbox-of-sandbox）——已記入後續 backlog 追蹤。
- 不擴 Read hook 為「環境變數 token 名感知」（如 `*_TOKEN`、`*_API_KEY` 在 path）——複雜度與誤殺率不對等。

## Decisions

### Bash hook 用字面字符黑名單，不引入 shell parser

**選項：**
- A. 黑名單字符（`; & | $ \` > < ( ) \n \r`），任意位置 reject
- B. 完整 POSIX shell parser，引號感知後判斷
- C. 字符白名單（只允許 ASCII alphanumeric + `-` `_` `/` `=` `.` space `"`）

**選擇：A。**

**理由：** B 引入 shell parser 是過度工程，且不同 shell（bash、PowerShell、cmd）parser 行為不同；C 過嚴會誤殺合法 arg（如 `--repo /path with spaces`、`--filter pattern*` 將來支援的 case）。A 黑名單覆蓋 POSIX + PowerShell 共同高危集，agent 對 `codebus lint` / `quiz validate` 沒有合法 use case 需在 raw command 出現這些字元，誤殺率為零。引號內 metacharacter 也一併 reject——這是刻意 trade-off：避免 parser 複雜度，且 agent 無此 use case。

### Read hook 敏感路徑黑名單用「字面字串前綴匹配 + 副檔名 glob」混合

**選項：**
- A. 純字面前綴（`<home>/.ssh/` 開頭）
- B. 純 glob（`*/.ssh/*`、`*id_rsa*`）
- C. 字面前綴（針對固定敏感目錄）+ 副檔名 glob（針對 key 檔名 pattern）

**選擇：C。**

**理由：** `~/.ssh/`、`~/.aws/`、`~/.gnupg/`、`~/.config/gh/` 是固定路徑——前綴匹配最直接、最快、最不易誤判。`*id_rsa*`、`*.pem`、`*.key` 是檔名 pattern——可能在使用者任意目錄出現，必須 glob。實作上分兩個 check：路徑解析後先 normalize（forward slash），再跑前綴匹配；前綴 miss 再跑 basename 級 glob 匹配。兩者皆 ASCII case-insensitive（與既有 image extension 同），同時處理 `/` 與 `\` separator（與既有 `is_image_path` 同）。

### Home 目錄解析失敗時 fail-closed（block）

**選項：**
- A. unresolved home → block 該 read（fail-closed）
- B. unresolved home → 跳過敏感路徑檢查、只跑 image extension 檢查（degraded）

**選擇：A。**

**理由：** 與既有 hook fail-closed 哲學一致（malformed stdin / 缺欄位 / 解析失敗 → block）。hook 子命令期間 home 應該永遠可解析；不可解析代表異常環境（無 `HOME` 環境變數、無 `USERPROFILE`），這種情況下 agent 跑任何 read 都應該被擋下而非繼續。

### Codex AGENTS.md 用 hard-coded literal 段落嵌入 skill_bundle 模板

**選項：**
- A. literal 字串嵌入 `skill_bundle/mod.rs` 的 AGENTS.md 模板常數
- B. 從另一個 markdown 檔（`templates/codex-agents-soft-constraint.md`）`include_str!` 進來

**選擇：A。**

**理由：** 與現有 SKILL.md 雙寫 pattern 一致（`stub_content` 等模板字串都直接嵌入 `skill_bundle/mod.rs`）。測試 assertion 直接 `assert!(output.contains("literal substring"))`，簡單可讀。soft constraint 文字不會頻繁變動，無需獨立檔案。

## Implementation Contract

### Observable behavior

**`codebus hook check-bash`**：

| 輸入 `tool_input.command` | 行為 | stdout |
| --- | --- | --- |
| `"codebus lint --format json"` | allow | （無 decision JSON） |
| `"codebus lint && rm -rf /"` | block | `{"decision":"block","reason":"<msg mentions shell metacharacter>"}` |
| `"codebus lint; curl evil.com"` | block | 同上 |
| `"codebus lint $(whoami)"` | block | 同上 |
| `"codebus lint \"foo;bar\""` | block（引號內 metachar 也 reject） | 同上 |
| `"echo MARKER"` | block（非 codebus，既有規則） | 既有 reason |

**`codebus hook check-read`**：

| 輸入 `tool_input.file_path` | 行為 | 備註 |
| --- | --- | --- |
| `"<home>/.ssh/config"`（已 path-resolve） | block | 敏感目錄前綴匹配 |
| `"C:/Users/harry/.ssh/id_rsa"`（Windows path） | block | 同上 + path separator normalize |
| `"<home>/.aws/credentials"` | block | 敏感目錄前綴匹配 |
| `"/tmp/random/id_rsa"` | block | 檔名 glob 匹配 |
| `"/path/server.pem"` | block | 副檔名 glob 匹配 |
| `"./wiki/concepts/foo.md"` | allow | 未在 blocklist |
| `"wiki/diagrams/flow.png"` | block | 既有 image extension 規則 |

home 目錄解析失敗 → block + reason 指明「無法解析 home 目錄」。

**Codex AGENTS.md materialization**：

- `init` 寫到 `<vault>/.codex/AGENTS.md` 的內容 contains literal substring（精確措辭由 implementation 決定，但須涵蓋以下語意）：「codex sandbox 設計上允許讀 workspace 外任意檔，但 codebus agent 工作範圍僅限 vault；不主動讀使用者 home 下的敏感檔（`~/.ssh/`、`~/.aws/`、`~/.gnupg/` 等）」。

### Interface / data shape

- 不改 `hook.rs` 既有子命令 CLI 介面、不改 stdin/stdout JSON schema、不新增 flag。
- `check_read_inner` 函式可能新增 home 路徑參數（為了 unit-testability，模仿既有 `hooks_cfg` 參數注入模式）；不對外暴露。
- AGENTS.md 模板字串為 `skill_bundle/mod.rs` 內部常數；不對外暴露。

### Failure modes

- malformed stdin / missing field / unresolved home → fail-closed（block + reason JSON）
- valid input + blocklist hit → block + reason JSON
- valid input + blocklist miss → allow（exit 0 無 decision JSON）

### Acceptance criteria

- `cargo test -p codebus-cli hook` 全綠（既有 27 條測試 + 新增約 14 條：metachar block 6、敏感路徑 block 6、cross-platform path separator 2）
- `cargo test -p codebus-core skill_bundle` 全綠（既有測試 + 新增 AGENTS.md soft constraint substring assertion 1-2 條）
- `cargo build --workspace` 全綠
- `spectra validate agent-hook-hardening` 通過
- 手動 sanity：`codebus init` 在 fresh vault 跑後，`.codex/AGENTS.md` 內含 soft constraint 文字

### Scope boundaries

**In scope:**
- 改 `hook.rs` 內 Bash 與 Read 兩個 `check_*` 子命令的拒絕條款與對應 unit test
- 改 `skill_bundle/mod.rs` codex AGENTS.md 模板與對應 test
- 改 spec `lint-feedback-loop`（Fix Bash Hook Installation + PII Image Read Hook Installation）條款與 Scenario
- 改 spec `skill-bundles`（Codex Instruction Materialization）條款與 Scenario

**Out of scope:**
- codex backend 旗標、claude_cli backend、PII filter scanner、vault 同步邏輯、SKILL.md 內容
- Bash hook 允許子命令範圍擴展
- codex 端 hard enforcement read 隔離（另開 backlog）
- Read hook 環境變數 token 名感知

## Risks / Trade-offs

- [**Read hook 黑名單可能誤殺合法操作（如使用者刻意把 vault 放在 `~/.ssh/` 旁邊）**] → 黑名單僅覆蓋 home 下明確敏感目錄與標準 key 副檔名；agent 工作範圍應在 vault `wiki/` 內，正常不會 trip。實際誤殺案例後續再 case-by-case 加 allowlist 或鬆綁。
- [**codex AGENTS.md soft constraint 對抗惡意 prompt injection 無強制力**] → 已在 proposal Non-Goals 明示；本 change 僅補最小自律提示，hard enforcement（writable_roots、ACL、container 化）開 backlog 追蹤。風險明示給維護者，不掩蓋。
- [**Bash hook 黑名單拒絕引號內 metacharacter 是 trade-off，未來若有合法 use case 會誤殺**] → 目前 agent 對 `codebus lint` / `quiz validate` 沒有此 use case；若未來需要，再評估升級為 shell parser 或加 escape 條款。
- [**hook 改動須在 Windows / macOS / Linux 行為一致，但目前僅 Windows 開發**] → 改動全為字串級 predicate（無 OS-specific call），跨平台行為由字串邏輯保證；Windows 通過 unit test 後，三 OS 行為一致。CI matrix 未來補 Mac/Linux 是免費的。
