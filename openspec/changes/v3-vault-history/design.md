## Context

v3-init #2 archive 把「不建 nested `.git/`」明確寫進 Vault Layout requirement（[v3-init proposal:9, 54](file:///D:/side_project/codebus/openspec/changes/archive/2026-05-08-v3-init/proposal.md)），原本理由是「vault diff 歷史是 future change 的事」。但 #5 v3-goal 跟 #8 v3-fix spawn 收尾都需要 `auto_commit` API 把 wiki 變動寫進 vault repo —— 這條依賴讓 vault-history 從原 follow-up 升級為 #5/#8 的 prerequisite，2026-05-09 roadmap 已重排為主序列 #4（[commit ccca618](file:///D:/side_project/codebus/docs/v3-roadmap.md)）。

v2 已有完整 `nested_repo.rs` 實作（[legacy/v2-rust/codebus-core/src/git/nested_repo.rs](file:///D:/side_project/codebus/legacy/v2-rust/codebus-core/src/git/nested_repo.rs)）：`init_nested_repo` 跑 `git init -b main` + 設 local `user.email=codebus@local` / `user.name=codebus`，`auto_commit` 跑 `git add -A` + `git commit -m`。本 change 直接 carry，無實質設計變動。

## Goals / Non-Goals

**Goals:**

- init 結尾的 `.codebus/` 是合法 git repo，HEAD 指向第一個 commit（內含全部 init artifacts）
- 公開 `auto_commit` API 給 #5 v3-goal / #8 v3-fix 在 spawn 收尾呼叫
- vault-internal `.gitignore` 排除 `.lock` / `raw/code/` / `**/.obsidian/` / `logs/`（commit 只記 wiki 演化，不記 raw mirror 重複內容）
- nested repo 的 commit author 跟 user 全域 git config 解耦（local override `codebus@local`）

**Non-Goals:**

- spawn 收尾 `auto_commit` wire（這是 #5 / #8 各自的事）
- nested repo 的衝突解決 / branch / remote / push / GC 等 git lifecycle policy
- user 手動編輯 vault 後的 commit policy（vault-history 只 commit init 結尾的 snapshot；運行期間的 user 編輯靠後續 verb 收尾 commit 收口）
- source repo git config 透傳到 nested repo（commit author 永遠是 `codebus@local`）
- 既有 archived v3-init / v3-pii 的 vault re-init 行為變更通知（既有 vault re-跑 init 會自動 promote 進 nested git tracking，但 v3 仍是 0.3.0-dev 階段、無外部 user，無需 migration doc）

## Decisions

### nested repo author hardcode `codebus@local` / `codebus`

不繼承 user 全域 `~/.gitconfig` 的 `user.name` / `user.email`。

**Rationale**：

- 不依賴 user 全域 config 才能跑（CI / fresh container / 沒設 git config 的 dev box 也可用）
- nested repo 的 commit 是 codebus 自動產出、不是 user 手動 commit，作者署名 `codebus@local` 比 user 真名更貼近事實
- v2 carry 同樣設計

**Alternatives considered:**

- (A) 透過 `--get` 讀 user 全域，fall back to `codebus@local`：增加分支邏輯、user 全域 config 變動會影響 vault commit 一致性
- (B) 從 source repo `.git/config` 讀：source repo 不是 git repo 時 fall back 困難；commit author 跟 source repo author 同步沒實質好處

### init 流程順序：raw_sync → internal gitignore → nested repo init → 後續產物 → 收尾 auto_commit

```
1. create_vault_layout (mkdir .codebus + 7 子目錄)
2. sync_with_scanner (raw mirror; PII warn)
3. ensure_codebus_internal_gitignore (.codebus/.gitignore)        ← 新增
4. init_nested_repo (.codebus/.git, git config local)             ← 新增
5. ensure_codebus_in_source_gitignore (source repo .gitignore)
6. write_schema_if_missing (.codebus/CLAUDE.md)
7. compute_source_signal + write_or_update_manifest
8. write_skill_bundles
9. register_vault (obsidian, optional)
10. auto_commit "init: codebus vault"                             ← 新增
```

**Rationale**：

- step 3 在 step 4 之前，這樣 step 4 的 `git init -b main` + 後續 `auto_commit` 一啟動就帶著正確 ignore 規則，`raw/code/` 不會被 stage 進第一個 commit
- step 10 放最後，第一個 commit 內含所有 init artifact 的最終狀態（含 manifest 的 `last_sync_at` 時間戳、skill bundles 內容等），代表「這份 vault 已 init 完整」的 snapshot

**Alternatives considered:**

- (A) auto_commit 放 step 4 之後立刻跑：第一個 commit 只有 raw mirror + internal gitignore，後續 schema / manifest / skill bundles 變成 untracked → 第二次 init 才會 commit 進去，HEAD 跟 vault 實際狀態不一致
- (B) 多次小 commit（每階段一個 commit）：commit 量過多無實質幫助；user 看到 vault 第一次 init 期待「init: codebus vault」一個 commit 比較直觀

### `auto_commit` 失敗 = init 失敗

`auto_commit` 走 `io::Result<String>` 傳遞 error，呼叫端 init.rs map_err 後 `return ExitCode::from(1)`。

**Rationale**：commit 是 init 完整性 invariant 的一部分，commit 失敗代表 nested repo 狀態不可預測，後續 verb 在這個 vault 上 spawn 都可能踩雷。Fail loudly 比 silent skip 安全。

### 既有 .codebus/ 重跑 init 自動 promote 進 nested tracking

`init_nested_repo` 對已存在 `.git/` 是 no-op；對不存在 `.git/` 的既有 vault（archived v3-init / v3-pii 落地過的 vault）會新建 nested repo。第一次 `auto_commit` 把整個 vault 當前內容當 first commit。

**Rationale**：v3 仍 0.3.0-dev、無外部 user 部署；既有 vault re-init 後自動有 git 歷史是 ergonomics 改善，不需 migration doc。

## Risks / Trade-offs

- [Risk] user 系統沒裝 `git` binary → init 失敗於 step 4 → Mitigation：error message 帶「is `git` installed and in PATH?」hint；spec scenario 不覆蓋（CI 一律有 git，dev box 沒裝極罕見）
- [Risk] `.codebus/.gitignore` 4 行內含 `**/.obsidian/`，user 若把 vault 從 obsidian 切到別的工具（例如 logseq）想 commit obsidian config 也會被擋 → Mitigation：v2 同樣行為、accept；v3-config #9 未來可加 `vault.gitignore_extra` 開放 user 自訂
- [Trade-off] commit author `codebus <codebus@local>` 對 user 看 git log 可能困惑（不是自己的名字）→ 接受；comment block 跟 schema 文件提一下「nested repo author intentionally decoupled」

## Migration Plan

不適用 — 既有 vault re-init 自動 promote，無需手動操作。

## Open Questions

無待解 —— 所有設計問題都在前面 thread 對齊，v2 carry 直接落地。
