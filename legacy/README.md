# legacy/ — 退役版本墓園

codebus 走到 v3 是踩過兩次坑後的第三次重寫。前兩版被擱在這裡當化石，**不執行、不修改**，但可挖。

## 目錄

| 路徑 | 是什麼 | 為什麼還在 |
|---|---|---|
| [`ts-src/`](ts-src/) | **v1** — TypeScript prototype（Tauri app 殼 + sidecar 後端） | iter-8 / iter-9 累積的 sandbox argv / stream-parser schema / `enrichSourceMetadata` invariant 教訓寫在 comments + tests 裡，v2 / v3 carry-over 時要 grep 這裡 |
| [`v2-rust/`](v2-rust/) | **v2** — Rust CLI rewrite，phase 1 ship 後 path D pivot 失敗 | v2 完整 codebase + openspec history + strategy doc 都在。v3 開新 change 時要 grep v2 對應 module 跟 spec，不靠記憶猜 |

## v1 怎麼死的（ts-src）

最早從前端切入做 Tauri app 殼，後端跟著 sidecar 拼上去。前端走太快、後端跟不上 — 餅畫太大沒先驗證 backend 可行性，反覆修改、sidecar 讓整體行為複雜化，最後放棄。但 iter-8 / iter-9 累積的硬底子（sandbox argv 怎麼下、stream-json 怎麼 parse、enrichSourceMetadata invariant）全留下來，是 v2 的起點。

## v2 怎麼死的（v2-rust）

撇除前端、純 CLI 做完 phase 1：5-folder taxonomy、`--tools` whitelist sandbox、auto-lint、source enrichment、PII filter、lint feedback loop、token tracking、Obsidian-clickable wikilinks。phase 1 收尾後 2026-05-08 一場 strategy 討論（[`v2-rust/docs/strategy/2026-05-08-skill-vs-binary-pivot.md`](v2-rust/docs/strategy/2026-05-08-skill-vs-binary-pivot.md)）浮出 4 條路；第一次嘗試 path D（pivot 成 Claude Code skill）失敗、revert，歸納出 4 條 anti-patterns 後重啟成 v3。

紀律就是從 v2 失敗的 first-attempt 萃出來的：no speculative single-impl trait、no schema double-ship、carry-over 前先 grep v2、`/spectra-apply` 不亂 checkpoint。

## 規則

- **別跑** — `ts-src/` 沒在 build / test / ship；`v2-rust/` 也不是 codebus 入口
- **別改** — 兩個資料夾都不再演進，改了沒人會驗
- **可挖** — v3 開新 change 前，先 grep 對應 v2 module / spec，看 v2 怎麼處理同 edge case
