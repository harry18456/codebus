---
title: CacheKey trait 與穩定 hashing
type: concept
sources:
  - path: crates/uv-cache-key/src/cache_key.rs
    sha256: b36c4dfdaab274f14c6d46d6a812599f7cabb254c2e0d66e66684a36cfe93b57
    at_commit: a1c90c1fa12c95485f3d6a210daa4e6cc7466a90
  - path: crates/uv-cache-key/src/digest.rs
    sha256: b2de0c4ba065604de52edafd6244670dfdc355cace578e39102cfd446ad30b5f
    at_commit: a1c90c1fa12c95485f3d6a210daa4e6cc7466a90
goals:
  - 了解 uv-cache-key 的設計目標跟它對外的 API 表面
created: '2026-05-05'
updated: '2026-05-05'
related:
  - '[[uv-cache-key]]'
  - '[[canonical-url]]'
stale: false
---

# CacheKey trait 與穩定 hashing

`CacheKey` 是 uv 自訂的 hashing 介面，定位上等同於 Ruff 的 `CacheKey` trait（程式碼註解明確指出這個血緣）。它存在的理由只有一個：**`std::hash::Hash` 不保證跨版本、跨平台穩定**，但 cache 需要這個保證 —— 同一份輸入下次再 hash 一次，必須得到同一個鍵，否則整個 cache 會作廢。

## Trait 定義

```rust
// from crates/uv-cache-key/src/cache_key.rs
pub trait CacheKey {
    fn cache_key(&self, state: &mut CacheKeyHasher);

    fn cache_key_slice(data: &[Self], state: &mut CacheKeyHasher)
    where
        Self: Sized,
    {
        for piece in data {
            piece.cache_key(state);
        }
    }
}
```

跟 `std::hash::Hash` 的差別：

- 介面寫死 `&mut CacheKeyHasher`（不是泛型 `H: Hasher`），確保所有實作走的是同一個 hasher。
- 命名是 `cache_key` 而不是 `hash`，跟 `Hash` 並存而不互斥。
- `cache_key_slice` 模仿 `Hash::hash_slice`，提供同樣的批次特化掛點。

## CacheKeyHasher 內部就是 SeaHasher

```rust
// from crates/uv-cache-key/src/cache_key.rs
#[derive(Clone, Default)]
pub struct CacheKeyHasher {
    inner: SeaHasher,
}
```

`CacheKeyHasher` 同時 `impl Hasher`，把所有 `write_*` 都直接 forward 給 inner SeaHasher。SeaHash 是個有公開 spec 的非加密 hash，輸出在所有平台一致，這是穩定性的核心保證。

設計上有個小細節：很多型別（`str`、`String`、`Path`、`PathBuf`、`Url`）的 `cache_key` 實作其實是直接呼叫 `self.hash(&mut *state)` —— 因為 `CacheKeyHasher` 也是個 `Hasher`，它會把 bytes 餵給 SeaHasher，等同於繞道走 `Hash` trait 也達成同樣的穩定性。其他型別（整數、tuple、`Option`、collection）則手動列舉、一個 byte 都不漏掉。

## 涵蓋的型別

`cache_key.rs` 為以下型別提供了 `CacheKey` 實作：

- 純值：`bool`、`char`、整數家族 (`u8..u128`、`i8..i128`、`usize`、`isize`)、各種 `NonZero*`。
- 字串／路徑：`str`、`String`、`Path`、`PathBuf`。
- URL：`url::Url`（透過 `as_str()`，避免被上游 hash 實作的變動波及）。
- Tuple：`()` 到 12 元組（用 `impl_cache_key_tuple!` 巨集展開）。
- Container：`Option<T>`、`[T]`、`&T`、`&mut T`、`Vec<T>`、`BTreeSet<V>`、`BTreeMap<K, V>`、`Cow<'_, V>`。

可以注意：

- `[T]`、`Vec<T>`、`BTreeSet`、`BTreeMap` 都先寫入 `len()`，避免 `[a, b]` 跟 `[ab]` collision。
- `Option<T>` 寫入 0 / 1 區分 `None` / `Some`，跟 `Hash` 的慣例一致。
- 沒有 `HashMap` / `HashSet` 的實作 —— 因為它們的迭代順序不穩定，當 cache key 會炸。只接受 `BTreeMap` / `BTreeSet` 是設計上的有意取捨。

## 與 digest 的關係

`cache_digest` 把 trait + hasher 組合起來收斂成一個 `String`：

```rust
// from crates/uv-cache-key/src/digest.rs
pub fn cache_digest<H: CacheKey>(hashable: &H) -> String {
    fn cache_key_u64<H: CacheKey>(hashable: &H) -> u64 {
        let mut hasher = CacheKeyHasher::new();
        hashable.cache_key(&mut hasher);
        hasher.finish()
    }
    to_hex(cache_key_u64(hashable))
}

fn to_hex(num: u64) -> String {
    hex::encode(num.to_le_bytes())
}
```

`hash_digest` 是 `cache_digest` 的姊妹版，差別只在它接受任意 `Hash` 型別、繞過 `CacheKey` 而直接用 `SeaHasher`：

```rust
// from crates/uv-cache-key/src/digest.rs
pub fn hash_digest<H: Hash>(hashable: &H) -> String {
    fn hash_u64<H: Hash>(hashable: &H) -> u64 {
        let mut hasher = SeaHasher::new();
        hashable.hash(&mut hasher);
        hasher.finish()
    }
    to_hex(hash_u64(hashable))
}
```

兩個函式輸出都是 16 個字元的小寫十六進位字串（`u64` 的 little-endian 8 bytes，每 byte 兩字元），方便當作檔名／目錄名片段使用。

## 為什麼不直接用 `Hash`？

如果 uv 直接用 `std::hash::Hash` + `SeaHasher`，多數情況也會穩定 —— 但有幾個風險：

- 某些 std 型別（`HashMap` 等）的 `Hash` 實作可能變、或本來就依賴順序。
- 第三方 crate（例如 `url::Url`）的 `Hash` 實作可能跨版本改變。
- 自訂型別 `#[derive(Hash)]` 跟 cache 鍵不該放同一個介面，避免共用導致誤動。

`CacheKey` 把這些不確定性隔開，讓「能不能當 cache key」變成顯式的選擇而非 derive 的副作用。`CanonicalUrl` 對 `Url` 的處理（`as_str()` 的註解明確說「insulates against possible changes in how the URL crate does hashing」）就是這個哲學的具體體現。
