# Backlog: raw mirror 巢狀 .git 未排除（submodule / nested repo leak）

**Date:** 2026-05-19
**Surfaced during:** backlog 討論（roadmap review，user 詢問「複製 codebase 有複製 .git 嗎」）
**Severity:** PII / 體積 leak（edge case，非主流情境）
**Owner:** harry
**Status:** archived 2026-05-20 — implemented as change `raw-sync-nested-git-leak`(any-depth `.git` 排除 + 兩處 call site 共用 `is_excluded_path` helper + 5 個新 regression tests)

---

## 觀察

`raw_sync.rs` 把 source repo 鏡像進 `.codebus/raw/code/` 時，用
`ALWAYS_SKIP_AT_ROOT = [".codebus", ".git", ".env"]`（raw_sync.rs:26）排除
版本控制目錄。但排除判斷只看 **相對路徑的第一個 segment**：

```rust
let first_seg = rel.iter().next().and_then(|s| s.to_str()).unwrap_or("");
if ALWAYS_SKIP_AT_ROOT.contains(&first_seg) { continue; }
```

`sync_with_scanner_into`（raw_sync.rs:184-187）與 `walk_source_for_signal`
（raw_sync.rs:96-99）皆同一邏輯。後果：

| 情境 | 第一段 | 是否被擋 |
|------|--------|---------|
| `<root>/.git/config` | `.git` | ✓ 擋掉 |
| `<root>/vendor/foo/.git/config`（submodule / nested repo） | `vendor` | ✗ **會被鏡像** |
| `<root>/sub/.env` | `sub` | ✗ 同樣只擋 root 層 |

唯一的兜底是 walk 為 gitignore-aware（`git_ignore(true)` + `standard_filters(true)`，
raw_sync.rs:154-165）。但巢狀 `.git/` 通常**不會**被外層 `.gitignore` 列入
（submodule 的內容由 `.gitmodules` 管，objects 不在外層 ignore 規則裡），
所以實務上一個含 submodule 或內嵌 repo 的 source，其 `.git/objects/`、
`config`（可能含 remote URL / credential helper 設定）、`.git/logs/` 都會被
原樣複製進 raw mirror，並進入 vault 的巢狀 git 歷史。

風險面：
1. **體積**：`.git/objects` 可能比工作目錄還大，膨脹 raw mirror 與 vault repo
2. **PII / secret**：`.git/config` 可能含帶 token 的 remote URL；packed objects
   含歷史上曾 commit 過、後來刪掉的敏感檔（PiiScanner 對 binary packfile 無效，
   因為非 UTF-8 直接 verbatim copy，raw_sync.rs:217-224）
3. **drift 噪音**：submodule 內 `.git/` 變動會影響 source signal 計算

目前 codebase **無測試覆蓋**巢狀 `.git`（`always_skip_root_dot_codebus_dot_git_dot_env`
只測 root 層）。

## 為什麼現在沒做

- 主流 source repo 沒有 submodule / nested repo，root 層排除已足夠
- 非 ship-blocking，不影響 F `v3-app-polish-ship` 的 release gate
- 影響面僅限「source 含巢狀 git」的 user，且需該 repo 同時含敏感 git 內容才升級成 leak

## 選項

### A. 什麼都不做

維持現況。對沒有 submodule 的 repo 完全沒問題。風險僅在特定 source 結構下成立。

工程量：0。

### B. 排除任意層級的 .git / .env（推薦）

把判斷從「第一段命中」改成「任一路徑 segment 命中 `.git`」。`.git` 在任何
深度都應排除（git 目錄名不會是合法 source 內容）。`.codebus` 維持只擋 root
層（深層 `.codebus` 是 user 自己的內容，不該假設）；`.env` 改成檔名比對
（任一層的 `.env` 都該擋，與現有 root-only 行為相比更安全，需確認無回歸）。

涉及：
- raw_sync.rs：新增 `path_has_skipped_segment(rel)` helper，取代 `first_seg` 比對
- `sync_with_scanner_into` 與 `walk_source_for_signal` 兩處同步改（兩者 filter
  必須一致，否則重蹈 v3-bug-fixes 的 init→goal drift 覆轍）
- 新增測試：`vendor/foo/.git/config` 不鏡像、`a/b/.git/objects/x` 不鏡像、
  深層 `.codebus` 仍鏡像（確認沒過度排除）

工程量：小（半天，含測試）。

### C. 用 `ignore` crate 的 `.gitignore` 機制 + 顯式 `.git` filter

除了 B 的 segment 比對，額外用 `WalkBuilder::filter_entry` 在進目錄前就剪枝
（避免 walk 進 `.git/objects` 再逐檔丟棄的 IO 浪費）。對大 submodule 有效。

工程量：小-中（半天-1 天）。

## 建議

走 B。一個 helper + 兩處 call site 對齊 + 3-4 條測試即可閉合，邏輯單純、
回歸面可控。C 的 `filter_entry` 剪枝是效能優化，等真的遇到大 submodule
拖慢 sync 再升級，不必一開始就做。

不單獨開 spectra change —— 體量小，建議併進 F `v3-app-polish-ship`（該
change 本來就要做跨平台驗證 + 收尾，順手把這個 PII/體積 edge 一起閉合），
或在 F 之前若有其他 raw_sync 相關 change 順帶處理。

## Tasks（方案 B，粗估）

1. raw_sync.rs：`fn path_has_skipped_segment(rel: &Path) -> bool`（`.git`
   任一層擋；`.codebus` 僅 root；`.env` 任一層 by file name）
2. `sync_with_scanner_into` 改用新 helper（取代 `first_seg` 比對）
3. `walk_source_for_signal` 同步改（兩處 filter 必須 byte-equivalent）
4. 測試：root `.git` 仍擋、`vendor/x/.git/config` 擋、深層 `.codebus/y` 不擋、
   `a/.env` 擋；並補一條 init→goal drift 不誤觸（沿用既有 manifest 測試模式）

## Out of scope

- `.git` worktree（`.git` 是檔案而非目錄的情況）—— 罕見，遇到再評估
- `.gitmodules` 解析、submodule-aware sync —— 過度工程，B 的目標只是「不要把
  巢狀 .git 內容鏡進去」，不是「正確處理 submodule 工作樹」
- 已存在 vault 的歷史清洗（若之前已鏡入巢狀 .git）—— 另開 migration 議題

## 何時動

併入 F `v3-app-polish-ship`（跨平台驗證 + 收尾階段）；或 F 前任何 raw_sync
相關 change 順帶。不獨立起 change。
