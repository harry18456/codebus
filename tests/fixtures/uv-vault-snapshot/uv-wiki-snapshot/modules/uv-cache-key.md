---
title: uv-cache-key crate
type: module
sources:
  - path: crates/uv-cache-key/Cargo.toml
    sha256: 8a3a411361752ff21319940c1bce60ab788d588af95c0fb7951d3f87ff4cd5c3
    at_commit: a1c90c1fa12c95485f3d6a210daa4e6cc7466a90
  - path: crates/uv-cache-key/src/lib.rs
    sha256: f3d9412a813790d854cb202d0c0546ba3cfa1468b00c9525ec2150448ecb4618
    at_commit: a1c90c1fa12c95485f3d6a210daa4e6cc7466a90
  - path: crates/uv-cache-key/src/cache_key.rs
    sha256: b36c4dfdaab274f14c6d46d6a812599f7cabb254c2e0d66e66684a36cfe93b57
    at_commit: a1c90c1fa12c95485f3d6a210daa4e6cc7466a90
  - path: crates/uv-cache-key/src/canonical_url.rs
    sha256: 6de648afe833384da8d005cb50841e7128044186f53694d93794b53e5320ef0c
    at_commit: a1c90c1fa12c95485f3d6a210daa4e6cc7466a90
  - path: crates/uv-cache-key/src/digest.rs
    sha256: b2de0c4ba065604de52edafd6244670dfdc355cace578e39102cfd446ad30b5f
    at_commit: a1c90c1fa12c95485f3d6a210daa4e6cc7466a90
goals:
  - 了解 uv-cache-key 的設計目標跟它對外的 API 表面
created: '2026-05-05'
updated: '2026-05-05'
related:
  - '[[cache-key-trait]]'
  - '[[canonical-url]]'
stale: false
---

# uv-cache-key crate

`uv-cache-key` 是 uv 專案的內部 crate，集中處理「為了當作快取鍵 (cache key) 而對任意值產出穩定指紋」這件事。它對外的 API 表面非常薄 —— `lib.rs` 只 re-export 6 個 symbol。

## 設計目標

1. **跨版本、跨平台穩定**：標準函式庫的 `Hash` 不保證同一個值在不同版本或不同平台會產生相同 hash，因此 uv 用一個自訂的 [`CacheKey`] trait 顯式重新實作 hashing 行為。詳見 [[cache-key-trait]]。
2. **URL 在比對前先標準化**：使用者寫 `github.com/foo/bar` 跟 `github.com/foo/bar.git` 的時候，uv 應該把它們視為同一個來源。`CanonicalUrl` 與 `RepositoryUrl` 把這種「同一資源不同寫法」收斂到同一個鍵。詳見 [[canonical-url]]。
3. **產出可當作檔案/目錄名的指紋字串**：cache 經常需要把 hash 拼進路徑，所以 [`cache_digest`] 與 [`hash_digest`] 直接輸出十六進位字串。
4. **內部使用、不保證 SemVer**：`README.md` 明確聲明此 crate 是 uv 的 internal component，API 會頻繁 breaking change，不適合外部直接依賴。

## 對外 API 表面

從 `lib.rs` re-export，全部 6 項：

```rust
// from crates/uv-cache-key/src/lib.rs
pub use cache_key::{CacheKey, CacheKeyHasher};
pub use canonical_url::{CanonicalUrl, RepositoryUrl};
pub use digest::{cache_digest, hash_digest};

mod cache_key;
mod canonical_url;
mod digest;
```

| 名稱 | 種類 | 角色 |
|------|------|------|
| [`CacheKey`] | trait | 自訂 hashing 介面，覆蓋常見型別 (整數家族、`String`、`Path`、`Url`、`Vec<T>`、`BTreeMap`、`Option<T>`、tuple ...) |
| [`CacheKeyHasher`] | struct | 包裝 `seahash::SeaHasher`，在 [`Hasher`] 介面上把 bytes 餵進 SeaHash |
| [`CanonicalUrl`] | struct | URL 規範化形式：去掉 credentials、`.git` 後綴、trailing slash、percent-encoding |
| [`RepositoryUrl`] | struct | 在 `CanonicalUrl` 之上再去掉 ref / fragment / query，可選地以 LFS bit 區分 |
| [`cache_digest`] | fn | `(&impl CacheKey) -> String` 十六進位指紋，餵 `CacheKeyHasher` |
| [`hash_digest`] | fn | `(&impl Hash) -> String` 十六進位指紋，餵原生 `SeaHasher` |

## 依賴

```toml
# from crates/uv-cache-key/Cargo.toml
[dependencies]
uv-redacted = { workspace = true }

hex = { workspace = true }
memchr = { workspace = true }
percent-encoding = { workspace = true }
seahash = { workspace = true }
url = { workspace = true }
```

關鍵點：

- **`seahash`**：所有 hashing 最終都走 SeaHash，這是個非加密、速度快、有公開 spec 的演算法，是「跨版本穩定」的承諾基礎。
- **`uv-redacted::DisplaySafeUrl`**：`CanonicalUrl` 與 `RepositoryUrl` 都儲存 `DisplaySafeUrl` 而不是 `url::Url`，避免日誌或錯誤訊息誤洩 credentials。
- **`memchr` + `percent-encoding`**：用在 URL 路徑的 percent-decoding fast-path（先用 `memchr` 找 `%` 符號再決定要不要 decode）。
- **`hex`**：`digest.rs` 用它把 `u64` 轉成 16 個字元的小寫十六進位字串。

## 內部結構

`lib.rs` 只負責 re-export，沒有任何 logic。三個子模組各自獨立：

- `cache_key.rs` — trait 與 hasher 包裝。
- `canonical_url.rs` — URL 兩階段標準化。
- `digest.rs` — 把 hash 轉成 hex 字串。

`Cargo.toml` 設 `doctest = false`，所以文件區塊裡的範例不會被 cargo test 跑到。
