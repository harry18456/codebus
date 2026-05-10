## Problem

UV repo 驗收（`docs/v3-uv-verification-2026-05-10.md`）暴露兩個非 BREAKING bug：

1. **`init` 緊接 `goal` 觸發冗餘 raw_sync re-sync** — manifest 的 `source_signal.total_bytes` 不 match 隨後 goal 階段 `walk_source_for_signal()` 算的值，`detect_drift` 回 `true`，goal 在用戶毫無修改的情況下重新跑一次完整 raw_sync（uv repo 1289 檔多花 1.6s + 多吐 672 條 PII warn 噪音）。
2. **`codebus lint --repo <vault-root>` 安靜回 `0 pages, no issues`** — 當使用者把 `--repo` 指向 `.codebus/` 自身（誤用），lint 不報錯也不走 vault 內容，silently 跑空。看不出有問題、也找不到實際 lint issue。

## Root Cause

### Bug 1

`init.rs` orchestration 順序：

```
1. create_vault_layout
2. raw_sync           ← 算 summary.bytes（pre-mutation source state）
3. merge_internal_gitignore (vault internal)
4. init_nested_repo
5. ensure_codebus_in_gitignore (source .gitignore)  ← 把 ".codebus/\n" 寫進 source 的 .gitignore，bytes 變了
6. write_schema_if_missing
7. write_or_update_manifest with compute_source_signal(repo, &summary)
                       ← signal 用的是 step 2 的 summary.bytes（pre-mutation）
                       ← 但寫到 manifest 後，source 的真實狀態已是 post-mutation
```

`compute_source_signal` 直接用 `sync_summary.bytes` 當 `total_bytes`，等 goal 跑 `walk_source_for_signal(repo)` 重新算時，source `.gitignore` 已多了 `.codebus/\n` 那幾 bytes，兩者必然不 match → `detect_drift` 回 true → 多餘 re-sync。

### Bug 2

`locate_vault_root` 對 `repo_override` 路徑無條件 `repo.join(".codebus")`：

```rust
if let Some(repo) = repo_override {
    return Ok(repo.join(".codebus"));   // 永遠 join，不檢查
}
```

當 `repo_override = D:/.../.codebus` 時，回傳 `D:/.../.codebus/.codebus`（不存在）。`lint_wiki` 對不存在的 vault 路徑 walk 出 0 pages，回乾淨結果，CLI exit 0 — 完全 silent。

## Proposed Solution

### Bug 1 Fix

把 `ensure_codebus_in_gitignore(repo)` 從 init 第 5 步移到 **raw_sync 之前**（在 `create_vault_layout` 之後、`raw_sync` 之前）。raw_sync 看到 post-mutation source `.gitignore` → summary.bytes 與 goal 階段 walk 結果一致 → 無 drift。

`.codebus/` 在 `ALWAYS_SKIP_AT_ROOT` 永遠被 walk filter 跳過，所以 .gitignore 加 `.codebus/` 行對 raw_sync 內容**無實質影響**（不會掃 `.codebus/` 進 mirror），只影響 bytes count 一致性。

### Bug 2 Fix

`locate_vault_root` 對 `repo_override` 改成兩階段：

```rust
if let Some(repo) = repo_override {
    // 已是 vault root（含 wiki/）→ 直接用
    if repo.join("wiki").is_dir() {
        return Ok(repo.to_path_buf());
    }
    // 否則假設是 source repo → 再 join .codebus
    return Ok(repo.join(".codebus"));
}
```

兼顧兩種使用方式：
- `lint --repo <source-repo>` → 仍 `repo/.codebus`（正常 source 用法）
- `lint --repo <vault-root>`（即 `.codebus/`） → 直接用 `<vault-root>`，不再二次 join

對「路徑根本不存在」的情境（既有 unit test 覆蓋）— 保留現行「不檢查存在性、回傳 `repo.join(".codebus")`」行為，因為 `wiki/` 不存在時 `is_dir()` 回 false，自動 fall through 到舊 path。

## Non-Goals

- **不引入 path canonicalization**：`locate_vault_root` 仍只用字面 path 比對，不 `canonicalize()`。Bug 2 的根本是「使用者意圖被誤解」而不是 path normalization 問題
- **不改 detect_drift 演算法**：信號比對邏輯本身正確；bug 1 是 init 寫入 signal 的時機錯了，不是演算法 bug
- **不動 raw_sync 過濾規則**：`.gitignore` 仍照常寫入 raw mirror（不會被當 codebus-managed 跳過）— 改變 ALWAYS_SKIP 是 BREAKING 而且影響範圍大
- **不處理其他驗收 quality findings**：PII mask 過於激進（finding #1）和 spawned agent PATH（finding #4）各自有獨立 follow-up 路徑（v3-pii-severity-dispatch + docs commit）

## Success Criteria

- 對任意 git repo 跑 `codebus init` 緊接 `codebus goal "..."`（無 user 修改）：goal 階段 stdout 不包含 `~ 同步 source → raw/code...` banner（detect_drift 回 false → skip re-sync）。Verifiable：UV repo 重跑 init→goal，stdout 比對。
- `codebus lint --repo <vault-root>` 與 `codebus lint --repo <source-repo>` 對同一 vault 產生**一致**的輸出（兩種都正確 lint vault `wiki/` 內容）。Verifiable：UV repo 同時跑兩種，diff stdout。
- 既有 unit tests `cwd_with_only_dot_codebus_no_wiki_subdir_does_not_match` / `explicit_repo_override_does_not_check_existence` 仍綠（不破壞 cwd-based 偵測 + 路徑不存在 fallback 行為）。
- 全 workspace `cargo test` 綠。

## Impact

- Affected specs: `cli` (modified), `lint-feedback-loop` (modified)
- Affected code:
  - Modified:
    - codebus-cli/src/commands/init.rs
    - codebus-core/src/wiki/lint/locate.rs
