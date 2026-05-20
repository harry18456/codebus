## Why

PreToolUse hook 目前只 gate Bash（既有 `lint-feedback-loop` 的 Fix Bash Hook Installation），對 Read tool 完全沒覆蓋。Claude Code 的 Read tool 可讀取圖片（PNG/JPG/PDF/GIF/WebP/BMP/TIFF/HEIC/AVIF 等）並把 binary 內容當 visual context 餵給 model，**完全 bypass codebus 現有的 regex_basic PII filter**（後者只掃文字）。Repo 內含 screenshot（credentials UI / 內網 dashboard / 個人臉孔）時 agent 可直接 ingest 並寫進 wiki，跟 codebus 「PII-sanitized wiki」核心保證對撞。

## What Changes

- 新增 `codebus hook check-read` internal 子命令：讀 PreToolUse JSON、抽 `tool_input.file_path`、依副檔名命中即 emit_block（fail-closed default 沿用既有 check-bash 模式）
- 更新 vault 預設 settings.json template，加第二條 PreToolUse rule，matcher = Read，hook 指向 `codebus hook check-read`
- 副檔名比較走 ASCII case-insensitive 跨平台一致（**偏離** is_codebus_binary 的 OS-split 行為，因為 Linux 上 `foo.PNG` 也是圖片）
- 既有已 init 過的 vault 升級走 release note 不自動 migrate（保留 settings 寫法既有的 write-if-missing byte-identical 契約）
- Block 副檔名清單固定：png / jpg / jpeg / gif / webp / bmp / tiff / tif / pdf / ico / heic / heif / avif
- 不擋 SVG（純文字 XML，可走 regex_basic 文字掃描）

## Non-Goals

- magic bytes / mime sniff：codebus 環境下副檔名可信，over-engineering
- 白名單反向比較：副檔名宇宙太雜（md / rs / py / ts / json / yaml / ...），維護成本高
- SVG 內嵌 base64 image 偵測：等真實 case 出現再加
- audio / video 檔案：同樣 binary 但更冷門，需要再列再評
- 把現有 vault 自動 retro-fit 補 Read hook：避免覆寫 user 客製化的 settings.json
- 加 `codebus init --migrate-hooks` 子旗標：破壞既有 byte-identical 契約

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `lint-feedback-loop`: 加新 Requirement `PII Image Read Hook Installation`，跟既有 `Fix Bash Hook Installation` 並列。既有 `Fix Bash Hook Installation` 不改（其 scenario 寫「SHALL contain a Bash matcher entry」沒寫「ONLY」，加 Read entry 後仍成立；兩條 requirement 用各自 scenario 守 regression）

## Impact

- Affected specs: `lint-feedback-loop`
- Affected code:
  - Modified: codebus-cli/src/commands/hook.rs（加 CheckRead variant + check_read 函數 + is_image_path predicate + tests）
  - Modified: codebus-core/src/vault/settings.rs（更新 DEFAULT_SETTINGS_JSON + 既有測試 assertions）
  - New: (none)
  - Removed: (none)
