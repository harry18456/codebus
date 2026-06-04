## Context

codebus 透過 PreToolUse hook `codebus hook check-read` 守 claude-path 的 Read。現行 `check_read` / `check_sensitive_path` 是 **denylist**：擋 image 副檔名、`*id_rsa*`/`*.pem`/`*.key` basename、`~/.ssh`/`~/.aws`/`~/.gnupg`/`~/.config/gh` home prefix，全程 gated by `hooks.read_image_block`。settings.json 的 `hooks.PreToolUse` 只裝 `Bash`→check-bash、`Read`→check-read 兩個 matcher；`REQUIRED_HOOKS` 是 settings 內容、`vault-gate-integrity` lint rule、drift-guard 測試的單一真相。

這留下 F1（MEDIUM：絕對路徑 Read 可讀母 repo 未遮罩 source + denylist 外憑證如 `~/.kube`/`~/.env`）與 F2（LOW：Glob/Grep 無 hook、Grep -content 讀 .pem 內容繞過）。F2 於 2026-06-04 實機驗證確認。盤點得知：goal/query/chat/quiz 的 SKILL 已明文限定 cwd-relative + 禁 `..`/絕對路徑；唯一以絕對路徑讀 vault 內檔的正常功能是 **fix verb**（依 lint JSON `issues[].path` 的絕對路徑 verbatim 做 Read/Write/Edit）。

## Goals / Non-Goals

Goals:
- check-read 以 vault-root 含括取代 denylist 作為主邊界，一併封 F1+F2。
- hook 覆蓋 Glob/Grep。
- 不誤擋任何現有正常 verb（含 fix 的 vault 內絕對路徑）。
- 既有 vault 可被偵測並有補救路徑。

Non-Goals:
- 不改 codex-path（codex 走 OS sandbox、不裝此 hook）。
- 不移除既有 image/sensitive denylist（降為 vault 內 defense-in-depth）。
- 不在本 change 自動 in-place 改寫既有 vault 的 settings.json（維持 write-if-missing；migration 走偵測 + 引導）。
- 不處理 Write/Edit 邊界（由 acceptEdits + cwd confine 已守、非破口）。

## Decisions

### vault-root containment allowlist 取代 denylist 作為主 gate
canonicalize 目標 path 後要求落在 canonicalize 過的 vault root 內才放行；denylist（image/sensitive）保留為 vault 內 defense-in-depth。
理由：denylist 對「目錄根」Grep 無效（Grep path 給的是目錄、basename glob 不命中），且 agent 可直接 Grep 母 repo 撈 secret pattern、永不碰 `.pem` 檔名 → denylist 對 search 工具形同虛設。containment 是唯一同時封 F1+F2 的機制。
Alternative（rejected）：把 `.pem`/sensitive denylist 延伸到 Glob/Grep path — 可被「Grep 目錄根 + pattern」繞過，治標不治本。

### 必須 canonicalize-then-contain，禁止 ban-absolute
containment 一律先 canonicalize（相對 path 對 vault root 解析、正規化 symlink/大小寫）再比 prefix，不可用「禁絕對路徑」當 vault 外判據。
理由：fix verb 的 SKILL 指示 agent 用 lint JSON `issues[].path` 的絕對路徑 verbatim 做 Read/Write/Edit，這些絕對路徑指向 vault 內 wiki 檔；ban-absolute 會擋死整個 fix Read 鏈。
Alternative（rejected）：沿用 goal/query SKILL 的「no absolute paths」soft 規則當硬 gate — 與 fix 的絕對路徑契約直接衝突。

### 獨立 config key `read_path_containment`（預設 true），與 read_image_block 分離
containment 由新 key `hooks.read_path_containment` gate（預設 true、fail-safe）；既有 `hooks.read_image_block` 繼續只 gate denylist。
理由：containment 是安全邊界、不該被「image PII 便利開關」一起關掉（共用會讓 `read_image_block:false` 重新打開破口）；但 Windows canonicalize 邊界有誤擋風險，需保留 emergency escape hatch，故不採 unconditional。
Alternatives（rejected）：共用 `read_image_block`（false 會關掉邊界、不安全）；unconditional 無 key（canonicalize bug 時無法救、operability 差）。
連動：既有 scenario「read_image_block:false allows all reads」需更新為「跳過 denylist，但 containment 仍依 read_path_containment 生效」。

### Glob/Grep 覆蓋走 REQUIRED_HOOKS 加 matcher（單一真相）
在 `REQUIRED_HOOKS` 新增 Glob、Grep 兩個 RequiredHook（→ `codebus hook check-read`）；DEFAULT_SETTINGS_JSON、drift-guard 測試、`vault-gate-integrity` rule 自動連動。同時 `ToolInput` 增 `path` 欄位，check_read 取 `file_path`（Read）或 `path`（Glob/Grep）任一。
理由：REQUIRED_HOOKS 是 settings 內容、lint rule、測試的單一真相；從此處改，vault-gate-integrity 自動 enforce 新 vault 並 flag 舊 vault。

### vault root 來源：PreToolUse stdin `cwd`（首選）、hook 子程序 cwd（備援）— 2026-06-04 實機驗證
containment 需 vault root。**已實機 spike 驗證**（throwaway PreToolUse hook dump stdin+cwd）：PreToolUse stdin JSON **確帶 `cwd` 欄位**＝vault root（`D:\side_project\codebus\.codebus`），且 hook 子程序自身 cwd **亦＝vault root**；stdin 同時帶 `tool_name` 與 `tool_input`（Read 的 `file_path`、Grep 的 `path` 皆逐字到位）。裁決：**首選讀 stdin `cwd`，stdin 缺 `cwd` 時備援子程序 `std::env::current_dir()`**，不引入新 persistent config 欄位。現行 check_read 只用 `default_config_path()` 取 home，本 change 另從 stdin/cwd 取 vault root。

## Implementation Contract

In scope：claude-path check-read 的 path 邊界 gate + Glob/Grep 覆蓋 + 新 config key `read_path_containment` + vault-gate-integrity 連動 + migration 偵測。
Out of scope：codex-path、Write/Edit 邊界、自動改寫既有 vault settings.json。

Observable behavior（apply 完成可驗）：
- check-read 收到 Read 的 `tool_input.file_path` 或 Glob/Grep 的 `tool_input.path`：當 `read_path_containment` 解析為 true 且該 path canonicalize 後不在 vault root 內 → exit 0 + stdout `{"decision":"block","reason":<提及 vault containment 與該 path>}`；在 vault root 內 → 續走既有 denylist（受 read_image_block gate）。
- path 欄位皆缺（Grep/Glob 省略 path = 隱含 cwd = vault root）→ 視為 in-vault、不因缺 path 而 fail-closed block。
- `codebus init` 對 fresh vault 寫的 settings.json `hooks.PreToolUse` 含 Bash、Read、Glob、Grep 四個 matcher（Glob/Grep route 到 check-read）。
- `codebus lint` 對缺任一 required hook 的 vault emit `vault-gate-integrity` error（每缺一個一條）。
- fix verb 用 lint 絕對路徑 Read vault 內 wiki 檔 → 通過（canonicalize 後在 vault 內）。
- `read_path_containment: false` → 跳過 containment（denylist 仍依 read_image_block）。

驗證目標：hook 單元測試（containment in/out、Glob/Grep path 欄位、省略 path、Windows canonicalize 邊界、fix 絕對路徑 allow）；`codebus-cli/tests/lint_flow.rs` 的 vault-gate-integrity 缺 Glob/Grep 場景；settings.rs drift-guard 測試更新為四 hook；一次真實 codebus query + fix 在本 repo 自身 `.codebus` vault 的 live smoke。

## Risks / Trade-offs

- [Grep/Glob 省略 path 被 fail-closed 誤擋、打死正常 Grep] → 明確把「缺 path = cwd = in-vault = allow」寫進契約與測試，不走 fail-closed。
- [Windows canonicalize 邊界（`\?\` 前綴、磁碟機代號大小寫、8.3 短檔名、UNC）使 vault 內檔被判 vault 外、誤擋正常功能] → vault root 與 target 一律經同一套 canonicalize 再比；補 Windows 案例測試。
- [相對 path 未對 vault root 解析 → 誤擋] → 相對 path 先 join(vault_root) 再 canonicalize。
- [symlink/junction：vault 內 symlink 指外或 repo 位於 symlink 路徑] → canonicalize 兩端；spike 驗 raw/code（fs::copy 非 symlink）+ wiki + symlinked vault 不被誤擋。
- [fix verb 絕對路徑被擋、fix 失效] → 鐵則 canonicalize-then-contain；fix 絕對路徑 Read 設為必測 allow 案例。
- [per-call process spawn latency：Glob/Grep 每次 +1 個 check-read 子程序、goal/query 大量用變慢] → 接受為安全成本；check_read 維持輕量純 path 比對；若成瓶頸再評估批次或常駐方案。
- [既有 vault 不自動升級、破口續存] → vault-gate-integrity flag + release note 引導；本 repo 自身 vault 一併補。

## Migration Plan

- 新 vault：init 直接寫四 matcher。
- 既有 vault（含本 repo 的 `.codebus/.claude/settings.json`，現只有 Bash/Read）：write-if-missing 不覆蓋 → 跑 `codebus lint` 被 `vault-gate-integrity` flag → 依 release note 手動補 Glob/Grep matcher 片段，或於新位置 re-init。
- Rollback：`hooks.read_path_containment: false` 立即停用 containment（denylist 行為回到改前）；或 revert REQUIRED_HOOKS 變更移除 Glob/Grep matcher。

## Open Questions

- vault root 來源 — **已解決**（2026-06-04 實機 spike）：PreToolUse stdin `cwd` 與 hook 子程序 cwd 皆＝vault root；採 stdin `cwd` 首選、子程序 cwd 備援，不需新 config 欄位。
- matcher 形式：三個獨立 entry vs 單一 regex matcher `Read|Glob|Grep` — 實作期依 Claude matcher 對 regex 的支援與 settings 可讀性定。
