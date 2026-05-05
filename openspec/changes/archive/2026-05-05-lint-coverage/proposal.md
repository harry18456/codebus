## Why

Phase 1 archive 後盤點發現兩個 process gap：

1. **`wiki-lint` 整個 capability 沒進 openspec**——auto-lint after ingest（soft mode）、`codebus --check` standalone 命令、所有 lint 規則跟 nav file 補強，全部已 ship 且有 tests，但 `openspec/specs/` 沒有任何對應 requirement。原 phase 1 spec §16 把 lint 寫成「phase 2 backlog」，實作期 commit `cd5adff` / `6323971` / `8d73cbb` 把 soft mode + `--check` + 5-folder lint + nav file 補強都落地了，archive 流程沒 catch 到「spec 寫 backlog 但 code 已 ship」的 mismatch。
2. **`docs/superpowers/` 跟實作 diverge 而沒 marker**——superpowers spec / plan 是 phase 1 設計期快照，archive 後續修補（Karpathy 5-folder migration、iter-9 sandbox fix 部分內容、lint nav file 補強）只有 sandbox iter-9 同步回 superpowers spec，其他都沒回填。未來實作者讀到 spec 寫 `wiki/pages/`、plan 寫 `--disallowedTools` 會誤以為是當前設計，造成 implementation drift。

兩件事 root cause 相同（phase 1 過程中文件沒跟上 ship），合併處理一次清算。

## What Changes

- **新增 `wiki-lint` capability spec**：retroactive 紀錄 phase 1 已 ship 的 lint 功能，包含 7 條 requirements（auto-lint trigger、--check 命令、5 folder/type 規則、nav file 補強、報告格式等）
- **加 starting-spec banner 到 superpowers 文件**：
  - `docs/superpowers/specs/2026-05-04-codebus-v2-phase1-design.md` 開頭加 banner
  - `docs/superpowers/plans/2026-05-04-codebus-v2-phase1.md` 開頭加 banner
  - banner 內容固定字串：標明檔案是 phase 1 brainstorming snapshot、source of truth 是 `openspec/specs/`、drift 預期會發生、未來實作不要從這檔案動工
- **新增 `docs/superpowers/README.md`**：governance doc，說明 superpowers 目錄的角色（初期發想工具）跟對應的 openspec 同步 policy（後續所有 capability 變更走 `/spectra-propose`）

## Non-Goals

- **不修 superpowers 文件的內容 drift**——明確接受 spec/plan 文字停留在 phase 1 設計期狀態（譬如 spec 6 處 `wiki/pages/`、plan 9 處殘留），banner 已警告讀者，內容修了也只是 phase 1 過去的考古，沒有 forward value
- **不重新 design lint 規則**——retroactive spec 對應現有 code state，不藉機新增規則或調整行為。任何 lint 規則的真實升級（譬如 Karpathy 完整 lint 的 contradictions / orphans / coverage check）走獨立 change
- **不動 phase 1 archive**——`openspec/changes/archive/2026-05-04-codebus-v2-phase1/` 內容保持當時狀態，不回填新 capability。lint capability 加在 main `openspec/specs/wiki-lint/` 而非 archive
- **不改 lint production code**——本次 change 不動 `src/core/wiki/lint.ts` / `src/ui/lint-report.ts` / `src/commands/goal.ts` / `src/commands/check.ts`。但 retroactive spec 寫進 3 個 scenario（lint:null fallback、--check 在 errorCount>0 時 drives exit 1、related[] 非 `[[...]]` 格式）原本沒對應的 dedicated test，加 3 個 test 補回，覆蓋已 shipped 行為。如果 retroactive spec 過程中發現 code 跟設計不一致，分裂出獨立 bug-fix change

## Capabilities

### New Capabilities

- `wiki-lint`: Vault wiki 的結構/相容性驗證——auto-lint after ingest (soft mode 不阻擋 commit) + standalone `codebus --check` 命令 + 跨 5 type folder 規則 + nav file body wikilink 掃描 + folder/type 一致性檢查

### Modified Capabilities

(none)

## Impact

- Affected specs:
  - New: `openspec/specs/wiki-lint/spec.md`
- Affected code: tests only — `tests/commands/goal.test.ts` (+1: lint:null fallback), `tests/commands/check.test.ts` (+1: errorCount > 0 → exit 1), `tests/core/wiki/lint.test.ts` (+1: related[] entry not in [[wikilink]] format). Production code unchanged (already shipped in phase 1).
- Affected documentation:
  - Modified: `docs/superpowers/specs/2026-05-04-codebus-v2-phase1-design.md` (add starting-spec banner at top)
  - Modified: `docs/superpowers/plans/2026-05-04-codebus-v2-phase1.md` (add starting-spec banner at top)
  - New: `docs/superpowers/README.md` (governance doc)
