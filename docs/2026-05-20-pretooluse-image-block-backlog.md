# Backlog: PreToolUse Read hook 擋圖片 / binary 檔案

**Date:** 2026-05-20
**Surfaced during:** 2026-05-20 roadmap review，user 詢問現有 hook 行為，發現 image bypass PII filter
**Severity:** PII safety gap（隱性洩漏路徑）
**Owner:** harry
**Status:** open

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
