# T6 品質檢查：codebus-core（Part 1 — PII 冗餘層 + git）

**Date:** 2026-05-22
**Task:** loop T6（只讀分析，產 backlog 候選）
**範圍說明:** core 共 ~18k LOC。一輪 loop 無法深讀全部，本輪聚焦**安全關鍵的 PII redaction 路徑**（pii/ + vault/raw_sync.rs）與 git/，全檔精讀。其餘大模組（verb 4.5k / config 3.5k / vault 其餘 / wiki / log / render）列為後續 review 候選（見末尾）。

---

## 🔴 F1（headline）：PII Mask 的「非重疊」前提未被強制 → 可能漏遮 / 輸出損壞

**位置:** `vault/raw_sync.rs:345 mask_matches` + `pii/scanners/regex_basic.rs:100 scan`
**嚴重度:** 中-高（安全關鍵層的正確性；後果是 PII/credential 漏進 raw mirror）

`mask_matches` 的 doc（`:343`）明寫「Assumes `matches` are non-overlapping and sorted ascending」，實作靠 `.rev()` + `replace_range(start..end)` 倒序替換以保 offset 有效。**但這個前提沒有任何 producer 保證**：`scan()`（`regex_basic.rs:100-115`）對每條 rule 各自 `find_iter` 後只 `sort_by_key(start)`，**從不合併/去除跨 rule 的重疊或包含關係**。

**觸發情境:** 兩條 rule 在同一段文字產生重疊/巢狀 match。最現實的是 `patterns_extra` 自訂規則框住一段含內嵌 builtin 命中的字串，例如自訂規則匹配整條連線字串、其中內嵌一個 email/IP/key：
- 排序後 outer（custom，start 較前）、inner（email，start 較後）。
- `.rev()` 先替 inner → 字串長度改變；再對 outer 做 `replace_range(outer.start..outer.end)`，但 `outer.end` 此時已落在 inner 替換後位移過的位置 → **切進別的內容、輸出損壞**，且 `m.end > out.len()` 的防呆只擋越界、擋不了「指到錯位置」。
- 後果可能是 **inner 的 secret 未被完整遮蔽**，或 mirror 出損壞檔。

**建議修法（之後實作）:** 在 mask 前（或 scan 末尾）做一次 interval-merge——把重疊/巢狀 match 合併成不相交區間（取聯集 start..max(end)），再餵給 `mask_matches`。約半天 + 測試（重疊 builtin、custom 框 builtin、相鄰非重疊不受影響）。
**為何重要:** 這是整個 PII floor 的最後一道；倒序替換的「聰明」做法剛好對重疊不安全，而前提只寫在註解、沒測試守。已加進 BACKLOG。

## 🟡 F2：>5 MiB 檔案被靜默排除出 mirror（無 warn 行）

**位置:** `vault/raw_sync.rs:242-244`（`if meta.len() > MAX_FILE_BYTES { continue; }`，`MAX_FILE_BYTES=5MiB :84`）
**嚴重度:** 低-中（可預期性 / 透明度）

超過 5 MiB 的檔案在掃描前就 `continue`——**既不複製、也不發任何 warn/summary 計數**。結果：大檔（大型 generated source、data fixture 等）悄悄不在 `raw/code/`，goal/query 看不到它，使用者無從得知為何某檔「沒被文件化」。建議：至少累加一個 `pii`-無關的 `oversized_skipped` 計數 + 一行提示，讓 silent gap 變可見。

## 🟢 F3：`changed_paths_under` 把刪除的頁也算「changed」

**位置:** `git/nested_repo.rs:74-96`
**嚴重度:** 低（邊緣）

`git diff --name-only <base> -- <subdir>` 會列出**被刪除**的檔（以舊路徑）。goal content-verify 拿這份清單去 Read 每頁做 faithfulness 檢查時，刪除頁會 Read 失敗。設計意圖是「added or modified」（doc `:69`），刪除不在內。實務上 goal 很少刪頁，故低；若要嚴謹可加 `--diff-filter=d`（排除 deleted）。

## ✅ 觀察到的好設計（非問題，記錄）
- git 模組：shell-out 失敗一律轉 `io::Error` 上拋（`:98-118`）、idempotent init 不覆蓋使用者 config、測試覆蓋紮實。
- PII：critical（AWS/Anthropic key）**無視 on_hit 強制 mask 的安全 floor**（`raw_sync.rs:291-313`）設計正確；非-UTF8 檔 verbatim 複製不誤掃；regex 用 RE2（無 catastrophic backtracking）；custom pattern compile 失敗 fail-fast。
- 整個 core **零 TODO/FIXME**，紀律好。

## 後續 review 候選（T6 未覆蓋，建議排 T6b/T6c）
逐模組深讀仍待做，優先序建議：
1. `verb/`（4.5k LOC，最大、含 spawn/錯誤處理/marker 解析）— 最可能藏 bug。
2. `config/`（3.5k，endpoint 解析 + 兩 provider profile + 驗證）。
3. `vault/` 其餘（init 流程、manifest、drift detect）。
4. `log/`、`wiki/`、`render/`。
重點 lens：741 處 `unwrap/expect`（非 test）的 spawn/IO 邊界是否有會 panic 的使用者可達路徑。

## 待 harry
F1 是真實 latent 安全 bug，建議排進實作（interval-merge，約半天）。要不要我（在解除「只讀」邊界後）直接修，或先繼續 review 其餘模組累積清單？
