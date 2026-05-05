---
title: 了解 uv-cache-key 的設計目標跟它對外的 API 表面
goal: "了解 uv-cache-key 的設計目標跟它對外的 API 表面"
created: '2026-05-05'
updated: '2026-05-05'
---

# Reading guide: uv-cache-key 的設計目標跟它對外的 API 表面

## TL;DR

`uv-cache-key` 是個小但聚焦的 crate，三層 API 環環相扣：

1. [`CacheKey`] trait + [`CacheKeyHasher`] 提供「穩定可跨平台」的 hashing 介面（不同於 `std::hash::Hash`）。
2. [`CanonicalUrl`] / [`RepositoryUrl`] 把語意相同但字面不同的 URL 收斂到同一個 hash。
3. [`cache_digest`] / [`hash_digest`] 把 `u64` hash 輸出成 16 字元小寫十六進位字串，方便當作檔名片段使用。

## 建議閱讀順序

1. [[uv-cache-key]] — 從模組頁出發，掃過 `lib.rs` 的 6 個 re-export，建立 API 表面的全貌。
2. [[cache-key-trait]] — 理解為什麼不能直接用 `std::hash::Hash`，以及 `CacheKeyHasher` 跟 `seahash::SeaHasher` 的關係。
3. [[canonical-url]] — 進入兩個 URL entity，看 canonical 化規則與 `RepositoryUrl` 在它之上多做了哪些事。

## 關鍵問題與答案

- **設計目標是什麼？**
  - 跨版本、跨平台穩定的 hashing。
  - URL 在比對前先做語意標準化。
  - 把 hash 變成可放進路徑的字串。
  - 內部使用，README 明示 API 不保證 SemVer。
- **API 表面有多大？** 6 個 symbol：2 個 hashing 設施、2 個 URL 包裝、2 個 digest 函式。
- **為什麼 `CanonicalUrl` 不對外暴露字串？** 註解明確：canonical 形式只用於比對，fetch 一律用原始 URL，避免誤把標準化後的 URL 拿去打 server。
- **為什麼有 `RepositoryUrl` 又有 `CanonicalUrl`？** 區分粒度不同：`CanonicalUrl` 區分到 commit / subdirectory；`RepositoryUrl` 區分到 repo（搭配可選的 LFS bit）。
- **為什麼只支援 `BTreeMap` / `BTreeSet`，沒有 `HashMap` / `HashSet`？** 因為後者迭代順序不穩定，當 cache key 會炸 —— 這是有意的省略。

## 主要 source 對照

| 檔案 | 重點 |
|------|------|
| `crates/uv-cache-key/src/lib.rs` | facade，6 個 re-export |
| `crates/uv-cache-key/src/cache_key.rs` | `CacheKey` trait、覆蓋多種型別、`CacheKeyHasher` 包裝 SeaHasher |
| `crates/uv-cache-key/src/canonical_url.rs` | `CanonicalUrl::new` 七步標準化、`RepositoryUrl::new` 再砍 ref/fragment/query、底部測試清單列出所有不變量 |
| `crates/uv-cache-key/src/digest.rs` | `cache_digest` 走 `CacheKey`、`hash_digest` 走 `Hash`，輸出 hex 字串 |
| `crates/uv-cache-key/Cargo.toml` | 依賴：`seahash`（穩定 hash）、`uv-redacted`（DisplaySafeUrl）、`hex`、`memchr`、`percent-encoding`、`url` |
| `crates/uv-cache-key/README.md` | 標明 internal、SemVer 不穩 |
