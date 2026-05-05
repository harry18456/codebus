## Context

CodeBus phase 1 用 TypeScript 實作，hexagonal 架構：`src/core/`（pure logic）、`src/infra/`（I/O：fs、git、claude-cli subprocess）、`src/ui/`（render、stream-parser、emoji-mode）、`src/commands/`（orchestration）。共 28 source files / 2067 LOC、152 tests、依賴 6 個 npm package（commander、gray-matter、js-yaml、simple-git、chalk、ora）。

當前狀態：
- **0.1.0 尚未 release**：npm 上沒有，無外部 user base
- **`main` working tree clean**、無 active spectra changes
- 一個 parked change `wiki-hygiene-signals`（0/18 task），來源是 `docs/spikes/2026-05-05-uv-validation/REPORT.md` 暴露的兩個 lint blind spot
- Long-term roadmap 升級：interactive tutorial 桌面 app（Tauri 實作）從「aspirational」變成 **day-1 committed**

主要約束：
- Tauri 桌面 app 會發生，且是核心產品形態（CLI 退化為 power-user / debug surface）
- 個人開發者單線開工、無團隊 review bottleneck
- iter-8 / iter-9 的 hard-won correctness 教訓（`--tools` sandbox 真相、stream-parser schema、enrichSourceMetadata invariant）必須在重寫時保留
- vault 格式跨語言相容是硬約束（`.codebus/wiki/**` 是純 markdown + git，不能變）

## Goals / Non-Goals

**Goals:**

- Rust workspace 取代 TS 實作，達到 CLI parity（4 個 subcommand、stdout / exit code byte-equal）
- 預先架好 Tauri 友善的 core lib boundary（`codebus-core` crate 同時被 CLI 與未來 Tauri app 共用）
- 吸收 parked change `wiki-hygiene-signals`，不另開重寫實作
- 統一 agent system prompt schema 來源（從 TS template literal 抽出獨立 `.md`，rewrite 期間 TS / Rust 共用）
- 保留既有 vault 格式與 CLI 行為契約（user 用 TS 0.1.0 init 過的 vault 用 Rust 版繼續操作不會壞）
- 5-7 週內到 CLI parity，期間 `main` 不需 ship TS 修補

**Non-Goals:**

- **不實作 codebus-app/ Tauri shell**：本 change 僅在 workspace 註冊 crate 位置，frontend 框架選型、station 模型 port、`<Checkpoint>` mdc 元件等留作下一個獨立 spec
- **不處理 Rust binary distribution**：cargo publish / GitHub Releases / homebrew tap / multi-platform CI 屬獨立 distribution spec
- **不 port Phase 2 LLM provider 多元化**：本 change 僅 port 現有 `ClaudeCliProvider`；Anthropic API direct / OpenAI / 本地 model 留作獨立 change
- **不導入新的 wiki schema 規則**：除已併入的 wiki-hygiene-signals 兩條 rule，不另外加 lint
- **不改變 vault 格式或 CLI 行為**：`.codebus/wiki/**` 結構保留、CLI subcommand args / stdout / exit code 與 TS 0.1.0 byte-equal
- **不 port npm 發行設定**：`package.json`、`tsconfig.json` 隨 Phase D 移除
- **不在本 change 裡選 frontend 框架（Svelte / React / SolidJS）**：Tauri spec 才討論
- **不導入 markdown AST 套件**：lint 沿用 `lint-markdown-aware-scan` 既有 trade-off（regex + 直接 byte length）

## Decisions

### Cargo workspace + 3 crate 結構

**選擇**：

```
codebus/
├─ Cargo.toml (workspace)
├─ codebus-core/   ← pure Rust lib：lint, frontmatter, stale-detect, page-merge, vault layout, stream parser, LLMProvider trait, fs, git, claude_cli
├─ codebus-cli/    ← clap binary，引用 core
└─ codebus-app/    ← Tauri 殼，本 change 僅預留 Cargo.toml，內容空
```

**為什麼**：core 邏輯一次寫好，CLI 與未來 Tauri app 共用；型別契約 compile-time 共享、不需要透過 process boundary 維護 schema。

**Alternatives considered**：

- **單一 binary 不分 crate**：簡單但 Tauri 整合時需把所有 module pub 出去當 lib 用，回頭重構工程量不小
- **CLI + Tauri 各自獨立 codebase（共享靠 git submodule / npm-style monorepo）**：自由度高但 dependency 版本同步、refactor 跨 repo 都痛
- **僅 `codebus-core` + `codebus-cli` 不預留 Tauri crate**：Phase E 開 Tauri 時再加；接受但會在 workspace 改動 `Cargo.toml` 時觸發一次 review，多此一舉

### 在 main 直接重寫，不開平行樹

**選擇**：rewrite 直接在 `main` 進行，TS 期間凍結；不開 `rewrite/rust` branch、不用 git worktree 平行。

**為什麼**：
- 0.1.0 未 release，**沒有 user 需要 ship TS 修補**
- `main` 無 active changes（剛 archive 完 lint 改動），平行樹只是徒增 merge 成本
- phase 1 active 開發已同意凍結
- 強制 commitment（沒 fallback 容易拖延）

**Alternatives considered**：

- **`rewrite/rust` branch + main 持續 ship**：典型「平行軌道」做法。被否決：當下沒 user、沒緊急 bug fix、平行樹反而拖慢 cutover
- **git worktree（同 repo 兩個工作目錄）**：類似 branch 但工作區隔離。被否決理由同上

### 既有 TS code 移入 legacy/ts-src/，Phase D 達 parity 後刪

**選擇**：

```
mv src/ legacy/ts-src/src/
mv tests/ legacy/ts-src/tests/
echo "reference impl, do not execute" > legacy/README.md
```

`legacy/ts-src/` 在 rewrite 期間：
- **不執行**：不 build、不 ship
- **保留作 reference impl**：iter-8 / iter-9 的 scar tissue（`--tools` sandbox comments、stream-parser schema 註記、enrichSourceMetadata invariant 檢查邏輯）隨時可 grep 翻閱
- 條件性 mining：例如重寫 `lint.rs` 卡 regex 細節時、回 `legacy/ts-src/src/core/wiki/lint.ts` 對照原本怎麼寫

Phase D 達 parity 後 `git rm -r legacy/`，commit message 標明「rust parity reached, removing ts reference impl」。

**為什麼**：iter-8 / iter-9 教訓不在型別簽章裡、只在 comments + behavioral tests + commit history 裡。直接砍光等於丟掉這些 invariant 知識。

**Alternatives considered**：

- **直接 `git rm`、靠 git history 翻**：技術可行，但 IDE 跨 git history grep 體驗差、慢；保留實體檔案 `grep -r enrichSourceMetadata legacy/` 一秒到位
- **保留到永遠**：違反「不留死碼」原則、誤導未來新 contributor

### Conformance 透過 fixture snapshot 確保行為一致

**選擇**：

```
Pre-rewrite (Day 0):
  npm run build (TS)
  ./codebus check --repo D:/side_project/uv > tests/fixtures/uv-vault-snapshot/check-output.txt
  echo $? > tests/fixtures/uv-vault-snapshot/check-exit-code.txt
  ./codebus init --repo D:/side_project/uv
  cp -r D:/side_project/uv/.codebus/wiki tests/fixtures/uv-vault-snapshot/wiki-after-init/
  // ... 同樣對 query、check-after-edit 等所有 deterministic command 各跑一次

During rewrite:
  Rust unit / integration test 用 include_str! / include_bytes! 載入 fixture，比 byte-equal
```

LLM-streaming 的 `goal` command 因為非 deterministic，無法 fixture：
- **mock LLMProvider**（餵固定 StreamEvent 序列）
- **fixture orchestration 副作用**：goals.jsonl entry、enrichSourceMetadata 後的 frontmatter、autoCommit message
- 真實 LLM 行為靠 manual smoke test 兜（uv vault 跑一個真 goal、肉眼看）

**為什麼**：iter-8 stream-parser schema bug、enrichSourceMetadata invariant break 都是「unit test 沒寫到的細微行為差異」型 bug。fixture 比對是「實際輸出 vs 凍結事實」，不靠想像力。

**Alternatives considered**：

- **純信 Rust unit test + 80%+ coverage + 手動 uv vault smoke**：不夠嚴格，subtle 行為差（換行符位置、git status output parsing）會被 user 在 cutover 後發現
- **保留 TS build chain，平行跑兩個 binary diff stdout**：與 `legacy/ts-src/ 不執行` 矛盾、且需維護兩條 build chain 4-7 週

### 吸收 wiki-hygiene-signals 兩條 lint rule

**選擇**：parked change `wiki-hygiene-signals` 的兩條 warn rule（page-size threshold per file type、unexpected-file detection）併進 Rust `lint.rs` 初版實作 + 對應 spec delta（本 change 的 `specs/wiki-lint/spec.md`）。實作後刪除 parked change 目錄（`spectra archive` 不適用 — 因為它從未進 active 狀態）。

threshold 沿用 wiki-hygiene-signals 的設定：
- `wiki/index.md` > 1 KiB
- `wiki/synthesis/<slug>.md` > 5 KiB
- `wiki/{concepts,entities,modules,processes}/<slug>.md` > 8 KiB
- `wiki/log.md` 不限（chronological-by-design）

**為什麼**：Rust lint 反正要從零寫一次、多接兩條 rule 工程量幾乎免費；parked change 已通過 spike 驗證真實需求；否則 Rust 0.2 一推出馬上比 TS 0.1 少兩條 warning，是 regression。

**Alternatives considered**：

- **Rust 先 port TS 0.1 的 4 條 rule、wiki-hygiene-signals 留 parked**：split work 但 Rust release 後馬上要再開另一個 change 加 rule、不必要的迭代
- **直接放棄 wiki-hygiene-signals**：spike 驗證的真實需求消失

### Schema 拆出獨立 .md，Rust 與 TS 共用

**選擇**：

```
Step 1 (pre-rewrite):
  從 src/schema/claude-md.ts 把 CODEBUS_SCHEMA_MARKDOWN template literal 抽出
  存成 codebus-core/src/schema/CLAUDE.md（檔案位置在 Rust 端，但路徑允許 TS 期間先讀）
  src/schema/claude-md.ts 改成 readFileSync('codebus-core/src/schema/CLAUDE.md')

Step 2 (rewrite):
  Rust 端 const CODEBUS_SCHEMA: &str = include_str!("./CLAUDE.md");
```

**為什麼**：rewrite 期間 TS 與 Rust 雙邊讀同一檔，避免 schema 兩邊各維護一份的漂移風險。Schema 是 string，最舒服的容器是 `.md` 檔（IDE markdown lint、預覽、表格全部能用）。

**Alternatives considered**：

- **rewrite 期間 schema 兩邊各維護**：高機率出現「TS 改了 schema 但忘了同步 Rust」的 drift bug、cutover 後才發現
- **schema 留在 codebus-core 只在 cutover 那刻搬**：cutover 那一刻 TS 還活著（reference impl），需要讀 schema 跑 fixture 比對，搬太晚

### Phase 順序：core pure → core I/O → CLI → cleanup

**選擇**：

| Phase | 內容 | 工期 |
|---|---|---|
| Pre-rewrite | fixture snapshot + 移 legacy + Cargo workspace 起手 + schema 拆檔 | 1 day |
| **Phase A** | core 純模組（schema, types, frontmatter, date, page-merge, stale-detect, lint, vault layout/sanity-check/lock, stream parser, llm provider trait） | 2-3 weeks |
| **Phase B** | core I/O 模組（fs/raw_sync, fs/file_ops, git/source_version, git/nested_repo, llm/claude_cli subprocess） | 1 week |
| **Phase C** | codebus-cli 的 4 個 clap subcommand、與 TS 0.1.0 CLI parity | 1 week |
| **Phase D** | cleanup（rm legacy/, rm package.json/tsconfig.json/node_modules） | 1-2 days |

**為什麼**：
- 純模組（input string → output struct）conformance 比對最乾淨，先穩這一刀
- I/O 模組有 dependency selection unknown（git2 vs shell-out、ignore crate 細節），晚一刀讓前面已穩的純模組墊底
- CLI 是組合層，core 穩了 CLI 才有機會 byte-equal
- cleanup 最後做、保留 legacy 直到 parity 蓋章

**Alternatives considered**：

- **平行三 phase 同時開**：solo dev 不可能、context switch 成本高
- **CLI 先 (clap subcommand 殼骨架先到)、再回頭填 core**：clap 殼空骨架沒測試價值、且 core API 還沒設計完前 CLI 殼簽名會 churn

### Tauri app 留作獨立 spec（codebus-app/ 本 change 只預留位置）

**選擇**：本 change 在 workspace `Cargo.toml` 註冊 `codebus-app/` member，但 `codebus-app/Cargo.toml` 內容只是空殼（`name`、`version`、`edition` 必要欄位 + 一個 placeholder `src/main.rs` 印 "tauri app placeholder"）。實際 Tauri 整合（frontend 框架選型、`tauri::command` IPC、station 模型 port、`<Checkpoint>` mdc 元件）留作 Phase E、獨立 spec。

**為什麼**：
- frontend 框架選型（Svelte / React / SolidJS）+ station 模型設計 + tauri-plugin 選用，每個都是獨立決策
- 同時開 Tauri 會推著 core API churn；core 應該先穩、Tauri 才有穩定底座
- 本 change scope 已大（5-7 週、~30 task），再加 Tauri 變失控

**Alternatives considered**：

- **本 change 含完整 Tauri MVP**：scope 失控、結束時間從 7 週推到 12+ 週
- **完全不預留 codebus-app/ crate**：Phase E 開 Tauri 時再加；可接受但會多一次 workspace `Cargo.toml` 改動 + review

### Async runtime 選 tokio

**選擇**：codebus-core 採 tokio 作 async runtime；`LLMProvider::invoke` return `impl Stream<Item = StreamEvent>`；subprocess 用 `tokio::process::Command`。

**為什麼**：
- Tauri 2.0 內建 tokio，core 與 Tauri 共用同 runtime 不需 bridge
- 生態成熟（reqwest、tokio-stream、async-stream macro）
- 未來 Phase 2 LLM provider 多元化（直打 Anthropic API SSE）需要 reqwest + eventsource-stream，這些都是 tokio-native

**Alternatives considered**：

- **async-std**：生態漸縮、Tauri 不用、未來要 bridge
- **smol**：輕量但 Tauri 整合需要 compat layer
- **同步 std::process + thread**：簡單但 LLM streaming + cancel + Tauri emit() 整合很彆扭

### LLMProvider trait 保持單一 ClaudeCli 實作（Phase 2 抽象延後）

**選擇**：本 change 的 `LLMProvider` trait 形狀與 TS 0.1.0 對齊，但只實作 `ClaudeCliProvider`（spawn `claude -p` subprocess、parse stream-json）。Anthropic API direct / OpenAI / 本地 model 留作獨立 change。

**為什麼**：trait 的設計重點是「未來能多 impl」、不是「本 change 就要多 impl」。一次只 port 一個 provider，conformance 才容易比對。

**Alternatives considered**：

- **本 change 同步加 Anthropic API direct provider**：scope shift、conformance 變雙倍工
- **本 change 不抽 trait、直接 hardcode `ClaudeCliProvider`**：未來加 provider 時要重構 trait + 每個 caller、違反 phase 1 的 hexagonal 切法

## Risks / Trade-offs

- **Risk: 重新 encode iter-8 / iter-9 hard-won invariant 時 re-introduce bug**
  → **Mitigation**：(a) `legacy/ts-src/` 整個 commit 期間都在、隨時 grep 翻閱、(b) conformance fixture 對 deterministic 路徑 byte-equal、(c) sandbox argv 在 Rust 端寫**雙重斷言** test：既測 `--tools` 帶到、也測「未授權 tool 不在 argv 裡」

- **Risk: Rust 編譯時間懲罰 LLM iteration loop**
  → **Mitigation**：(a) `cargo watch -x test` + incremental compile（首次 cold ~30s、之後 ~5-10s）、(b) schema 在 .md 檔，改 schema 不觸發編譯、(c) `goal` 命令的 prompt 迭代靠 `claude` CLI 直接跑、不需要 rebuild

- **Risk: dependency 選型卡住（git2 vs shell-out、ignore crate 細節、serde_yaml frontmatter quirks）**
  → **Mitigation**：(a) Phase A 全部避開 I/O、純模組先穩、(b) Phase B 開始前對每個 unknown 各開 spike（最多 1 day），確認接得起來再正式寫

- **Risk: 5-7 週 main 沒 shippable binary、parked change wiki-hygiene-signals 凍結**
  → **Mitigation**：(a) 0.1.0 未 release、無 user 受影響、(b) wiki-hygiene-signals 在本 change Phase A 一併實作、不真的丟掉

- **Risk: cutover 後發現 conformance fixture 沒覆蓋的行為差異**
  → **Mitigation**：(a) Phase D 完成後**保留 legacy/ts-src/ 一個 cool-down 週期**（例如 1 週），期間任何 issue 可快速翻舊實作、(b) cool-down 完成才真正 `git rm legacy/`

- **Trade-off: 接受 4-7 週「無新 feature 產出」的鎖死期**
  → 這是明確的成本，但與 Tauri commitment 一旦確定就要付的對齊工是同一筆錢，提前付比晚付便宜

- **Trade-off: 接受 Rust 學習曲線中段（async lifetime、`Pin<Box<dyn Future>>`、orphan rule）的開發速度下降**
  → 個人成長預算的一部分；buddy-gacha 的 cargo tauri dev 經驗可降低 ramp-up

## Migration Plan

### Pre-rewrite checklist (Day 0)

1. 執行 fixture snapshot：
   - `npm run build`（最後一次 TS build）
   - 對 `D:/side_project/uv` 各跑一次 `init` / `check` / `query`，stdout / exit code / vault state 全部存進 `tests/fixtures/uv-vault-snapshot/`
2. Schema 拆檔：
   - 從 `src/schema/claude-md.ts` 抽出 `CODEBUS_SCHEMA_MARKDOWN` 內容到 `codebus-core/src/schema/CLAUDE.md`
   - `src/schema/claude-md.ts` 改成 `readFileSync('codebus-core/src/schema/CLAUDE.md')`
   - 跑既有 152 test 確認無 regression
3. 移 legacy：
   - `git mv src/ legacy/ts-src/src/`
   - `git mv tests/ legacy/ts-src/tests/`
   - 寫 `legacy/README.md` 註明用途
4. Cargo workspace 起手：
   - 建 root `Cargo.toml` 註冊 3 crate member
   - 建 `codebus-core/`、`codebus-cli/`、`codebus-app/` 三個資料夾骨架（Cargo.toml + src/lib.rs 或 src/main.rs 空殼）
   - `cargo check` 確認 workspace 解析成功
5. 1 個 commit 完成 pre-rewrite，message 標明「pre-rewrite: snapshot fixture, move legacy, init cargo workspace」

### Rollback 策略

- **Pre-rewrite 階段**：失敗就 `git reset --hard` 回到 main 上次 commit
- **Phase A / B 中途放棄**：legacy/ts-src/ 還在，`git mv legacy/ts-src/src ./src && git mv legacy/ts-src/tests ./tests` 即恢復 TS 可運作；package.json / tsconfig.json 也都還在
- **Phase C 中途放棄**：同上
- **Phase D 之後放棄**：`git rm legacy/` 已執行，靠 `git revert` 或 `git checkout <pre-cleanup-sha> -- legacy/` 回復；理論上 cool-down 期間就該抓出問題、不該到這一步

### Cool-down period

Phase D 完成後**保留 legacy/ts-src/ 一週**（不 commit `git rm legacy/`），期間：
- 對 buddy-gacha / 公開 repo 跑 manual smoke test
- 任何 user-facing behavior diff 立刻翻 legacy/ 對照
- 一週無 issue 才正式 commit `chore: remove ts reference impl after cool-down period`

## Open Questions

- **fixture 中要不要包含 LLM-mock 後的 `goal` 結果？** 傾向「只 fixture orchestration 副作用、不 fixture LLM streaming text」。Phase A 進入時再 finalize。
- **`tests/fixtures/uv-vault-snapshot/` 體積**：可能 100KB-2MB（取決於 uv vault 大小）。git 直接存可接受、git-lfs 不需要。Day 0 實際存完看大小再決定。
- **codebus-app/ 內 placeholder 用 println! 印什麼**：留待 Phase E 第一次實作時取代，本 change 不糾結。
- **要不要在 pre-rewrite 階段先 publish TS 0.1.0 到 npm 作為「歷史記錄」**：傾向不發、因為 0.2.0 是 Rust 形態、distribution 通道可能換、發 0.1.0 反而誤導。需 confirm。
