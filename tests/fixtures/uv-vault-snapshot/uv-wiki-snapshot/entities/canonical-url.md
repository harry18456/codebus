---
title: CanonicalUrl 與 RepositoryUrl
type: entity
sources:
  - path: crates/uv-cache-key/src/canonical_url.rs
    sha256: 6de648afe833384da8d005cb50841e7128044186f53694d93794b53e5320ef0c
    at_commit: a1c90c1fa12c95485f3d6a210daa4e6cc7466a90
goals:
  - 了解 uv-cache-key 的設計目標跟它對外的 API 表面
created: '2026-05-05'
updated: '2026-05-05'
related:
  - '[[uv-cache-key]]'
  - '[[cache-key-trait]]'
stale: false
---

# CanonicalUrl 與 RepositoryUrl

兩個 URL 包裝型別，都包在 `uv_redacted::DisplaySafeUrl` 之上、刻意不對外暴露原始字串（`pub` 欄位是 0），用途是「讓字面上不同但語意相同的 URL 在 cache key 上對齊」。

## 設計動機

註解寫得很白：

> A "canonical" url is only intended for internal comparison purposes. It's to help paper over mistakes such as depending on `github.com/foo/bar` vs. `github.com/foo/bar.git`.

> This is **only** for internal purposes and provides no means to actually read the underlying string value of the `Url` it contains. This is intentional, because all fetching should still happen within the context of the original URL.

→ canonical 形式只用來「比對／當 cache key」，**不要拿去 fetch**；fetch 一律用使用者原本給的 URL。

## CanonicalUrl 的標準化規則

依照 `CanonicalUrl::new` 的順序：

1. 若 `cannot_be_a_base()` 則原樣回傳（這種 URL 沒辦法做 path 操作）。
2. **去掉 credentials**：`set_password(None)` + `set_username("")`。
3. **去掉 trailing slash**：`pop_if_empty()`。
4. **GitHub 特例**：若 host 是 `github.com`，把 scheme 與 path 全部 lowercase（GitHub 本身就是 case-insensitive；註解承認這個解法不夠通用，引用 issue #84）。
5. **`.git` 後綴去除**：分兩種情況處理 ——
   - path 內含 `@`（例如 `…packages.git@2.0.0`）：把 `@` 前的 `.git` 砍掉。
   - 沒有 `@`：把最後一個 segment 的 `.git` 砍掉。
6. **Percent-decode**：先用 `memchr::memchr(b'%', ...)` 偵測有沒有 `%`，有的話對每個 path segment decode。

關鍵：subdirectory 在 fragment、commit ref 在 path 的 `@` 後綴，這些 **保留**，所以 `…@v1.0.0` 跟 `…@v2.0.0` 會是不同的 `CanonicalUrl`、`#subdirectory=pkg_a` 跟 `#subdirectory=pkg_b` 也不同。測試清單把這些對照寫得很清楚（`canonical_url` 測試，line 282–368）。

```rust
// from crates/uv-cache-key/src/canonical_url.rs
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct CanonicalUrl(DisplaySafeUrl);
```

提供的方法：

- `new(&DisplaySafeUrl) -> Self` — 從已經 parse 好的 `DisplaySafeUrl` 建構。
- `parse(&str) -> Result<Self, DisplaySafeUrlError>` — 字串入口。
- `From<CanonicalUrl> for DisplaySafeUrl` — 取出底層 URL（注意：這違反「不對外暴露字串」的原則，但給了一條 escape hatch）。
- `Display`、`Hash`、`CacheKey` — 都走 `self.0.as_str()`，避免被 `url` crate 的 `Hash` 實作變動影響。

## RepositoryUrl：再抽象一層

```rust
// from crates/uv-cache-key/src/canonical_url.rs
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct RepositoryUrl {
    repo_url: DisplaySafeUrl,
    with_lfs: Option<bool>,
}
```

`RepositoryUrl::new` 先呼叫 `CanonicalUrl::new`，然後在它之上 **再** 做：

1. 如果 scheme 以 `git+` 開頭，把 path 中 `@` 後的 ref 砍掉（去掉 commit / branch / tag）。
2. `set_fragment(None)` — 砍掉 `#subdirectory=...` 等 fragment。
3. `set_query(None)` — 砍掉 query。

結果是「**指向同一個 git 倉庫**」的 URL 都會收斂成同一個 `RepositoryUrl`，不論它們指向哪個 commit、哪個 subdirectory。

唯一的例外是 LFS：`with_lfs(Some(true))` 時 `cache_key` 會額外寫入 `1u8`，使得啟用 / 未啟用 LFS 的同一個倉庫產生不同 key。註解說明：

> The additional information it holds should only be used to discriminate between sources that hold the exact same commit in their canonical representation, but may differ in the contents such as when Git LFS is enabled.

`with_lfs(None)` 跟 `with_lfs(Some(false))` 會跟未設過 lfs 一樣 → 沒有額外 byte 寫入。只有 `Some(true)` 會改變 hash。

## 兩者對比

| 比較項目 | `CanonicalUrl` | `RepositoryUrl` |
|----------|----------------|-----------------|
| 去掉 credentials | ✅ | ✅（透過 CanonicalUrl） |
| 砍 `.git` 後綴 | ✅ | ✅（透過 CanonicalUrl） |
| 去 trailing slash | ✅ | ✅（透過 CanonicalUrl） |
| GitHub lowercase | ✅ | ✅（透過 CanonicalUrl） |
| Percent-decode | ✅ | ✅（透過 CanonicalUrl） |
| 砍 commit ref `@xxx` | ❌（保留） | ✅（git+ 前綴時） |
| 砍 fragment | ❌（保留） | ✅ |
| 砍 query | ❌（保留） | ✅ |
| LFS bit 影響 hash | ❌ | ✅（僅 `Some(true)`） |
| 實作了 `Deref<Target=Url>` | ❌ | ✅ |

語意上：`CanonicalUrl` 區分到 commit 級別、`RepositoryUrl` 區分到 repo 級別。

## CacheKey / Hash 實作的安全模式

兩者的 `CacheKey` 與 `Hash` 實作都遵守同一個模式 —— **永遠走 `as_str()`**：

```rust
// from crates/uv-cache-key/src/canonical_url.rs
impl CacheKey for CanonicalUrl {
    fn cache_key(&self, state: &mut CacheKeyHasher) {
        // `as_str` gives the serialisation of a url (which has a spec) and so insulates against
        // possible changes in how the URL crate does hashing.
        self.0.as_str().cache_key(state);
    }
}
```

註解直接說明動機：URL 的字串序列化是 spec 化的（WHATWG URL），但 `url` crate 的 `Hash` 實作是內部實作細節，可能改。走 `as_str()` 等於把 hashing 綁在 spec 上，跨版本穩定。

## 測試覆蓋的關鍵不變量

`canonical_url.rs` 底部的測試 (line 227–486) 是理解這個 entity 最快的入口，逐條比對「應該相等 / 不應相等」的 URL 對：

- `user_credential_does_not_affect_cache_key` — credentials 變化不影響 hash。
- `canonical_url` — `.git` 後綴等價、subdirectory 不等價、commit tag 不等價、percent-encoding 等價但 percent-encoded slash 不等價。
- `repository_url` — subdirectory / commit tag / LFS 都被收斂成同一個 repo。
- `repository_url_with_lfs` — fragment 被吃掉、`with_lfs(None/Some(false))` 不影響、`with_lfs(Some(true))` 才會分流。
