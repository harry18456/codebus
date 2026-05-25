## Context

2026-05-24 實機跑 5 verb × codex provider 矩陣（commit `10de31d` 之後）：codex 0.133.0 上 codebus 的 5 verb 0/5 完整 work、claude 5/5 work。共通症狀是 codex agent 收到 `$codebus-<verb> <args>` 後沒進入 SKILL Mode：

- quiz：plan spawn 沒 emit `[CODEBUS_QUIZ_SCOPE]` marker，agent 走「I'm treating this as a planning task for the codebus-quiz area」分析模式
- goal：沒 Write 任何 wiki page，agent 走分析摘要模式
- query：完全沒讀 vault，用 generic 知識答
- chat：讀了 vault，但 emit「I found this is a documentation vault rather than application source」meta-comment
- fix：識別了 broken wikilink 但拒絕修（"this session is running with a read-only filesystem sandbox"）— B cluster，獨立 bug，本 change 不處理

2026-05-22 codex 0.132.0 spike 端到端 work（per memory `project_multi_provider_driver_confirmed`），2026-05-24 / 2026-05-25 重跑 codex 0.133.0 全 broken — 強 hint 是 codex CLI 版本回歸，但需 diagnose 確認，不可預先承諾修法形狀。

當前 codex provider 的 SKILL invocation 機制（per memory `project_codex_skill_invocation_mechanism`）：

- codebus 透過 SpawnSpec 的 prompt 欄位傳入 `$codebus-<verb> ...` 字串（codex native invocation）
- codex 端要在 prompt 解析 `$`-prefix、在 `.codex/skills/codebus-<verb>/SKILL.md` 查 bundle、進 SKILL Mode
- fallback 機制：description-match 走 `/codebus-<verb>`（token +25%，但通用）

相關 codebase 位置（locator 用途，contract 在 Implementation Contract 段）：

- `codebus-core/src/agent/codex_backend.rs`：argv 與 prompt 組裝
- `codebus-core/src/vault/init/skills/codex_*.rs`：codex SKILL bundle 內容寫入
- `<vault>/.codebus/.codex/skills/codebus-<verb>/SKILL.md`：vault-side bundle

## Goals / Non-Goals

**Goals:**

- 找到 codex 0.133.0 上 SKILL trigger 失靈的 root cause（diagnose 三層觀察落紙）
- 套最小修使 codex 5 verb 全部至少進得了 SKILL Mode（quiz emit marker、goal/query 進 vault workflow、chat 不 emit meta-comment）
- diagnose 結果與選用修法理由收斂於 `docs/2026-05-25-codex-skill-trigger-diagnose.md`，使後續 codex 版本 bump 可回溯
- 5 verb × codex provider 重跑驗證，行為對等 claude path（fix 例外：B cluster 仍會撞 sandbox-write）

**Non-Goals:**

- 不動 claude path / 不重構共用 SpawnSpec 或 verb library
- 不引入 multi-provider 抽象層（per memory `feedback_dont_speculative_abstract`，single-impl trait 不寫）
- 不嘗試讓 codex provider 在所有 verb 上 100% 等價 claude（grounded behavior 差異交給後續觀察，本 change 只保證「進得了 SKILL Mode + 可寫 vault」）
- 不涉 Phase 5 spike（per memory todo P3，前提不成立）
- **macOS / Linux sandbox-write 行為不在本 change 驗證範圍**：`-c windows.sandbox=unelevated` 是 Windows-only override（codex `[windows]` table key）；macOS（seatbelt）/ Linux（Landlock）用各自原生 sandbox 後端、可能需要不同 key 或本來就 work。本 change 在 Windows 上驗 5/5 verb work、其他平台 deferred

## Decisions

### Diagnose 走三層觀察、找到根因即停

按以下順序執行，任一層找到 confirmed root cause 即停、進入修法決策：

1. **codex CLI 版本對照（最快、訊號最強）**：在 isolated 環境（npx pin 0.132.0 或臨時改 PATH）安裝 codex 0.132.0，跑同 reproducer（`codebus quiz "JWT issuance and verification" --count 3` 於 `/tmp/exp-vault`），看是否 emit `[CODEBUS_QUIZ_SCOPE]`。若 0.132 work、0.133 broken → 確認是 codex CLI 0.132 → 0.133 regression，根因不在 codebus；若 0.132 也 broken → 不是 CLI regression，繼續層 (b)。
2. **argv 攔截（驗證 codebus 端輸出正確）**：在 `CODEBUS_CODEX_BIN` 環境變數指向 shim binary（小 PowerShell / Rust binary，stdout 輸出收到的全部 argv + stdin，exit 0），跑 reproducer，dump 收到的 argv 與 prompt 字串。確認：(i) `$codebus-quiz` sigil 沒被 shell escaping 改壞、(ii) prompt 不含異常前綴後綴、(iii) `--ignore-user-config / --disable apps / --ignore-rules / -c project_root_markers=...` 等隔離 flag 都還在。若 argv 拼接正確 → 排除 codebus 端 → 繼續層 (c)；若 argv 異常 → root cause 在 codebus，進修法 (3)。
3. **codex stream 觀察（看 codex 端怎麼解析）**：直接呼叫 `codex exec --json --ignore-user-config ... "$codebus-quiz plan: ..."`（複製 codebus 攔到的 argv），檢視 codex 自己 emit 的 stream events 是否有 `skill_invocation` / `skill_loaded` / `skill_not_found` 之類事件、或 plain text reasoning。若 codex 完全沒處理 `$`-prefix → CLI 端 SKILL invocation 機制 broken / 改了；若 codex emit skill_not_found → SKILL bundle 路徑或 frontmatter 不符。

**為什麼這順序？** (a) 觀察成本最低（裝個版本跑指令），訊號最強（version-level black-box bisect）。(b) 在 codebus 邊界釐清責任分界。(c) 進 codex CLI 內部，最費力。Grounded debugging（per memory `feedback_grounded_debugging`）：不連續猜三層、每一層都拿到 concrete observation 才繼續。

**為什麼不直接看 codex source？** codex CLI 是 closed-binary npm package（`C:\Users\harry\AppData\Roaming\npm\codex`），source 不在手邊；diagnose 走黑箱觀察是必要路徑（per memory `feedback_check_docs_not_only_blackbox`，先查官方 release notes 拿來輔助黑箱推理）。

### 修法選擇依 diagnose 結果擇一，不預先承諾

| Diagnose 結論 | 修法路徑 | 影響檔案（locator） |
|---|---|---|
| codex 0.132 work、0.133 broken（CLI regression）| 切換 codex provider 用 `/codebus-<verb>` description-match invocation。在 prompt 組裝層改 sigil（`$` → `/`），不動 SKILL.md 內容（bundle 仍是 codex-native 格式）。 | `codebus-core/src/agent/codex_backend.rs` 或 prompt 組裝層 |
| 0.132 也 broken（不是 CLI regression）+ argv 異常 | 修 codebus 端 argv / prompt 拼接 bug | `codebus-core/src/agent/codex_backend.rs::build_command` |
| 0.132 也 broken + argv 正確 + codex emit skill_not_found | 修 SKILL bundle 內容（frontmatter / body 結構符合 codex 0.133 新預期）| `codebus-core/src/vault/init/skills/codex_<verb>.rs` × 5 verb |
| 0.132 也 broken + argv 正確 + codex 完全沒處理 `$`-prefix | 等同 CLI regression — 改 description-match invocation | 同表格第一列 |

**為什麼 description-match `/codebus-<verb>` 是合理 fallback？** 已驗證機制（per memory `project_codex_skill_invocation_mechanism`），token 成本只增加 25%，且通用（不依賴 codex 的 `$` native sigil 解析）。代價可承受。

### 實際 diagnose 結論與修法（2026-05-25 完成）

兩個 root cause 都不在原候選表內、且都坐落於 init / spawn 的 isolation recipe 層：

**A cluster — SKILL trigger（init 端 codex 材料缺檔）**

- 證據：層 (b) shim dump 顯示 codebus argv 與 prompt 全正確、`$codebus-quiz` sigil 完整；filesystem 檢查發現 `/tmp/exp-vault/.codebus/.codex/skills/` 整段不存在；重跑 `codebus init` 補檔後 quiz 立刻 work
- Root cause：`codebus-core/src/vault/init.rs::init_vault` 用 `if codex_provider_active()` gate 守 codex 材料 materialization。Gate 在「用戶 init 時 active_provider=claude、後切 codex」場景失效
- 修法：移除 gate、改為無條件 write-if-missing。`codex_provider_active()` helper 一併刪（dead code）

**B cluster — codex sandbox 實際不可寫（`--ignore-user-config` 副作用）**

- 證據：層 (b) shim dump 顯示 `-s workspace-write` 正確被傳；user 在獨立 codex shell + bisect 驗證 `--ignore-user-config` 是元兇（拿掉就能寫、保留則 sandbox 退回 read-only）；user `~/.codex/config.toml` 內有 `[windows] sandbox = "elevated"` 一行 — 對映 codex 0.133.0 的 `windows.sandbox` 配置（codex doctor 顯示僅接受 `elevated` / `unelevated` 兩值）
- Root cause：`codebus-core/src/agent/codex_backend.rs::build_command` 的 isolation recipe 用 `--ignore-user-config` 擋 user config（合理 — 避免 MCP / personality / plugins 滲入），但這也擋掉 sandbox-write 啟用所需的 `windows.sandbox` 預設值
- 修法：保留 `--ignore-user-config`，補一條 `-c windows.sandbox=unelevated`。值選 `unelevated` 而非 user 的 `elevated`：實測 `elevated` 雖能寫檔但會阻擋 codex 的 `Shell` tool 啟動 subprocess（觀察到 `windows sandbox: spawn setup refresh` error），因為 codex 試圖以 admin 身分 spawn child；codebus 一般 user 跑、`unelevated` 反而讓 sandbox 以當前用戶權限運作、同時允許 file write 與 shell spawn（兩者都驗過 — 寫 `note.md` + 跑 `Get-Date`）。實測（option K + L bisect）證實 node_repl MCP 仍沒載入、sandbox-write 啟用、vault workdir 真實可寫、PowerShell shell 可 spawn

### `-c windows.sandbox=unelevated` 是對的 trade-off

候選修法比較：

| 修法 | sandbox-write 可寫 | MCP 擋住 | 風險 |
|---|---|---|---|
| 拿掉 `--ignore-user-config` | ✓ | ✗ (node_repl 載入) | personality / plugins 滲入 agent 行為 |
| `-c mcp_servers={}` 覆蓋 + 拿掉 `--ignore-user-config` | ✓ | ✗ (測試顯示 node_repl 仍載入) | 同上 |
| `--dangerously-bypass-approvals-and-sandbox` | ✓ | ✓ | sandbox 整段關掉、agent 可寫 vault 外 |
| `CODEX_HOME=isolated-dir + auth.json` | ✗ | ✓ | sandbox 退回 read-only |
| **保留 `--ignore-user-config` + `-c windows.sandbox=unelevated`** | ✓ | ✓ | Windows-only key、其他平台需驗 |

選最後一列：所有目標都達成、唯一代價是跨平台需 follow-up。

**Windows-only 是已知 scope limitation**：codex `[windows] sandbox = unelevated` 是 Windows table key；macOS / Linux codex 用各自 sandbox 後端（seatbelt / Landlock），可能本來就 work、也可能需要對等 override（如 `[macos] sandbox=...`）。本 change 在 Windows 上驗 5 verb 全綠後 ship、跨平台驗證列 follow-up（per memory `feedback_dont_default_polish_ship`，solo dev + 主要在 Windows 開發，不視為 ship-blocker）。

### C cluster — codex verify-stage spawn 撞 batch-file argv 限制（多行 prompt 走 stdin）

**症狀**：A + B 修完跑 codebus quiz / goal，main spawn 全 work，但 content-verify 子 spawn 一律印「`warning: ... content-verify spawn failed (non-fatal; content_review: flagged): spawn agent: batch file arguments are invalid`」，verify 結果回 `flagged 0 page(s)` 是空集（沒 verify、不是 verified clean）。

**證據**：
- 用最小 Rust repro 確認 `Command::new("codex.cmd").arg(s)` 在 Rust 1.95（>1.77 hardening 之後）拒任何含 `\n` 的 arg，回 `InvalidInput: batch file arguments are invalid`；同樣 string 無 `\n` 則 exit 0
- codex npm 在 Windows 是 `codex.cmd` shim（PowerShell + Get-Content 看內容是 `node bin/codex.js %*`）。real `.exe` 在 `node_modules/.../bin/codex.exe`，路徑 brittle
- verify spawn 的 input 形狀：`goal=<task>\n\nCHANGED PAGES:\n<paths>` 或 `goal=<task>\n\nCONTENT DEFECTS:\n<defects>\n\nFLAGGED PAGES:\n<pages>`，組合進 `$codebus-<verb> verify: ...` 就含 `\n`
- main goal / quiz / chat / query / fix spawn 的 prompt 都是單行（`$codebus-quiz plan: API rate limits` 等），所以只有 verify / repair sub_mode 中招

**Root cause**：Rust stdlib 對 `.cmd` shim 的 argv 驗證（1.77+ hardening），不是 codebus 也不是 codex bug — 但 codex 在 Windows 的 npm 安裝形狀觸發到這條 hardening。

**修法評估**：

| 候選 | 可行 | 代價 |
|---|---|---|
| 1. `raw_arg` 繞過 Rust 驗證 | ✗ — 實測 cmd.exe / npm shim 把多行 arg mangle 掉、agent 只收到 first line + last line | — |
| 2. 解析 `codex.cmd` 找真實 `.exe` 跳過 shim | △ — 路徑深、brittle、隨 codex npm 版本變 | 維護成本 |
| 3. encode `\n` 為 sentinel + SKILL body unescape | ✗ — 改 SKILL body + 與 user input 衝突風險 | 漏 sentinel 邊界 |
| 4. 寫 prompt 到 temp file + 傳 file path arg | △ — I/O 依賴、cleanup、secrets-on-disk | 額外 race |
| 5. 用 `--dangerously-bypass-approvals-and-sandbox` 換條路 | ✗ — sandbox 整段關、與 B 修法矛盾 | sandbox 失效 |
| **6. 用 codex `-` prompt arg + stdin pipe 餵 prompt** | ✓ — 實測 multi-line prompt 完整 round-trip（agent 收到全部 lines）；codex exec 原生支援 `-` 讀 stdin | 1 個 optional trait method |

選 6。代價是 `AgentBackend` trait 多一個 optional method `stdin_payload(&SpawnSpec) -> Option<String>`（default `None`），claude 不動、codex 在 prompt 多行時 opt-in。這需要 modify `agent-backend` spec（archive 寫 「exactly three methods」），改為「3 required + optional with safe defaults」— 不是 speculative abstraction（per memory `feedback_dont_speculative_abstract`），是 concrete 跨 backend variation（claude 不需、codex 需）必要的接口擴充。

**Implementation pieces**：

| 檔案 | 修改 |
|---|---|
| `codebus-core/src/agent/backend.rs` | trait 加 optional `stdin_payload` method with default `None` body |
| `codebus-core/src/agent/codex_backend.rs` | 新增 free fn `format_codex_prompt(spec)` shared by `build_command` 與 `stdin_payload`（兩條 path 不能 drift）；`build_command` 對多行 prompt pass `-` argv；`stdin_payload` 對多行回 `Some(formatted_prompt)` 否則 `None` |
| `codebus-core/src/agent/claude_cli.rs` (`invoke`) | 讀 backend stdin_payload；`Some` → `Stdio::piped` + write_all + drop stdin；`None` → 原 `Stdio::null` |
| `codebus-core/src/agent/claude_backend.rs` | 不動（用 trait default） |

### 不為 multi-impl 預留抽象層

A 修法是「移 init gate」、B 修法是「argv 加一行」、C 修法是「加一個 optional trait method」。**新增 trait method 是否違反 Non-Goal？** Non-Goal 寫的是「不為 multi-impl 預留抽象層」— 重點是「speculative future」。本 case 是 concrete cross-backend variation（Windows codex 走 `.cmd` shim + Rust 1.77+ argv validation 是真實限制、不是假設情境；claude 在 Windows 走 `.exe` 沒這問題），所以 optional method 是 reflecting reality not future-proofing。Per memory `feedback_engineer_best_not_easiest`：選工程最正確解；其他繞路（raw_arg / shim bypass / sentinel encoding）都更脆弱。

agent-backend archive spec 的「exactly three methods」要 MODIFY 成「3 required + optional with safe defaults」，這由本 change 的 specs/agent-backend delta 處理；既有實作（claude）行為 0 影響。

### Diagnose 觀察必須寫成 doc 而非僅在 commit message

`docs/2026-05-25-codex-skill-trigger-diagnose.md` 記錄三層觀察結果與選用修法理由，使 codex 後續版本（0.134+）若再回歸時，可快速比對「上次 0.132→0.133 regression 是哪一層、是怎麼修的」。Per memory `feedback_grounded_debugging` 與 `feedback_check_docs_not_only_blackbox`，diagnose 落紙是必要環節。

## Implementation Contract

**Observable behavior（修完後）：**

- 在 active_provider=codex 的 `/tmp/exp-vault`（或等價乾淨 vault）上，跑 `codebus quiz "<topic>" --count 3` SHALL 在 plan spawn 第一行（first stream line）emit `[CODEBUS_QUIZ_SCOPE]` 或 `[CODEBUS_QUIZ_NO_MATCH]` marker 之一。
- 在同 vault 上，跑 `codebus goal "<task>"` SHALL 觀察到 agent Write 至少一個 `.codebus/wiki/**/*.md` page（B cluster 修完後寫權限已解、不再 carve-out）。
- 在同 vault 上，跑 `codebus query "<question>"` SHALL 觀察到 agent 讀取 `.codebus/wiki/` 下的 markdown file（透過 Read/Glob/Grep tool call 可見），不再走 generic 知識回答。
- 在同 vault 上，跑 `codebus chat`（stdin 餵單發問）SHALL NOT emit 形如「I found this is a documentation vault rather than application source」的 meta-comment（agent 進 SKILL Mode 後該行為由 SKILL body 抑制）。
- 在同 vault 製造一個 broken wikilink 後跑 `codebus fix` SHALL 觀察到 agent 進 codebus-fix SKILL workflow（第一個 tool-call 直指 lint warning 對應頁面）並實際 edit 修掉 lint warning，跑完 `codebus lint` 該 warning 不再存在。

**Interface / data shape：**

- A 修法：`codebus-core/src/vault/init.rs::init_vault` 中 codex materialization 的 if-gate 拿掉、改無條件 `write_codex_materialization_if_missing(...)`；helper `codex_provider_active` 一併刪除。
- B 修法：`codebus-core/src/agent/codex_backend.rs::build_command` 在現有 isolation recipe argv 後追加一條 `-c windows.sandbox=unelevated`；其餘 argv（含 `--ignore-user-config / --disable apps / --ignore-rules / -s workspace-write / project_root_markers / model / azure provider`）不變。
- 不新增 trait method、不改 `AgentBackend` 介面、不引入新 SpawnSpec 欄位、不引入新 config 欄位。

**Failure modes：**

- 任一 verb scenario 5 verb 重跑後仍不滿足 Observable Behavior → 不視為成功，diagnose doc 補「殘餘症狀」段 + 視情況開後續 change。
- macOS / Linux 上若 `-c windows.sandbox=unelevated` 不生效但 sandbox-write 本就 work（不同 OS 預設不同）→ 接受；若 sandbox-write 不 work 又無對等 override → 列入 deferred follow-up change。

**Acceptance criteria：**

- `docs/2026-05-25-codex-skill-trigger-diagnose.md` 存在且包含 diagnose 三層 + K-mode bisect 的 concrete observation（reproducer command + actual output snippet），含 Root Cause 結論、選用修法理由、self-review checklist。
- 5 verb × codex 在 Windows 上重跑：goal/query/fix/chat/quiz 全部對應的 Observable Behavior 條件成立、log path 寫進 diagnose doc。
- `cargo test -p codebus-core` 全綠（含本 change 新加的 init / codex_backend argv 單元測試、現有測試無 regression）。
- `cargo build --workspace` 通過。
- analyzer 確認本 change spec deltas（skill-bundles + codex-backend）對齊 Modified Capabilities 宣告。

**Scope boundaries：**

- **In scope**：codex provider 上 SKILL trigger 進得了 SKILL Mode + vault 可寫；diagnose 落紙；skill-bundles + codex-backend spec delta；Windows 上 5 verb 重跑驗證；TDD 單元測試紅→綠。
- **Out of scope**：claude path 任何變動；codex provider 上 grounded behavior 對等 claude；Phase 5 spike；multi-provider 抽象層重構；codex 0.133 以外版本支援；macOS / Linux 上 sandbox-write 行為驗證；CI / GitHub Actions 配置變動。

## Risks / Trade-offs

- **Risk**：`-c windows.sandbox=unelevated` 是依賴 codex CLI 的 `[windows]` config table key，codex 後續版本可能改 schema → Mitigation：diagnose doc + codex-backend spec 明示這條 override 的目的（讓 sandbox-write 在 `--ignore-user-config` 下仍真實可寫），codex 版本 bump 時若行為破掉、重跑 diagnose K-mode bisect 找新 key、補 spec scenario。
- **Risk**：macOS / Linux 上 `-c windows.sandbox=...` key 不被 codex 識別、可能無聲忽略也可能噴 unknown-key warning → Mitigation：本 change Windows-only scope；spec 明示 deferred；其他平台啟用 codex 時須單獨驗證 sandbox-write、必要時補對等 override（如 `[macos] sandbox=...`）。
- **Risk**：vault init 無條件 materialize codex 材料後，純 claude 使用者的 vault 也會有 `.codex/skills/` + `AGENTS.md` + `.codebus-vault`，多耗一點點 disk 與 commit 噪音 → Mitigation：write-if-missing 保留用戶 customization、size 可忽略；comment 改寫說明設計用意（user 可後切 codex provider 不需 re-init）。
- **Risk**：spec delta 用 SHALL 描述「進 SKILL Mode + 可寫 vault」的可觀察行為，但這是 codex CLI 內部狀態、不是 codebus 可直接 assert 的 invariant → Mitigation：spec requirement 改寫成 codebus 端可觀察的代理條件（quiz emit marker、goal Write wiki page、fix 修掉 lint warning 等），spec scenario 對齊 Implementation Contract 的 Observable Behavior。

## Migration Plan

- 本 change 是純內部 fix，無 user-facing migration。對既有 vault：
  - 之前用 claude 模式 init、現切 codex provider 的 vault → 重跑一次 `codebus init <path>` 即可補 `.codex/skills/` + `AGENTS.md` + `.codebus-vault`（write-if-missing 保留用戶 customization、不破壞既有內容）。本 change 不自動 migrate 已 init vault，因 codebus 沒有 vault-discovery + bulk-migrate flow。
  - 之前用 codex 模式 init 的 vault → 無變化（材料本來就在）。
- 對 codebus 本身：本 change 不影響 build / install / 用戶資料層。`cargo install --path codebus-cli --force` 就完成 binary 升級。

## Open Questions

- macOS / Linux 上 `-c windows.sandbox=unelevated` 預期不生效（codex `[windows]` table 在非 Windows 平台是 no-op）。當這些平台啟用 codex provider 時，sandbox-write 是否需要對等 override（如 `[macos] sandbox=...` 或 `[linux] sandbox=...`）？本 change 不解、留 deferred follow-up；解法格式視 codex CLI 跨平台 config schema 而定。
- codex 後續版本（0.134+）若把 `[windows] sandbox` key 重新命名或行為改變，sandbox-write override 會破。是否需要把這條 override 變成 feature-detection 風格（先測 capability、再決定要不要傳 flag）？暫不做 — 簡單一行 hard-coded override 更易於追蹤；codex 版本 bump 時若破掉、走 diagnose K-mode bisect 找新 key、補 spec scenario。
- `codex_provider_active()` helper 刪除後，是否還有其他地方需要 active_provider 判斷？grep 證實只有 init.rs 用過、刪 ok；endpoint config 自己讀 active_provider、與此 helper 無關。
