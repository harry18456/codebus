---
title: uv-cache-info 跟 uv-cache-key 的關係
type: concept
sources:
  - path: crates/uv-cache-info/Cargo.toml
    sha256: 971e2e145b6122d17ae2e8732d9a2bafa99eb7c7c9187669e911aabdbb040d7d
    at_commit: a1c90c1fa12c95485f3d6a210daa4e6cc7466a90
  - path: crates/uv-cache-info/src/cache_info.rs
    sha256: b877f39ea29ce5bf442af8c95be641336a8dce3ee29dcc82faadac87c3d18880
    at_commit: a1c90c1fa12c95485f3d6a210daa4e6cc7466a90
  - path: crates/uv-cache-key/src/cache_key.rs
    sha256: b36c4dfdaab274f14c6d46d6a812599f7cabb254c2e0d66e66684a36cfe93b57
    at_commit: a1c90c1fa12c95485f3d6a210daa4e6cc7466a90
  - path: crates/uv-cache/Cargo.toml
    sha256: 0c97120b11dbe8ad8c484acdd4ac245f6fda68d3e183ff984896c242ae2b0c0b
    at_commit: a1c90c1fa12c95485f3d6a210daa4e6cc7466a90
  - path: crates/uv-cache/src/wheel.rs
    sha256: fc4dadb75d7e99f38b8e9913d7d404d904f6a7acb104e15ba44d80bf2823cdd8
    at_commit: a1c90c1fa12c95485f3d6a210daa4e6cc7466a90
  - path: crates/uv-cache/src/lib.rs
    sha256: 3c827e1f486cea0edb1962e30b833eb36712c3826fb2153bf6f5c18e8b62aed3
    at_commit: a1c90c1fa12c95485f3d6a210daa4e6cc7466a90
  - path: crates/uv-distribution/src/index/built_wheel_index.rs
    sha256: bad42823b7bc6ef04c3115e54ba000fb7fde99a3f3d360b5537873b16d520ea6
    at_commit: a1c90c1fa12c95485f3d6a210daa4e6cc7466a90
goals:
  - 了解 uv-cache-info 怎麼跟 uv-cache-key 互動
created: '2026-05-05'
updated: '2026-05-05'
related:
  - '[[uv-cache-info]]'
  - '[[uv-cache-key]]'
  - '[[cache-info]]'
  - '[[cache-key-trait]]'
stale: false
---

# uv-cache-info 跟 uv-cache-key 的關係

**TL;DR：兩者沒有直接互動。** `uv-cache-info` 的 `Cargo.toml` 不依賴 `uv-cache-key`，反過來也一樣。它們是 uv cache 機制裡兩條獨立的軸，各自有一個叫 `CacheKey` 的型別，但意義跟用途完全不同。在 build graph 上要爬到 `uv-cache` 才看到兩者並存。

## 對照表

|                 | [[uv-cache-key]] | [[uv-cache-info]] |
|-----------------|------------------|-------------------|
| 解決的問題      | 給定相同輸入，怎麼算出**穩定的指紋字串**？ | 給定 source 目錄，**它變了沒有**？ |
| 主要型別        | `CacheKey` **trait** + `CacheKeyHasher` + `CanonicalUrl` | `CacheInfo` **struct** + `CacheKey` **enum** + `Timestamp` |
| 輸出形式        | 16 hex chars (`u64` little-endian) | 結構化資料：timestamp / git commit / tags / env / dirs |
| 失效判斷方式    | 比較 hex 字串相等 | `==` / `!=` 結構比對 (`PartialEq` / `Eq`) |
| 何時計算        | cache 路徑形成時、cache 命中查找時 | 建構前蒐集、寫入 pointer；讀 cache 時重算+比對 |
| 用在哪裡        | wheel cache 目錄名、URL→shard 映射 | source distribution 是不是 stale 的判斷依據 |
| 跨版本/平台穩定？| 是（SeaHash + 自訂 trait 顯式控制） | 是（用 `BTreeMap` 而非 `HashMap`，欄位都是 serde） |

## 兩個 `CacheKey` 的職責對比

兩邊都叫 `CacheKey`，是這個 goal 的最大陷阱：

```rust
// uv-cache-key
// from crates/uv-cache-key/src/cache_key.rs
pub trait CacheKey {
    fn cache_key(&self, state: &mut CacheKeyHasher);
    // ...
}
```

```rust
// uv-cache-info
// from crates/uv-cache-info/src/cache_info.rs
pub enum CacheKey {
    Path(Cow<'static, str>),
    File { file: Cow<'static, str> },
    Directory { dir: Cow<'static, str> },
    Git { git: GitPattern },
    Environment { env: String },
}
```

- `uv_cache_key::CacheKey` 是「**任何要被 hash 進 cache 鍵的東西**都該實作的 trait」，類似 `Hash` 但保證跨平台穩定。詳見 [[cache-key-trait]]。
- `uv_cache_info::CacheKey` 是「**`pyproject.toml` 的 `[tool.uv].cache-keys` 陣列**裡每一條設定」的反序列化型別，是設定 schema 的一部分。

兩者都帶有「我用來標識某個東西的等同性」這個共通語義，所以名字互相吸引，但實作層級沒有任何關聯：trait 沒有為 enum 實作 trait、enum 沒有用到 hasher、兩個 crate 互不依賴。

## 它們其實在哪裡相遇？

直接相遇處只有一個：**`uv-cache` crate 同時 use 兩者**，但用法上井水不犯河水。

```toml
# from crates/uv-cache/Cargo.toml (節錄)
uv-cache-key = { workspace = true }
uv-cache-info = { workspace = true }
```

各自被誰用：

```rust
// from crates/uv-cache/src/wheel.rs
use uv_cache_key::{CanonicalUrl, cache_digest};
// 用來把 URL 算成 hex 指紋當 cache 路徑

// from crates/uv-cache/src/lib.rs (簡化)
use uv_cache_info::Timestamp;
// 用來判斷 cache entry 的時間戳

// from crates/uv-cache/src/by_timestamp.rs
use uv_cache_info::Timestamp;
// 同上
```

也就是說：

- `uv-cache-key` → 形成 cache 路徑（**WHERE** 存）。
- `uv-cache-info` → 判斷 cache 內容是否還新鮮（**WHEN** 重算）。

兩者都是 cache 機制的零件，但職責正交，通常不會在同一行程式碼裡互動。

## 在更上層 `uv-distribution` 的協同

`uv-distribution` 把兩條軸串起來看更明顯：

```rust
// from crates/uv-distribution/src/index/built_wheel_index.rs
let cache_shard = self.cache.shard(
    CacheBucket::SourceDistributions,
    WheelCache::Path(&source_dist.url).root(),  // ← 這裡走 uv-cache-key 的 cache_digest
);

let Some(pointer) = LocalRevisionPointer::read_from(cache_shard.entry(LOCAL_REVISION))?
else {
    return Ok(None);
};

// 這裡走 uv-cache-info 的 from_directory + PartialEq
let cache_info = CacheInfo::from_directory(&source_dist.install_path)?;
if cache_info != *pointer.cache_info() {
    return Ok(None);
}
```

兩步流程：

1. **定位 cache shard**：用 `WheelCache::Path(url).root()`，內部呼叫 `cache_digest(&CanonicalUrl::new(url))` —— 這是 `uv-cache-key` 的事。
2. **驗證 cache 還新鮮**：在那個 shard 讀出 pointer，把 pointer 裡序列化的 `CacheInfo` 跟現在算出來的 `CacheInfo` 用 `!=` 比 —— 這是 `uv-cache-info` 的事。

兩個 cache key 概念**串連而非耦合**：先用 `uv-cache-key` 找到位置，找到了再用 `uv-cache-info` 確認新鮮度。彼此不知道對方的存在。

## 為什麼要刻意分開？

從 crate 切割可以看出設計意圖：

- **`uv-cache-key` 的輸入是純值**（URL、字串、路徑），輸出是 hex 字串。它需要「算出來的東西在每台機器、每個版本都一致」。
- **`uv-cache-info` 的輸入是檔案系統 + git + 環境變數**，輸出是結構化資料。它需要「捕捉所有可能讓建構結果改變的環境因素」，但不需要把它們塞進同一個 hasher。

如果硬把它們合併，會讓 `uv-cache-key` 不再是純函式（要碰檔案系統），或讓 `uv-cache-info` 失去結構化、可序列化、可人工檢視的優點（變成單一 hash 黑盒）。所以兩者刻意保持平行，由 [[uv-cache-key|uv-cache]]/`uv-distribution` 等更上層 crate 在使用點上組合。
