# Backlog: PreToolUse Read hook 擋圖片 / binary 檔案

**Date:** 2026-05-20
**Surfaced during:** 2026-05-20 roadmap review，user 詢問現有 hook 行為，發現 image bypass PII filter
**Severity:** PII safety gap（隱性洩漏路徑）
**Owner:** harry
**Status:** deferred 2026-05-25 — 原 framing 過時（claude-only 假設、現需 multi-provider）；2026-05-25 codex hook spike 確認 codex 沒便宜的 project-local hook 路徑；re-framed proposal 見尾段「2026-05-25 update」

---

## 觀察

現有 `.codebus/.claude/settings.json` 的 PreToolUse hook 只有一條 `matcher: "Bash"` rule（`codebus-core/src/vault/settings.rs:31-46`），唯一邏輯是 `codebus hook check-bash` 把 Bash command argv 過白名單（只放行 `codebus lint *` / `codebus quiz validate *`）。

`Read` / `Glob` / `Grep` / `Write` / `Edit` 全部**沒有** PreToolUse hook。Claude Code 的 `Read` tool 可以讀圖片（PNG / JPG / PDF / GIF / WebP / BMP / TIFF），binary 內容會以 visual context 餵給 model。

### PII 洩漏路徑

raw_sync 的 `regex_basic` PII scanner（`codebus-core/src/pii/scanners/regex_basic.rs`）只掃**文字內容**：

| 資料 | PII 掃過嗎 |
|---|---|
| source file 文字內容（mirror 進 `raw/code/`） | ✓ |
| 圖片走 Read tool 進 agent context | **✗ 完全 bypass** |
| PDF 內含文字 + 圖片 | **✗** |

實際 risk：

- repo 內若放 screenshot（含 credentials UI / 內網 dashboard / 個人臉孔 / 第三方授權頁面）→ agent 可讀
- agent 把對圖片的觀察寫進 wiki → PII 洩到 wiki
- 跟 codebus 「PII-sanitized wiki」核心保證直接對撞

跟 [git-context-tool](2026-05-14-git-context-tool-backlog.md)（已結案不做）的差別：git context 是「資料來源在 git metadata」可以靠不複製 `.git` 防護；圖片是「資料來源就在 repo 內」，且 `Read` 是核心 tool 不能拿掉，**只有 hook 能擋**。

## Proposed fix

`.codebus/.claude/settings.json` 加第二條 PreToolUse rule，matcher = `Read`，routing 到新子命令 `codebus hook check-read`：

```json
{
  "hooks": {
    "PreToolUse": [
      { "matcher": "Bash", "hooks": [{ "type": "command", "command": "codebus hook check-bash" }] },
      { "matcher": "Read", "hooks": [{ "type": "command", "command": "codebus hook check-read" }] }
    ]
  }
}
```

### Block 規則

`check-read` 讀 PreToolUse JSON，抽 `tool_input.file_path`，依**副檔名**（case-insensitive）命中即 `emit_block`：

```
.png .jpg .jpeg .gif .webp .bmp .tiff .tif
.pdf .ico .heic .heif .avif
```

設計選擇：

- **副檔名匹配**（而非 magic bytes / mime sniff）：codebus 環境下副檔名可信，user 自己 vault；做 magic bytes 是 over-engineering
- **白名單反向比較不做**：副檔名宇宙太雜（`.md/.rs/.py/.ts/.json/.yaml/...`），維護成本高
- **SVG 不擋**：是純文字 XML，PII scanner 可走（雖然可內嵌 base64 image，但需要先驗證真有 case 再加）

### 實作模式

複用 `codebus-cli/src/commands/hook.rs` 既有骨架：
- `emit_block` / `json_escape` / `PreToolUseInput` struct 直接複用
- 新加 `is_image_path(path: &str) -> bool` predicate
- `HookArgs` enum 加 `CheckRead` variant
- fail-closed default 沿用（讀不到 path、parse 失敗等 → block）

## Tasks（粗估）

1. `codebus-cli/src/commands/hook.rs` 加 `CheckRead` variant + `is_image_path` predicate
2. `codebus-core/src/vault/settings.rs` `DEFAULT_SETTINGS_JSON` 加第二條 Read matcher
3. settings.rs 既存測試補：第二條 hook 結構正確
4. hook.rs unit tests：副檔名命中 / 大小寫 / 路徑分隔符 / 空 path / 缺 file_path field 都正確 block
5. integration test：vault init 後 settings.json 含兩條 PreToolUse rule
6. existing user 升級路徑：write-if-missing 不動，已 init 的 vault 需 re-init 或手動補（在 release notes / wiki 提醒）

工程量：輕（半天）。

## Out of scope

- magic bytes / mime sniff（副檔名足夠）
- SVG 內嵌 base64 image 偵測（等真實 case 出現再加）
- 對外部上傳 / 貼上的圖片（codebus 沒這通道）
- 把現有 vault 自動 retro-fit 補 Read hook（避免覆寫 user 客製化；用 release note 引導手動處理或 re-init）
- audio / video 檔案（同樣 binary 但更冷門，需要再列再評）

## 升級路徑（important）

`write_settings_if_missing` 對既有 vault 不會覆寫。已 init 的 vault 升級需要：

- **方案 A**（推薦）：release notes 列出需手動加第二條 hook 的 JSON snippet
- **方案 B**：加 `codebus init --migrate-hooks` 子旗標，diff 合併 settings.json（破壞既有 byte-identical 保證，需另設計）

建議方案 A，工程量輕、不破壞既有契約。

## 何時動

可獨立、無依賴。建議在 `v3-app-polish-ship` 之前順手做 —— release 帶這個 hardening 出去比 release 後補強好。

---

## 2026-05-25 update — multi-provider 後重新框架 + codex hook spike

### 為什麼原 framing 過時

原 proposal（2026-05-20）寫於 codex provider 還沒 land 前，整段邏輯假設「codebus 只有 claude provider、所有 enforcement 都靠 `.claude/settings.json` 的 PreToolUse hook」。

2026-05-23 後 codex provider 完整 land（`codex-skill-trigger-fix` 2026-05-25 收尾、commit `a4a931b` 進 main、5/5 verb × codex × Windows 全綠）。現在 multi-provider 架構下，「只擋 claude Read」會留下 codex 端的對等 PII 洩漏路徑。原 proposal 三條 mismatch：

1. **PreToolUse 是 Claude Code-specific behavior** — codex CLI 沙箱（`-s read-only/workspace-write/danger-full-access`）是黑白制、CLI flag 沒等價 per-tool hook
2. **沒 config toggle** — 原提案是「always-on hard block」、未保留 user opt-in/opt-out 控制權
3. **abs path read 仍洩** — 即使 claude PreToolUse 攔 `Read tool_input.file_path`，agent 可繞用 absolute path 從 vault 外讀檔；codex 同；只有 filesystem 層擋圖才真的 provider-agnostic

### Codex hook spike 結果（2026-05-25）

為了驗證 codex 是不是真有 user-accessible PreToolUse 機制，跑了 7 組實驗。

**確認的事**（codex binary strings + figma plugin 範例）：

- codex binary 內建完整 hook 系統：`PreToolUse` / `PostToolUse` / `PermissionRequest` / `PreCompact` / `PostCompact` / `SessionStart` / `UserPromptSubmit` / `SubagentStart` / `SubagentStop` 事件種類；`HookHandlerConfig::Command` / `::Agent` / `::Prompt` 三種 handler 型別；wire types 含 `PreToolUseHookSpecificOutputWire` / `PreToolUsePermissionDecisionWire`；`Tool call blocked by PreToolUse hook: <reason>` 訊息 template 都在
- `--dangerously-bypass-hook-trust` flag 被 codex 識別（emit `--dangerously-bypass-hook-trust is enabled. Enabled hooks may run without review for this invocation.`）
- Plugin 內部 `<plugin-dir>/hooks.json` + `<plugin-dir>/plugin.json` reference 是已知工作的 hook 註冊路徑（user 的 `~/.codex/.tmp/plugins/plugins/figma/hooks.json` 真實樣本驗證 schema 跟 Claude Code 幾乎相同）

**spike 試了 6 種 config 路徑都沒讓 hook 真實 fire**（hook script 是 `.cmd` 寫 marker 到 trace file、exit 2、stderr 帶 reason —— 跑得起來就應該寫 trace file）：

| 嘗試 | 結果 |
|---|---|
| `<vault>/.codex/hooks.json` (claude-analog 路徑) | ✗ 沒 fire |
| `<vault>/.codex/config.toml` 內含 `[[hooks.PreToolUse]]` (project layer) | ✗ 沒 fire |
| `~/.codex/config.toml` 內含 `[[hooks.PreToolUse]]` (user layer) | ✗ 沒 fire（test 7 codex 卡住、kill 後 log 空）|
| `--dangerously-bypass-hook-trust` 開啟 | flag 被識別但 hook 仍不 fire |
| `--ignore-rules` 拿掉 vs 保留 | 兩種都沒 fire |
| catch-all matcher `.*` vs specific matcher | 兩種都沒 fire |

**推斷**：codex 的 hook system 設計給 plugin 用、不給 end-user 在 `config.toml` 寫。`developers.openai.com/codex` 的 public docs 也沒 hook schema 文件（只有 plugin 開發者文件）。要讓 codebus 走 codex hook 路、唯一方式是**把 codebus 包成 codex plugin**（plugin.json + hooks.json + marketplace register 或本機 plugin 安裝）—— 但這是 scope 巨大的整合、不該 piggyback 在 PII safety 這條 backlog 上。

完整 spike logs 與每測試 reproducer 在 chat history（2026-05-25 session），本檔做為日後接手的決策紀錄。

### Re-framed proposal（2026-05-25 起的真實設計）

PII binary-block 改用**雙層架構**，第一層 provider-agnostic、第二層 provider-specific defense in depth：

```yaml
# ~/.codebus/config.yaml 新增
pii:
  block_binary_reads: true   # default on（安全預設）
```

| 層 | 機制 | 適用 provider |
|---|---|---|
| 1（主防線）| `raw_sync` mirror **跳過** `.png .jpg .jpeg .gif .webp .bmp .tiff .tif .pdf .ico .heic .heif .avif .mp3 .mp4 .mov .wav` 等 binary file | provider-agnostic — vault `raw/code/` 不含 binary、agent relative-path Read 自然 ENOENT；claude / codex 都受惠 |
| 2（defense in depth, claude only）| `<vault>/.codebus/.claude/settings.json` 補第二條 PreToolUse rule，matcher = `Read`，routing 到新 subcommand `codebus hook check-read` 走副檔名 block list | claude only — codex 端無對等公開機制（codex 端只能靠 `AGENTS.md` 寫 prompt 紀律「don't Read .png/.jpg/...」、屬軟性 instruction、非 hard gate）|

**`block_binary_reads=false`**：兩層全關 —— `raw_sync` 維持原 mirror 行為、`.claude/settings.json` 不加第二條 hook、codex 端 AGENTS.md 不加 prompt instruction。Power user 用。

**Abs path read 仍是已知 gap**：兩層都不擋 agent 用 abs path 從 vault 外讀檔。Claude PreToolUse `tool_input.file_path` 看任何路徑都會擋（這層仍對 claude 有效），但 codex 端沒這層。short-term 接受、long-term 評估 codex plugin 包裝路徑。

### 範圍 + 工程量重估

| 工程項 | 工作量 |
|---|---|
| `raw_sync` 加 binary skip predicate + config gate | 小（1 個 fn + 既有 sync loop 對接）|
| `pii.block_binary_reads` config 欄位 + default | 小 |
| `codebus-cli/src/commands/hook.rs` 加 `CheckRead` variant + `is_image_path` predicate | 小（原 proposal 已估）|
| `codebus-core/src/vault/settings.rs` `DEFAULT_SETTINGS_JSON` 加第二條 Read matcher（gated by config） | 小 |
| codex 端 `AGENTS.md` template 加 binary-read prompt instruction | 小 |
| 既有 vault migration（write-if-missing 不動、release notes 引導）| 小 |
| spec deltas：`pii` MODIFIED + `skill-bundles` MODIFIED（或新 capability `pii-binary-block`）| 中 |
| 跨 provider 5 verb 重跑驗證 + 故意放 PNG 看 raw/code 真的沒鏡像 | 中 |

從原估「半天」變「1-2 天」。沒爆但翻倍。

### Follow-up（不在本 backlog 範圍）

- **codex plugin 包裝**：把 codebus 註冊為 codex plugin、補上 codex 端真實 PreToolUse hook enforcement。Spike 工作量大、收益是 claude/codex 對等 defense-in-depth。獨立 backlog 評估
- **abs path read**：兩 provider 都有的 gap。需要 sandbox 級限制（filesystem ACL / 不同 user account / container），超出本 change 範圍
- **PDF 內含文字的 OCR 路徑**：本 change 連 PDF binary 也擋掉，所以連帶擋文字 PDF。如果未來想恢復「PDF 文字 PII-scan 後讓 agent 讀」，需 PDF-to-text 預處理層（另一個大 scope）

### 結論

- 原 2026-05-20 proposal 還是**部分正確**（claude PreToolUse hook 的設計）— 保留進新方案作為層 2 defense in depth
- 真正的 PII gap 修法在**層 1 raw_sync skip binary**（原 proposal 沒考慮的角度）
- codex 端短期內**接受是 prompt 紀律保護**、不追究 hook 等價 enforcement
- 整段工作 defer 到 user 有時間 + scope 主動拉起時動；不阻塞 codex provider work（已完成）也不阻塞 v3-app-polish-ship
