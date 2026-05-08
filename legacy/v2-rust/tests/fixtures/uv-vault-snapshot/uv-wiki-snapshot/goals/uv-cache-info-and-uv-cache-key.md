---
title: 了解 uv-cache-info 怎麼跟 uv-cache-key 互動
goal: 了解 uv-cache-info 怎麼跟 uv-cache-key 互動
created: '2026-05-05'
updated: '2026-05-05'
---

# 了解 uv-cache-info 怎麼跟 uv-cache-key 互動

## 一句話結論

兩者**沒有直接互動**：`uv-cache-info/Cargo.toml` 不依賴 `uv-cache-key`，反向也一樣。它們是 uv cache 機制裡正交的兩條軸 —— `uv-cache-key` 算「cache 放哪」，`uv-cache-info` 算「cache 還新鮮嗎」 —— 由 `uv-cache` / `uv-distribution` 在使用點上串接。

## 推薦閱讀順序

1. **[[cache-info-vs-cache-key]]** — 直接給出對照表跟撞名陷阱說明，先看這頁建立全景。
2. **[[uv-cache-info]]** — 補上 `uv-cache-info` 這個 crate 本身的細節：API 表面、依賴、`from_directory` 流程、下游怎麼用。
3. **[[cache-info]]** — 想看 `CacheInfo` struct 各欄位、`cache-keys` 設定 enum 五個 variant 的細節再進這頁。
4. **[[uv-cache-key]]** + **[[cache-key-trait]]** — 對照組，已存在；同時複習 `cache_digest` / `CacheKey` trait 來看出兩邊命名為什麼撞車但職責不同。

## 重點 takeaway

- **`uv_cache_info::CacheKey` 是 enum，`uv_cache_key::CacheKey` 是 trait。** 命名衝突純粹靈感相近，不是設計關聯。
- **`CacheInfo` 用 `==` / `!=` 失效判斷，不用 hash。** 雖然 derive 了 `Hash`，但實際 cache 比對都是結構等價（`PartialEq` / `Eq`）。
- **唯一跟 cache 路徑相關的 hashing 在 `uv-cache-key` 那邊**：`WheelCache::root()` 用 `cache_digest(&CanonicalUrl::new(url))` 算出 hex 路徑，跟 `CacheInfo` 完全分離。
- **串接點在 `uv-distribution`**：`built_wheel_index.rs` 先用 `WheelCache::Path(url).root()` (走 uv-cache-key) 找 shard，再讀 pointer 裡的 `CacheInfo` 跟現場重算結果用 `!=` 比對 (走 uv-cache-info)。

## 探索過的源碼

| 路徑 | 看了什麼 |
|------|----------|
| `crates/uv-cache-info/Cargo.toml` | 確認沒有 `uv-cache-key` 依賴 |
| `crates/uv-cache-info/src/lib.rs` | 確認對外只 re-export `cache_info` + `timestamp` |
| `crates/uv-cache-info/src/cache_info.rs` | `CacheInfo` struct、`CacheKey` enum、`from_directory` 流程 |
| `crates/uv-cache-info/src/timestamp.rs` | Unix `ctime` vs 其他平台 `mtime` 的選擇 |
| `crates/uv-cache-info/src/git_info.rs` | `Commit` / `Tags` 從 `.git/HEAD` + `.git/refs/tags` 讀取 |
| `crates/uv-cache-info/src/glob.rs` | `cluster_globs` 用 trie 把 glob 依 LCP 分組 |
| `crates/uv-cache/Cargo.toml` | 確認 `uv-cache` 同時 depend 兩者 |
| `crates/uv-cache/src/wheel.rs` | `WheelCache::root()` 用 `cache_digest` 算路徑 |
| `crates/uv-distribution/src/index/built_wheel_index.rs` | 兩條軸的串接點 |
| `crates/uv-distribution-types/src/cached.rs` / `installed.rs` | `cache_info: CacheInfo` 欄位被掛在 `CachedDist` / `InstalledDist` |
