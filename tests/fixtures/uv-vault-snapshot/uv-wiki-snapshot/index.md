---
title: Wiki index
updated: '2026-05-05'
---

# Wiki index

## Modules

- [[uv-cache-key]] — uv 內部 crate，提供穩定的 cache-key hashing 與 URL 標準化。
- [[uv-cache-info]] — uv 內部 crate，蒐集 source distribution 的環境快照判斷是否需要 rebuild。
- [[uv-resolver]] — uv 的 PubGrub 為核心相依性解析器，crate 級別總覽與公開 API 索引。
- [[uv-installer]] — uv 的 wheel 安裝 crate：把 resolution 拆成 Plan / Prepare / Install 三段，永遠從 cache link 到 venv。

## Concepts

- [[cache-key-trait]] — 自訂 `CacheKey` trait + `CacheKeyHasher` 為什麼存在、怎麼運作。
- [[cache-info-vs-cache-key]] — `uv-cache-info` 跟 `uv-cache-key` 的職責對比、撞名陷阱、如何在 `uv-cache` / `uv-distribution` 串連。
- [[resolver-provider]] — `ResolverProvider` trait + `DefaultResolverProvider`：把 resolver 的所有 IO 抽出去的接縫，含 `Reporter`。

## Entities

- [[canonical-url]] — `CanonicalUrl` / `RepositoryUrl` 兩個 URL 包裝型別與其標準化規則。
- [[cache-info]] — `CacheInfo` struct 與 `cache-keys` 設定 enum 的欄位與序列化結構。
- [[resolver-manifest]] — `Manifest`：resolver 的主要輸入容器（requirements / constraints / overrides / preferences …）。
- [[resolver-options]] — `Options` / `OptionsBuilder` / `Flexibility`：resolver 的策略開關。
- [[resolver-output]] — `ResolverOutput` 與 `AnnotatedDist`：成功解析後的 graph 表示。

## Processes

- [[resolver-resolve]] — `Resolver::new` / `resolve()` 雙線程解析流程：tokio fetcher × OS thread 上的 PubGrub solver。
- [[wheel-install-pipeline]] — `Planner` → `Preparer` → `Installer`：cache 在三段安裝流程中的角色（查、填、link）。

## Synthesis

(尚無)

## Goals

- [[uv-cache-key-design]] — 了解 uv-cache-key 的設計目標跟它對外的 API 表面。
- [[uv-cache-info-and-uv-cache-key]] — 了解 uv-cache-info 怎麼跟 uv-cache-key 互動。
- [[uv-resolver-core-api]] — 了解 uv-resolver 的核心 API 跟 entry point。
- [[uv-installer-cache-install]] — 了解 uv-installer 怎麼用 cache 裝 wheel。
