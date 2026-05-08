## Context

`raw_sync` 是 ingest flow 的 first I/O step：每次 `--goal` 跑前把 user repo mirror 進 `.codebus/raw/code/`，agent 之後只透過 `Glob/Read` 看 raw mirror（cwd-isolated 在 `.codebus/`，不能 read 原 repo）。整個 PII attack surface 集中在 mirror 的「寫入瞬間」— 寫進去之後無論 agent 怎麼跑，泄密就已經發生。

plugin-architecture-refactor（archived 2026-05-06）已 ship：

- `PiiScanner` trait（sync，object-safe）
- `NullScanner`（default，0.2.0 行為保留）
- `RegexBasicScanner`（4 條 builtin pattern：aws-access-key、anthropic-api-key、email、ipv4 + `patterns_extra`）
- `factory::build_scanner(ScannerConfig)` → `Box<dyn PiiScanner>`
- `OnHit` enum（`Warn`、`Skip`、`Mask`）
- `~/.codebus/config.yaml` `pii` section（loader.rs 的 13 個 spec scenarios 已 cover）

缺的只是把 scanner 接進 raw_sync 的 fs::copy 那一步、以及實作 `OnHit` 三模式的 actual 行為。raw_sync 既有 5 個 unit tests + uv vault `--check` byte-equal gate 覆蓋了「不接 scanner 的行為」，所以這次新增的測試只要 cover 「接了 scanner 後的行為」即可。

## Goals / Non-Goals

**Goals:**

- `--goal` flow 預設行為與 0.2.0 byte-equal（user 沒設 config.yaml → NullScanner → no-op）
- user opt-in 後（`pii.scanner: regex_basic`）每個 text 檔被掃，命中時依 `on_hit` 三模式分別觀察得到結果
- raw_sync 的既有 5 個 unit tests 全綠（不能 regress）
- 新增 inline 測試覆蓋 `OnHit::Warn`、`OnHit::Skip`、`OnHit::Mask`、binary-file fall-through、multi-pattern-hit、`patterns_extra` 真的觸發
- 對 binary file 不假命中（regex 跑不下去就 fall through）

**Non-Goals:**

- 不掃 `wiki/` 內容（scope 是 raw mirror）
- 不持久化掃描結果（warn-stderr only）
- 不擴展 builtin pattern 集
- 不對 `.codebus/raw/` 既有檔做 incremental scan（raw_sync 是 wipe-and-rebuild，不需要）
- 不做 multi-thread scan parallelism（per-file scan 跟 fs::copy serialized；regex 已 RE2-fast，瓶頸在 fs I/O）

## Decisions

### Scanner invocation 點：copy-then-scan vs read-scan-write

**選擇**：對每個 candidate file（過 size limit、過 gitignore 後），改 `fs::copy(src, dst)` 為「`fs::read_to_string(src)` → `scanner.scan(content, rel_path)` → 依 OnHit 寫入 dst（或不寫）」。

```rust
match fs::read_to_string(path) {
    Ok(content) => {
        let matches = scanner.scan(&content, rel_path_str);
        if matches.is_empty() {
            fs::write(&dst, &content)?;  // identical to fs::copy outcome
        } else {
            apply_on_hit(on_hit, &content, &matches, rel_path_str, &dst, &mut stderr)?;
        }
    }
    Err(_) => fs::copy(path, &dst)?,  // binary or non-UTF-8 → original copy path
}
```

**為什麼**：
- 一次 read 把整檔載到 memory，scanner.scan 純 CPU、零額外 I/O — 比 mmap 簡單、5 MiB 上限 already enforced，記憶體 footprint 可預測（worst case 每檔 5 MiB × 1 thread）
- mask 模式天生需要原內容才能改寫；warn / skip 模式也都需要 scan 結果，所以 read-once-scan-once 是 minimum 必要工
- 不用 read 兩次（先 read 掃、再 fs::copy）— 那會 double I/O

**Alternatives considered**：

- **fs::copy 後再從 dst 掃 + 視情況 truncate / rewrite**：方便 reuse 現有 fs::copy 路徑；否決：mask 模式要 rewrite，等於 copy 完再讀再寫三次 I/O；skip 模式更糟（要 copy 完再 unlink）
- **mmap 當輸入給 scanner**：避免整檔載入；否決：5 MiB 上限下 mmap 沒收益、Windows path 額外複雜

### Binary file 處理：fall through 走原 fs::copy

**選擇**：`fs::read_to_string` 失敗（非 UTF-8 / IO 錯誤）→ `fs::copy(path, &dst)` 完全不掃。

**為什麼**：
- regex crate 對 `&str` 操作；強行 `String::from_utf8_lossy` 會把 `\xFF\xFE` 換成 U+FFFD，讓 byte-pattern 失準
- binary（image、binary blob）天生不會有 plain-text secret；secret 在 binary 裡通常是 base64 / hex string，那是 text 的問題不是 binary 的
- agent 看不到 binary 內容（Read tool 對 image / binary 走另條路徑），漏掃對 LLM context 沒影響
- 既有 5 MiB 上限 + UTF-8 篩選 = 雙重保險

**Alternatives considered**：

- **強制 UTF-8 decode、有效 byte 才掃**：複雜、收益小
- **UTF-8 失敗就 skip 整檔**：太激進、會把 PNG / PDF 全 skip 掉，agent 看不到 binary 是 0.2.0 既有行為，現在突然 skip 會視覺退步
- **針對 binary 用 byte-level pattern 庫**：scope creep，留給未來 ML scanner

### `OnHit::Mask` 多 match 重疊處理：last-match-wins

**選擇**：`RegexBasicScanner.scan` 已保證 `matches` 按 `start` 升序排序；mask 從後往前替換、單純跳過 overlap（後 match start < 前 match end → 丟棄前者）。實作上：

```rust
fn apply_mask(content: &str, matches: &[PiiMatch]) -> String {
    let mut out = content.to_string();
    let mut last_end = content.len();
    for m in matches.iter().rev() {
        if m.end > last_end { continue; }       // overlap with already-replaced range
        out.replace_range(m.start..m.end, &format!("[REDACTED:{}]", m.pattern_name));
        last_end = m.start;
    }
    out
}
```

**為什麼**：
- 從後往前替換避免 offset 漂移
- overlap drop 對既有 4 條 builtin pattern 不會發生（aws-key / anthropic-key / email / ipv4 互不交集）
- 行為可預測 + 文檔一句話說完

**Alternatives considered**：

- **interval merge + 一次替換**：lib code 該寫；否決：複雜度不值現有 patterns
- **first-match-wins**：要先過濾再替換、邏輯較繞；同樣不值

### `OnHit::Skip` 真的整檔不寫 vs 寫空檔

**選擇**：整檔不寫（不在 `raw/` 出現）。

**為什麼**：
- agent 後續 Glob 看不到，邏輯一致：「這檔不存在於 mirror」
- 寫空檔 / placeholder 反而誤導 agent — 可能跟著嘗試 Read 然後拿到空字串
- skip 行為對 sha256 / stale_detect 不是問題（那兩個吃 frontmatter `sources[].path`，agent 沒寫過 source 自然不會 enrich）

**Alternatives considered**：

- **寫一個 `// skipped: pii hit` placeholder**：好像有點告知性；否決：agent 用 Glob 看 raw 結構時會把 placeholder 算進「這檔有」、可能誤判 module 存在
- **mirror 但 chmod 000**：跨平台噁心（Windows 沒這語意）

### Stderr warning 格式：固定 ASCII en-us

**選擇**：兩條格式：

```
warning: PII match in <rel_path>: <pattern_name> at offset <byte_start>
skipped: <rel_path> (reason: pii hit <pattern_name>)
```

`mask` 模式不對 stderr 寫（內容已替換、靜悄悄）。

**為什麼**：
- en-us：spec 容易 pin 關鍵字、不依賴 locale
- per-match 一行（`Warn` 模式可能多行）：grep-friendly
- offset 是 byte offset：與 `PiiMatch.start` 一致、reproducible
- mask silent：替換已生效、stderr 干擾流程

**Alternatives considered**：

- **summary-style「N matches across M files」**：方便讀；否決：太抽象、看不到哪檔哪條 rule、debug 不便
- **mask 也 stderr**：double signal；否決：噪音、且 user 已經主動選 mask = 預期不再被打擾

### raw_sync API 形態：新增 with-scanner 入口、保留舊 alias

**選擇**：

```rust
pub fn sync_repo_to_raw(repo: &Path, raw_dir: &Path) -> io::Result<()> {
    let null = NullScanner::new();
    sync_repo_to_raw_with_scanner(repo, raw_dir, &null, OnHit::Warn)
}

pub fn sync_repo_to_raw_with_scanner(
    repo: &Path, raw_dir: &Path,
    scanner: &dyn PiiScanner, on_hit: OnHit,
) -> io::Result<()> { ... }
```

**為什麼**：
- 既有 5 個 unit tests 都用 `sync_repo_to_raw(repo, raw)` 兩參數版 — 0 個改 — null path 自然走預設 no-op
- goal command 改 call new 4-arg 版本、注入 cfg.pii 衍生的 scanner
- 未來 commands 若要直接掃 (e.g., `--check --pii` standalone)，介面已存在

**Alternatives considered**：

- **直接改 sync_repo_to_raw 為 4 參數**：所有 caller 都要改；既有 5 個 tests 都要改參數列；損益不對
- **builder pattern**：殺雞用牛刀

### Test 策略：inline，不開 fixture-vault

**選擇**：tests 全寫在 `codebus-core/src/fs/raw_sync.rs` 的 `#[cfg(test)] mod tests` 內、用 tmp dir + fixture content（既有 helper `tmp`/`write`/`list_relative` 已備）。

**為什麼**：
- raw_sync 既有 test 已 inline、加 case 不破風格
- PII 行為的 input / output 都是檔案內容、不需要對 wiki / lint / CLI 任何整合就能驗證
- 跑得快、CI 友善

**Alternatives considered**：

- **build a `tests/fixtures/pii-vault-snapshot/` fixture + byte-equal gate**：和 uv vault 並列；否決：raw mirror 不是 user-visible artifact（不會 commit、不在 git history），byte-equal 是 over-spec

## Risks / Trade-offs

- **Risk: false positive 把無辜檔 mask 掉，影響 agent 對 source 的理解**
  → Mitigation：(a) 4 條 builtin pattern 都嚴格邊界（`\b` + 長度下限）、test 已含 negative case；(b) `on_hit: warn` 是預設模式，user 主動選 skip / mask 才會對 agent 可見；(c) `patterns_extra` 進來前已 fail-fast on bad regex（plugin-architecture-refactor 的 lock-in test 已 cover）

- **Risk: 大檔 read_to_string 失敗，整檔被當 binary 漏掃**
  → Mitigation：(a) 既有 `MAX_FILE_BYTES = 5 MiB` 預過濾；(b) UTF-8 fail 路徑記 stderr debug log（非 user-visible warning）— 讓 user 知道有檔走了 fall-through；(c) 文檔明寫「PII 過濾僅作用於 UTF-8 text 檔」

- **Risk: mask 替換改變檔內容，agent 引用 line number 失準**
  → Mitigation：(a) `[REDACTED:<name>]` 是同行替換、不增減換行 → line-number 不變；(b) byte offset 會位移，但 agent 在 ingest flow 用 line-number-base 寫 sources，不在 ingest 中跑 byte offset；(c) 文檔裡標「mask 是破壞性的、需要原始字串請改 skip」

- **Trade-off: 接受 read-then-write 比 fs::copy 慢**
  → 數字上：每檔多一次 read + write syscall（vs sendfile / CopyFileEx 的 kernel-side copy）；regex scan 對 5 MiB 文字 < 50 ms；對 1k 檔的 repo 預估增 1-3 s — acceptable for security baseline

- **Trade-off: en-us stderr 對非英文 user 不友善**
  → 接受：spec 行為要可 grep、訊息已有 prefix（warning: / skipped:）+ rel_path + pattern_name 三段資訊夠用；i18n 是日後 telemetry/UI 工作

## Migration Plan

單一 R 階段，不分 sub-phase（scope 比 plugin refactor 小一個量級）：

1. 加 `sync_repo_to_raw_with_scanner` + 內部 helper `apply_on_hit`、`apply_mask`
2. `sync_repo_to_raw` 改成 thin wrapper（呼 with_scanner + NullScanner + Warn）
3. 加 6 個 inline tests：warn (含 stderr)、skip、mask single、mask multi、binary fall-through、patterns_extra trigger
4. main.rs::run_goal_cmd 從 `cfg.pii` build scanner、改 call with_scanner 變體
5. 跑 `cargo test --workspace` + uv `--check` byte-equal + buddy-gacha smoke
6. final commit：`feat(pii): wire RegexBasicScanner into raw_sync with three on_hit modes`

### Rollback 策略

單一 commit，rollback = `git revert <hash>`。

### Cool-down

跑自己 buddy-gacha 一輪 `pii.scanner: regex_basic, pii.on_hit: warn`、確認 stderr 沒爆量也沒漏掃自己的 test secret（用 `INTERNAL-123456` 之類 patterns_extra 試）；ok 後才 archive。

## Open Questions

- **`mask` 模式對 secret 的 entropy 暴露**：替換成 `[REDACTED:<name>]` 暴露了「這裡有什麼 type 的 secret」，theoretically 比直接刪除多洩 1 bit metadata。傾向接受 — debug-friendliness 比微小 entropy 概念 leak 重要。R 階段確認時 reflect 一下要不要加 `pii.mask_label: false` flag 給 paranoid user。
- **scanner 建構失敗的 fallback**：`build_scanner(cfg)` 若 patterns_extra regex 編譯失敗會 Err。當前 plan：main.rs 接到 Err 直接 exit(1) + 打 user-facing 訊息。要 vs. fall back to NullScanner + warn？傾向 fail-fast — user 寫了 patterns_extra 就是有意圖，靜默退化反而危險。
- **未來支援 `--no-pii` CLI flag override**：類似 `--no-emoji`，臨時關掉 PII 過濾。本次不加；但確認 raw_sync API 的 4 參數版可以接 `&NullScanner` 直接覆蓋，flag 要時加 main.rs 一行即可、不需重寫。
