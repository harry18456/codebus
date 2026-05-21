## Why

`pretooluse-image-block` change（2026-05-20 ship）讓 `.codebus/.claude/settings.json` 預設帶兩條 PreToolUse hook（Bash + Read），Read hook 擋圖片副檔名是 PII safety floor。但目前**hard-coded**：

- User 沒有「我知道我這個 repo 沒 PII 風險、想讓 agent Read 圖片」的 escape hatch
- 既有 vault 升級沒自動 migrate，跟新 vault 行為不一致（已 init 過的沒 Read hook，新 init 的有；user 想全部 disable 也要逐個 vault 改 settings.json）
- Settings UI 沒有任何控制入口，user 連現況都看不到

加 config knob + Settings UI toggle 讓 user 有顯式的開關。預設**仍是 ON**（保留 PII safety floor），純粹加 escape hatch。

## What Changes

- `~/.codebus/config.yaml` 新增 `hooks.read_image_block` 布林欄位，預設 true（absent → true，沿用既有 PII safety floor 行為）
- `codebus hook check-read` 子命令在 stdin 處理流程開始時讀 config，若 `hooks.read_image_block: false` 則 always 返回 allow（exit 0、空 stdout），不再執行副檔名 blocklist 比對
- Config 變更**立刻對所有既有 vault 生效**（next agent spawn 讀新 config），不需要 re-init 或改 settings.json
- Settings UI EndpointSection（或對等位置）加 toggle row「Block image / binary reads」，狀態反映 `hooks.read_image_block`，文案警告關掉後圖片可被 agent ingest 而 bypass PII filter
- i18n：zh-tw + en 對應 label / tooltip / 警告文案
- Migration：既有 user yaml 沒 `hooks` section → config load tolerant default true → behavior 不變、不需 fail-loud parse error

## Non-Goals

- **拆 Bash hook toggle**：本 change 只動 Read hook。Bash hook 是 fix 沙箱必要 gate，沒理由給 toggle
- **per-vault override**：user 若要某個 vault 不擋圖片，可手動編 `<vault>/.codebus/.claude/settings.json` 移除 Read entry；本 change 不引入 vault-local config 機制
- **整個 hook system 變 optional**：scope creep；本 change 只處理 Read 一條
- **Fail-loud 對 yaml 沒 hooks section**：跟 verify-stage-independent-model 的 fail-loud 設計**故意相反**——verify 是改變 cost 行為（fail-loud 避免 cost surprise），這條只是讓既有行為可關（absent 默認等同既有行為，無 surprise 風險）
- **新增「per-extension toggle」（讓 user 自選擋哪些副檔名）**：YAGNI；想客製化的 user 可手動編 vault settings.json 移除 Read entry
- **`codebus init --no-image-block` flag 寫不同 settings.json template**：跟 config-driven 的 runtime check 重複；config 是 single source of truth
- **`codebus config get/set hooks.read_image_block` CLI**：scope creep（既有 config CLI 只管 keyring key，不管 yaml 一般 knob）
- **觀察性：log 紀錄 hook 被 disable**：events.jsonl 已紀錄每個 spawn 的工具呼叫，user 可從那邊間接看到圖片有沒有被 read。本 change 不加額外觀察性

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `lint-feedback-loop`: `PII Image Read Hook Installation` requirement 加 config gate 條款 —— `codebus hook check-read` 行為 gated by `hooks.read_image_block` config key（預設 true）；config 為 false 時 always allow、不執行副檔名 blocklist。Hook entry 仍 unconditionally 寫進 `.codebus/.claude/settings.json`（gate 在 runtime 不在 install time）
- `app-shell`: Settings UI 加新 toggle row（不屬於既有 EndpointSection / PII Section / Quiz Section）控制 `hooks.read_image_block`，狀態反映 yaml 值、改動觸發 save_global_config

## Impact

- Affected specs: `lint-feedback-loop`, `app-shell`
- Affected code:
  - Modified: codebus-core/src/config/global_starter.rs（STARTER_CONFIG 加 hooks section 註解）
  - Modified: codebus-cli/src/commands/hook.rs（`check_read` 讀 config 決定 block / allow + tests）
  - Modified: codebus-app/src/lib/ipc.ts（config schema TypeScript 補 hooks section + 預設值）
  - Modified: codebus-app/src/store/settings.ts（讀寫 hooks.read_image_block 欄位）
  - Modified: codebus-app/src/components/settings/SettingsModal.tsx（加 toggle row + tests）
  - Modified: codebus-app/src/i18n/messages.ts（zh-tw + en）
  - New: codebus-core/src/config/hooks.rs（hooks config 區塊 load helper，跟既有 pii / lint / log 並列）
  - New: (none — Settings UI toggle 進 SettingsModal 既有檔，不開新 component)
  - Removed: (none)
