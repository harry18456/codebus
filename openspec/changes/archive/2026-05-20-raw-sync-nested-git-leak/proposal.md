## Problem

`codebus-core/src/vault/raw_sync.rs` 排除 `.git` 目錄的判斷只看「相對路徑的第一個 segment」，所以巢狀 `.git`（git submodule、source repo 內含的 nested repo）會被原樣鏡像進 `.codebus/raw/code/`。後果三層：

1. **體積**：`.git/objects/` 常比工作目錄還大，膨脹 raw mirror 與 vault 巢狀 git
2. **PII / secret**：`.git/config` 可能含帶 token 的 remote URL；packed objects 含歷史上曾 commit 過、後來刪掉的敏感檔（PiiScanner 對 binary packfile 無效，因為非 UTF-8 直接 verbatim copy）
3. **drift 噪音**：submodule 內 `.git/` 變動會干擾 `walk_source_for_signal` 的 source signal 計算

## Root Cause

`ALWAYS_SKIP_AT_ROOT = [".codebus", ".git", ".env"]` 配合 `let first_seg = rel.iter().next()` 只比對第一段。`.git` 在巢狀位置（如 `vendor/foo/.git/config`）`first_seg` 是 `vendor`，落到 fallthrough → 進 walk → 進 mirror。外層 `.gitignore` 通常不會列入 submodule 的 `.git/`，所以 `git_ignore` filter 也救不了。

## Proposed Solution

把 `.git` 的排除規則從「first segment」升級為「any path segment」：任一相對路徑 segment 等於 `.git` 即跳過。`.codebus` 與 `.env` 維持 root-only（沒有改動需求；最小化回歸面）。

兩處 call site 必須**同步修改**：`sync_with_scanner_into`（rawmirror 寫入路徑）與 `walk_source_for_signal`（source signal 計算）。如果只改其中一邊會重蹈 `v3-bug-fixes` 那次 init → goal drift 誤觸的覆轍。

## Non-Goals

- 不擴展 `.env` 變成 any-depth（user 沒要求；保留現行 root-only 語意）
- 不擴展 `.codebus` 變成 any-depth（巢狀 `.codebus` 是 user 內容，不該假設）
- 不引入 `ignore::WalkBuilder::filter_entry` 剪枝（效能優化，等真的遇到 submodule 大到拖慢 sync 再做）
- 不處理 `.git` 是 worktree pointer 檔（而非目錄）的情況（極罕見）
- 不解析 `.gitmodules` 做 submodule-aware sync（過度工程）
- 不清洗既有 vault 歷史（若先前已鏡入巢狀 .git，是另開 migration 議題）

## Success Criteria

- `<repo>/vendor/foo/.git/config` 不出現在 `.codebus/raw/code/`（巢狀 submodule 場景）
- `<repo>/.git/config` 仍不出現在 mirror（root-level 回歸保護）
- `<repo>/.codebus/foo` 不誤擋（巢狀 `.codebus` 視為 user 內容應鏡像）
- `<repo>/a/.env` 不誤擋（巢狀 `.env` 視為 user 內容應鏡像；root-only 語意不變）
- `init` 緊接 `goal` 不誤觸 re-sync（兩處 call site filter 一致）
- 既有 18 條 `raw_sync.rs` 單元測試零退步

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `vault`: 「Raw Mirror with PII Scanner」requirement 的 skip 規則由「skip top-level entries `.codebus/`, `.git/`, and `.env`」擴充為「`.git/` at any path depth；`.codebus/` 與 `.env` 維持 top-level only」；新增 scenario 涵蓋巢狀 `.git` 排除

## Impact

- Affected specs: `vault`（modified）
- Affected code:
  - Modified:
    - codebus-core/src/vault/raw_sync.rs（exclusion helper + 兩處 call site 對齊 + 新增 regression tests）
  - New: (無)
  - Removed: (無)
