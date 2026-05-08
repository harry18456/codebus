## 1. Banner enum 擴充 + format 單元測試

- [x] 1.1 對每個新 Banner variant（SyncStart、SyncDone、PiiSummary、LintStart、LintDone、FixIterStart、FixIterDone、CommitDone）寫 `format_banner` 單元測試，emoji on / off 兩條路徑各一案
- [x] 1.2 在 codebus-core/src/render/event_renderer.rs 的 `Banner<'a>` 加上對應 variant，欄位嚴格按 spec：`SyncDone { files, mib, elapsed_ms }`、`PiiSummary { scanner, scanned, hits, action }`、`LintDone { errors, warns, elapsed_ms }`、`FixIterStart { i, max }`、`FixIterDone { i, fixed, remaining, elapsed_ms }`、`CommitDone { sha7 }`
- [x] 1.3 在 codebus-core/src/render/renderers/terminal.rs 的 `format_banner` 加 match arm，emoji prefix 沿用既有 lifecycle banner 同一套 glyph，讓 1.1 測試全綠

## 2. raw_sync 回傳 PII 計數

- [x] 2.1 為 `sync_repo_to_raw_with_scanner` 的新回傳型別 `SyncSummary { files, bytes, scanned, hits, action }` 寫單元測試（NullScanner 0 hits + RegexBasic n hits 兩案）
- [x] 2.2 把 codebus-core/src/fs/raw_sync.rs 的回傳改為 `io::Result<SyncSummary>`，內部累加 scanned/hits/action，更新所有 caller（commands/goal.rs、commands/fix.rs 與既有 unit / integration tests）解構新型別

## 3. Goal flow 串 stage banner

- [x] 3.1 [P] 為 commands/goal.rs 寫 integration test：CollectingRenderer 收集 `Vec<OwnedBanner>`，驗 sync → pii → lint → commit 的 banner 順序與 payload 欄位非空
- [x] 3.2 [P] 在 commands/goal.rs 的 `run_goal` 接入 SyncStart / SyncDone / PiiSummary / LintStart / LintDone / CommitDone（每個 stage 用 `Instant::now()` 量 `.elapsed().as_millis()`），實作 spec requirement "Render stage banners during goal flow" 與 "Render PII summary banner"

## 4. Fix loop stage banner

- [x] 4.1 [P] 為 commands/fix.rs 寫 integration test：每次 iteration 前後出現 FixIterStart / FixIterDone，payload `i` / `max` / `fixed` / `remaining` / `elapsed_ms` 一致
- [x] 4.2 [P] 在 commands/fix.rs 每次 iteration 前後 emit FixIterStart / FixIterDone，elapsed 量單一 iteration 不含 lint 或 commit

## 5. 驗證 banner 失敗不影響 goal 結果

- [x] 5.1 為 spec requirement "Stage banners do not block on stdout failures" 補 integration test：mock 一個會在 `render_banner` 第二次呼叫時返回錯誤的 renderer（既有 EventRenderer trait 簽名 `fn render_banner(&mut self, banner: &Banner<'_>)` 回 `()`，所以這條測試其實是驗「即使 renderer impl 內部把錯吞掉，goal flow 仍跑完並回正常 exit」），確認 wiki_changed / lint 結果與正常路徑一致

## 6. 測試 stub renderer 同步 + 驗收 + audit

- [x] 6.1 [P] 更新 codebus-cli/src/commands/goal.rs 內 `CollectingRenderer` stub：把 `render_banner` 從 no-op 改成收集 `Vec<OwnedBanner>` 以供 3.1 / 4.1 / 5.1 使用
- [x] 6.2 [P] 更新 codebus-cli/src/commands/fix.rs 與 commands/query.rs 的 stub renderer：對新 variant 補 no-op match arm（query 不發 stage banner，純避免 non-exhaustive match 編譯失敗）
- [x] 6.3 cargo test --workspace 全綠 + cargo clippy --workspace -- -D warnings 無警告
- [x] 6.4 cargo run --release 對 D:/side_project/uv 實機跑一次 goal，肉眼確認各階段 banner 順序、emoji prefix、elapsed 數字合理（與 bench_sync 量到的 6s / 39s 對齊）
- [x] 6.5 跑 spectra-audit：審查 raw_sync 新回傳型別 `SyncSummary` 是否在 panic / partial-write 路徑上仍能被 caller 安全解構，以及 stage timing 量測是否避免 `as_millis() as u64` 在極長 run 上溢位
