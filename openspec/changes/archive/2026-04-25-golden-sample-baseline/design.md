## Context

`docs/implementation-plan.md` 步驟 23 寫的是「Golden sample 首跑（Timeline ideal-route 對比）」，但「首跑」這個詞拆開來其實是兩件事：

1. **基礎設施第一次成形**：scoring 從 markdown 公式變 Python code、ideal-route 從文件變 JSON schema、replay 從只摸 P0 升級到摸全 stack。
2. **真 LLM 第一次跑**：用上面的基礎設施配真 OpenAI 跑一次 Timeline-style fixture，snapshot baseline。

D-006 已經明示第二件事「打磨期再做」（怕 live LLM 在 MVP 期就把每次測試都灌錢、且 CI 不該打 OpenAI）。所以本 change 的合理範圍是**第一件事**——把基礎設施準備好，讓打磨期接 live snapshot 時只是換一行 `MockProvider` → `OpenAIChatProvider`。

當前 golden 基礎設施盤點（要拆解才知道要加什麼）：

```
tests/golden/
├── demo-synthetic/                         ← P0 鎖點，不動
│   ├── README.md                           （Sanitizer 用，與 Explorer 無關）
│   ├── expected.json                       （5 欄 stations / stopped_reason / step_count / 兩 prompt_version）
│   └── workspace/src/{a,b,c}.py            （3 dummy file 讓 path resolve 不炸）
├── timeline-gdrive-adapter/                ← 設計文件
│   └── ideal-route.md                      （5 站手寫 ideal route + recall/noise 公式 markdown）
└── (本 change 新加)
    └── timeline-storage-adapter-synthetic/ ← 對應 ideal-route.md 的可執行 fixture
        ├── README.md
        ├── ideal-route.json                （Pydantic schema，機器讀）
        └── workspace/app/...               （9 檔模擬 Storage Adapter 拓撲）

sidecar/tests/golden/
├── test_explorer_replay.py                 ← P0 7 測，不動
└── (本 change 新加)
    ├── scoring.py                          （recall / noise / composite_score helpers）
    ├── test_scoring.py                     （scoring helpers 單元測試）
    └── test_timeline_synthetic_replay.py   （新 replay 對 timeline fixture）
```

約束：

- **不污染 production code**：`codebus_agent/` 下零改動；scoring 是 test-time 的事，沒有任何 runtime 路徑會去算 recall。
- **與既有 P0 並行**：`demo-synthetic` baseline 是 `explorer-judge-golden` archive 的 5 欄鎖點，本 change 不擴它（避免 re-baseline 連鎖）。新 fixture 用獨立 `ideal-route.json` schema、新 replay 用獨立測試檔。
- **D-022 wire payload 純度**：scoring 結果不寫 `reasoning_log.jsonl` / `token_usage.jsonl` / `llm_calls.jsonl` 任何 audit 檔；分數是測試時 assertion 的副產品。
- **D-006 公式 1:1 對應**：`composite_score = 0.5 * recall + 0.3 * (1 - noise) + 0.2 * depth`，weights 預設值寫死、可注入 override（為打磨期 tuning 預留）。
- **TypeScript fixture 不需編譯**：Explorer scanner 只看檔案內容做 grep / search，不會 `tsc` 編譯；fixture `.ts` 檔可以是不完整 stub，內容只要支援 grep 命中（例如 `interface IStorageService { ... }` 字串能被找到）。

## Goals / Non-Goals

**Goals:**

- 把 D-006 的 markdown 公式落成 Python 程式（`station_recall` / `station_noise` / `composite_score`），可被任何 future fixture 重用。
- 提供第一份「對齊 ideal-route.md」的可跑 fixture（`timeline-storage-adapter-synthetic/`），9 檔涵蓋 must_have（5）/ nice_to_have（2）/ noise（2）。
- 第一次把整套 Module 4 stack（Explorer + Tools + Judge + Coverage + Token budget + SSE emit）綁進同一個 scripted golden replay；證明 production wire 形狀在 test-time 可用同一條路重現。
- 為打磨期的 live LLM snapshot 預留接口：`scoring.py` 與 `IdealRoute` schema 與 fixture 結構都是 LLM-agnostic，未來 `MockProvider` → `OpenAIChatProvider` 一行替換即可接真 LLM。

**Non-Goals:**

見 proposal Non-Goals 段（live LLM / 真 Timeline mirror / 完整 depth 評估 / scoring 寫 expected.json / 多語言 fixture / 改 demo-synthetic baseline / scoring 進 production / 改 markdown 不用 JSON / 跑 ideal-route.md 真 repo）。

- 本 change **不**動 `codebus_agent.agent.types.Station.depends_on`（仍 hardcode `[]`）；depth 評估暫回 `1.0` placeholder，等 Module 5 把教材 MOC 圖反向填回 station depends_on 才能算真 depth。
- 本 change **不**做 GitHub Action / CI hook 把 Timeline fixture replay 加進 `pre-commit` 或 `pre-push`——既有 `uv run pytest sidecar/tests/` 已涵蓋；當 fixture 要進 CI gate 是「打磨期」決定。
- 本 change **不**讓 Module 5 / Module 8 共用 scoring helpers——`scoring.py` 留在 `sidecar/tests/golden/`；當 Q&A 或 Generator 也想用同公式評估時，再考慮升 `sidecar/src/codebus_agent/scoring/`（前提是真有非 test-time 路徑要呼）。
- 本 change **不**支援 fixture workspace 是真 git repo（無 `.git/` 子目錄）——Scanner / Explorer 對待 fixture 與普通資料夾相同，不依賴 git 元數據。

## Decisions

### Decision 1：scoring helpers 放 `sidecar/tests/golden/scoring.py`，不放 `sidecar/src/codebus_agent/scoring/`

選 test-only 模組路徑，不進 production package。

理由：

- **責任分層乾淨**：`codebus_agent.scoring` 暗示「runtime 會跑分數計算」——這對 Explorer / Generator / Q&A 都不成立。Scoring 只在離線分析（CI 跑 golden 測 / Demo 前驗證）時用。
- **Import 路徑反映 scope**：`from sidecar.tests.golden.scoring import station_recall` 一眼看出是 test-only 工具；`from codebus_agent.scoring import ...` 會誤導讀者覺得是 production module。
- **未來升 production 時門檻低**：當真有 production 路徑要算 recall（例如 Q&A self-critic），把檔搬到 `codebus_agent/scoring/`、改 import path 即可；現在不預設未來必要。

**替代方案**：放 `codebus_agent.scoring`。棄用——過度抽象、誤導 architecture，且現在沒任何 production 路徑會用。

### Decision 2：`IdealRoute` schema 用 Pydantic JSON 而非 markdown

選 Pydantic v2 BaseModel + `ideal-route.json`，不重用 `ideal-route.md`。

理由：

- **機器讀的真相 vs 人類讀的設計文件**：`ideal-route.md` 含敘述 / 評分 rubric / 待 review 點，是設計時討論用；`ideal-route.json` 是測試載入的 fact，schema 死的。
- **Pydantic 強制 schema**：欄位漏填、型別錯誤都在載入瞬間 raise；markdown 要自己寫 parser 容易遺漏邊界。
- **編輯器友好**：JSON schema 跟 IDE / lint 整合天然；markdown 要靠人眼。
- **與既有 `expected.json` 同調**：現有 demo-synthetic 已經用 JSON 鎖 baseline，新 fixture 用 JSON 一致。

**替代方案 A**：YAML（schema 差不多但可寫註解）。棄用——Pydantic 不直接讀 YAML，要再裝 PyYAML 額外依賴；JSON 已夠用。

**替代方案 B**：複用 `ideal-route.md` parse 出 must_have 等清單。棄用——markdown 結構鬆散、parser 易脆。

### Decision 3：Fixture 用合成 mini-files 而非真 Timeline mirror

9 個自己寫的 `.ts` stub，而不是把真 `~/projects/timeline` 拉進來。

理由：

- **可重現 + 可進 git**：合成 fixture 體積小（~300 行 total）、binary-free、不必處理 git submodule。
- **意圖明確**：每個檔的存在是為了 fixture 設計，不會有「為什麼有這檔？」的迷霧；真 repo 拉進來會帶 PWA / UI assets / build artifacts 等與 fixture 用途無關的雜訊。
- **法律 / 隱私保險**：使用者真實專案的程式碼進 codebus repo 有授權問題；自寫 fixture 完全乾淨。
- **與 `tests/golden/demo-synthetic/workspace/` 同模式**：demo-synthetic 的 `workspace/src/{a,b,c}.py` 也是 dummy stub；新 fixture 模式對齊。

**替代方案**：git submodule 把 timeline repo 掛進來。棄用——proposal Non-Goals 已列；體積與授權問題都未解。

### Decision 4：Coverage round 的 scripted MockProvider 也一併 wire

新 replay 用真 `LLMCoverageChecker`（餵 scripted CoverageProvider），取代既有 `_NoopCoverage` 殘影。

理由：

- **與 production handler 形狀對齊**：`api/explore.py` 的 handler 用 `LLMCoverageChecker(coverage_factory, workspace_root)`，replay 用 `_NoopCoverage` 是 P0 簡化的權宜，現在該補。
- **驗 `coverage_gaps` SSE event**：本 change 替全 stack pinned scenario 加 spy emitter，coverage round 必須真有跑才能觀察 event；`_NoopCoverage` 不會 emit `coverage_gaps`。
- **複用既有 fixture 形狀**：`mock_coverage_provider_factory` / `mock_script_coverage` 已在 `sidecar/tests/agent/conftest.py`（`coverage-gap-recurse` 加），但 conftest scope 是 per-directory，`sidecar/tests/golden/` 摸不到——本 change 要在 golden 內部 inline 重新定義 factory（與既有 `test_explorer_replay.py::_make_factory` 同模式）。

**替代方案**：留 `_NoopCoverage`、新 replay 也跳過 coverage。棄用——半套 stack 反而誤導；既然要證明全 stack 通電，就要全。

### Decision 5：Token probe 帶 `AggregatedTokenProbe([reasoning, judge.provider, coverage.provider])`

不另外做小型 mock probe；直接用 production aggregator 形狀。

理由：

- **production-shape 一致**：`api/explore.py` handler 也是這樣組 probe；replay 用同形狀對齊。
- **驗 `LLMJudge.provider` / `LLMCoverageChecker.provider` property**：這兩個 property 在 `context-compression-token-budget` archive 加進來、本 change 第一個真 caller 場景。
- **驗 `usage_delta.session_total_tokens` 累計正確**：scripted 5 step 跑完後，三 provider 的 `session_total_tokens` 加總應該 > 0，spy emitter 抓的 event 欄位應該對得上。

### Decision 6：Recall/noise 門檻 `>= 0.9` / `<= 0.1`，不寫進 `expected.json`

測試裡硬 code 門檻、不放 fixture。

理由：

- **分數是浮動量**：depth 暫 placeholder 1.0，未來改真 depth 會讓 composite 動；weights 也可能在打磨期 tune。把分數寫 fixture 等於每次 tuning 都要 re-baseline。
- **fixture 鎖的是「應命中哪些檔」**，不是「分數該多少」——這個分層邏輯與 `expected.json` 鎖 stations set 而非 relevance/why 一致。
- **門檻語意**：本 change scripted scenario 是設計成 100% 命中 must_have、0% 命中 noise，所以 recall=1.0 / noise=0.0 / composite=1.0 是「天花板」；門檻設 `>= 0.9` 給 0.05 浮動空間（depth placeholder 改動 / weights 微調都不會破）。
- **未來 live LLM 場景**會有真實漂移，那時門檻可能要降到 `>= 0.7`；但這是 LLM-snapshot change 的事，不是本 change 鎖點。

### Decision 7：Fixture 檔案內容極簡，只支援 grep 命中（不可編譯）

`.ts` 檔可以不是 valid TypeScript（缺 `import` / 缺 type）；只要 string 能被 search/read 命中即可。

理由：

- **Scanner / Explorer 不編譯 TS**：tools 只做 fs 級別讀寫 + grep，不過 tsc。fixture 不必跑 build。
- **體積最小化**：每檔 ≤ 40 行；interface 列 method 名稱、impl 用 `// stub` 註解占位。
- **意圖聚焦**：實作細節對 ideal-route 評估不重要，重要的是「Explorer 有沒有讀到這檔」。

### Decision 8：Drift guard 形狀——fixture 改檔即測試紅

`test_timeline_synthetic_replay.py` 載入 `ideal-route.json` 作為 must_have / nice_to_have / noise 來源，scripted actions 列舉 5 個 must_have 路徑；任何一邊改字串就斷。

理由：

- **三層 drift trip wire**：
  1. fixture 檔被刪 → scripted actions 路徑不命中 → recall 跌 → 紅
  2. `ideal-route.json` must_have 改 → scoring 算錯 → 紅
  3. scripted actions 改路徑（不再對齊 must_have）→ recall 跌 → 紅
- **強迫同步**：要動 fixture 就要 ideal-route.json + scripted actions 一起動，避免半改誤通過。

## Risks / Trade-offs

- **[scripted MockProvider 沒驗 LLM 真決策]** → 確實沒驗；但本 change 範圍就是基礎設施，proposal 已明列「真 LLM 留打磨期」。本 change 的價值是讓打磨期接 live snapshot 時 90% 工作已完成。**Mitigation**：proposal Non-Goals 已寫清楚邊界，避免讀者誤解。
- **[fixture 9 檔可能不夠覆蓋真實 Timeline 拓撲]** → 9 檔是「最小可識別 Storage Adapter 模式」的妥協；真 Timeline 還有 Pinia 中介、PWA service worker 等。**Mitigation**：fixture README 說明這是 mini-synthetic 不是 mirror；打磨期可以擴 fixture 或換 live repo。
- **[depth 暫 placeholder 1.0 讓 composite_score 永遠偏高]** → 是；但 D-006 對「以 score >= 0.7 為合格」是 informal 標準，本 change 門檻也是 informal。**Mitigation**：proposal 與 design 都明標「depth placeholder」，未來開新 change 實作真 depth 時記得回頭調門檻。
- **[scoring helpers 函式邊界條件可能誤判]** → empty must_have raise / empty extras 回 0.0 是設計選擇，可能與 D-006 markdown 公式直譯有出入。**Mitigation**：scoring 單元測試明列邊界 case 並在程式 docstring 說明 rationale；改公式要回頭改測試。
- **[fixture 命名 `timeline-storage-adapter-synthetic` 太長]** → 是，但比 `timeline-mini` 更明確標記「合成」防誤認真 Timeline 拉進來。短名 `timeline-syn` 又太縮。長名一次寫對勝過反覆改。
- **[`tests/golden/timeline-gdrive-adapter/ideal-route.md` 與本 change 新 fixture 內容不完全一致]** → md 列 5 站的具體檔名與 fixture 寫的 9 檔可能小有差異（fixture 用合成路徑簡化）。**Mitigation**：fixture README 明寫「對應 ideal-route.md 的拓撲、檔名做合成」；md 不刪不改，作為設計時的人類 reference 留在原處。
- **[`sidecar/tests/golden/scoring.py` 沒 import re-export]** → 沒做 `__init__.py` re-export，未來其他測試要用會寫 `from sidecar.tests.golden.scoring import ...` 比較長路徑。**Mitigation**：scoring 是 fixture 私有工具，不期待被其他測試直接 import；要分享時再升 module。

## Migration Plan

- 無 schema 破壞——`Station` / `ExplorerResult` / `expected.json` 5 欄完全不動。
- 無 production code 改動——`codebus_agent/` 零變更。
- 無 HTTP API 破壞——`POST /explore` 不動。
- 既有 `sidecar/tests/golden/test_explorer_replay.py` 7 測 path-resolve 不變（fixture root 還是 `_golden_root() = parents[3] / "tests" / "golden" / "demo-synthetic"`）；新 replay 用對應的 `_timeline_synthetic_root()` helper。
- `tests/golden/` 同層多 fixture 共存，無命名衝突（`demo-synthetic` / `timeline-gdrive-adapter` / `timeline-storage-adapter-synthetic` 三目錄並列）。
- 本 change 落地後 D-006 後續 checklist 兩 `[x]`（fixture 建立 + scoring 落地），保留 live LLM `[ ]` 給未來 change。

## Open Questions

無。（proposal + design 已涵蓋所有 fixture 結構 / scoring 形狀 / replay 範圍 / 邊界條件決策。）
