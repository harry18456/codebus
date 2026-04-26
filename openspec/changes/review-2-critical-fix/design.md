## Context

Review #2（`docs/reviews/2026-04-26-stage-5.md`，2026-04-26）由 6 個 read-only agent 平行掃出 84 條 issue。本 change 鎖定其中 7 條 Critical（CR-1 ~ CR-7），都是 Phase 6 前端動工前必須清掉的 production / demo-breaking 問題。

**動工時的 baseline**：
- M2 backend 已完整 archive：Modules 1, 2, 4, 5, 8 P0 全部通電，5 個 endpoint 全綠（POST /scan / kb/build / explore / generate / qa）。
- 七層 audit JSONL chain 全通電，最後一層 `kb_growth.jsonl`（`module-8-qa-p0`，2026-04-26 archive，~24 小時前）。
- 18 個 capability spec，823 passed / 19 skipped tests。
- `review-backlog-cleanup`（2026-04-25 archive）已落地 `RULES_VERSION` single-constant 收緊 + chat `cost_usd` pricing-table 真值。

**為什麼現在做**：Phase 6 前端會 pin 在後端 API contract 上（特別 `docs/sidecar-api.md`）。CR-4 / CR-5 / CR-6 / CR-7 都是 docs 跟 spec/code 字面衝突 —— 前端按 docs 寫會在 demo 當天炸。CR-1（KB_BUILD_FAILED）+ CR-3（kb_growth.entry_id）是 wire payload 真錯，CR-2（rules_version "rules-unknown"）是稽核鏈污染。

## Goals / Non-Goals

**Goals**：
- 7 條 Critical 一次性收掉，不留分批殘餘。
- 三個 MODIFIED capability spec（sidecar-runtime / knowledge-base / kb-growth）跟 production code 字面對齊，不變式 9 + Trust Layer R-01 join 鏈完整。
- 既有 823 passed 測試不破，新增 ~3-5 條 defensive test 鎖死 fix。
- docs/sidecar-api.md + docs/qa-agent.md 跟 spec / code 1:1 對齊，前端可直接以最新 docs 為準。

**Non-Goals**（詳 proposal Non-Goals 段）：
- 不收 22 條 Cat 1 doc-stale（另 doc-sync commit）。
- 不收 28 條 Cat 2 spec-wrong（另 `spec-cleanup-stage-5-batch-1`）。
- 不收 4 條 Cat 2.5 cross-cutting drift（另 `audit-path-unification-stage-2`）。
- 不重做 Q&A integration test（屬於 Cat 2，另走）。

## Decisions

### Decision 1：`KB_EMBED_FAILED` 直接 rename 為 `KB_BUILD_FAILED`，不留 alias

**選擇**：`api/tasks.py` ERROR_CODES frozenset 把 `"KB_EMBED_FAILED"` 直接刪掉、改為 `"KB_BUILD_FAILED"`；`_classify_exception` / `_safe_error_message` 對應分支同名 rename。test 內所有字面量同步 rename。

**對比方案 A（reject）**：保留 `KB_EMBED_FAILED` 作 backward-compat alias，frozenset 同時包含兩個。

**理由**：
- 沒 production deployment 需要兼容（codebus 是桌面 app、無公開部署、無第三方 SDK consumer）。
- alias 會延長 drift：之後新 code 路徑可能繼續 emit `KB_EMBED_FAILED`、舊 alias 永遠拿不掉。
- spec 字面是 `KB_BUILD_FAILED`（`sidecar-runtime/spec.md:441`），代表 source of truth；code 跟齊就是。

**取捨**：本 change 沒 backward-compat 包袱，但建立了「rename code 時不留 alias」的範例；未來若有第三方依賴需要前端 SDK 兼容，需另開 ADR 補 alias 政策。

### Decision 2：`upsert_chunk` 簽名改 `tuple[str, str]`，第一欄是 outcome enum-like 字串、第二欄是真實 point_id

**選擇**：簽名從
```python
async def upsert_chunk(text: str, *, payload: KBPayload) -> str
```
改為
```python
async def upsert_chunk(text: str, *, payload: KBPayload) -> tuple[str, str]
```
其中第一欄 `outcome ∈ {"new", "dedup_hash", "dedup_sim"}`；第二欄是真實 point_id（hash dedup → 從 backend 查 hash 對應的既有 point_id；similarity dedup → 從 `find_similar` 回的 KBHit 取 `point_id`；new → 新建的 UUID）。

**對比方案 A（reject）**：簽名改回 `str` 但回傳「outcome:point_id」結合字串（如 `"dedup_hash:abc-123-uuid"`），caller `split(":", 1)` 解構。

**對比方案 B（reject）**：簽名改回 dict / Pydantic（`UpsertResult(outcome, point_id)`）。

**理由**：
- tuple 比結合字串清楚（type checker 看得見兩欄、不需 caller 自己 split）。
- tuple 比 dict / Pydantic 輕量（`upsert_chunk` 是熱路徑、避免每次 chunk 新建 BaseModel instance）。
- outcome 用字串而非 Literal type 是因為 Python `Literal["new", "dedup_hash", "dedup_sim"]` 在 caller 解構時 mypy 推得出來，但 runtime 等同 str；不需多一層 enum class。
- 這個改動是 API 簽名 breaking change，但 `upsert_chunk` 唯一 caller 是 `agent/tools/add_to_kb.py`，影響範圍可控。

**取捨**：spec scenario 寫 tuple 比寫 dedup token 字串麻煩一些，但對齊 type checker 友善。

### Decision 3：Hash dedup path 加 `_lookup_existing_point_id_by_hash` helper

**選擇**：`KBQdrantBackend` Protocol 已有 `exists_by_hash(collection, text_hash) -> bool`；新增 helper（不必加 Protocol method、走既有 `search_points`）：在 `KnowledgeBase` 內以 `await self._backend.search_points(..., query_filter={"text_hash": payload.text_hash}, limit=1)` 查到既有 point id。`exists_by_hash` 後若 True，緊接 `_lookup_existing_point_id_by_hash` 拿真實 id。

**對比方案 A（reject）**：擴 `KBQdrantBackend` Protocol 加 `find_point_id_by_hash(collection, text_hash) -> str | None` 一個 method，`exists_by_hash` 可改為 wrapper。

**對比方案 B（reject）**：把 `exists_by_hash` 改回 `find_point_id_by_hash`，bool 改為「None / point_id」，所有 caller 隨之改。

**理由**：
- 方案 A 是長線正解但要動 Protocol 跟 InMemoryQdrantBackend test fixture，超出本 change Critical-fix 範圍。
- 方案 B 改動更大且會 break 既有 `KnowledgeBase.build` 的 hash dedup 邏輯（line 191-199），太冒險。
- 走既有 `search_points` 有 `query_filter` 路徑（line 132-145）成本最低、不擴 Protocol、`InMemoryQdrantBackend.search_points` 已支援 dict filter（conftest.py:139-153 `_matches` helper）。
- 雖然查 hash 用 vector search 走 cosine 不如直接索引精確，但 hash 本身就是 unique（SHA-256 64 char），`text_hash` 在 KBPayload 是 indexed field（per `module-2-kb-builder-p0` 設計），filter exact match 一定命中或不命中、不會多筆。

**取捨**：未來如果 Qdrant `text_hash` index 改非 unique 索引（例如多 chunk 共 hash），這條 fast path 會失效；屆時擴 Protocol 加 `find_point_id_by_hash` 是乾淨方案。本 change 標 TODO comment 留 follow-up。

### Decision 4：`add_to_kb` rules_version 走 module-level import 直接使用，不留 fallback

**選擇**：`add_to_kb.py:124-131` 整段拆掉，改檔頂 module-level
```python
from codebus_agent.sanitizer import RULES_VERSION
```
function body 內直接 `rules_version = RULES_VERSION`。

**對比方案 A（reject）**：保留 `getattr(sanitizer, "rules_version", None)` 第一層 fallback，移除剩下兩層。

**對比方案 B（reject）**：留 `try / except ImportError: pass` 但縮窄 except（具名 `ImportError`）。

**理由**：
- `SanitizerEngine` 本來就沒 `rules_version` 屬性（M2 baseline）—— 第一層 fallback 永遠走 None。
- import RULES_VERSION 失敗代表 codebase 完全壞掉、應該 fail-loud（沒道理 swallow + 寫 `"rules-unknown"` 進稽核鏈）。
- 跟 `folder_tools.py:41`（`from codebus_agent.sanitizer import RULES_VERSION as _SANITIZE_RULES_VERSION`） / `tracked.py`（同樣 module-level import） 模式一致 —— 本 change 收齊三個 callsite。

**取捨**：未來如果 sanitizer engine 加上 `rules_version` instance attribute（讓不同 sanitizer instance 用不同 rules version），需另開 change 重新引入 fallback；屆時要明寫 ADR 並用具名 except。

### Decision 5：Defensive test 鎖在 `test_rules_version_constant.py`，不另開新檔

**選擇**：補 1 條 test 進既有 `sidecar/tests/sanitizer/test_rules_version_constant.py`：
```python
def test_add_to_kb_uses_rules_version_constant_directly():
    from codebus_agent.agent.tools import add_to_kb
    from codebus_agent.sanitizer import RULES_VERSION
    # add_to_kb module 必直接引用，不可有 fallback chain
    assert add_to_kb.RULES_VERSION is RULES_VERSION
```

**對比方案 A（reject）**：另開 `sidecar/tests/agent/tools/test_add_to_kb_rules_version.py`。

**理由**：
- `test_rules_version_constant.py` 已是 single-constant 收緊的官方 home（`review-backlog-cleanup` 落地）；新加一條 callsite 鎖死最一致。
- 別再分散：未來新 sanitizer rules_version callsite（若有）都應該加進這個檔。

**取捨**：無，純命名一致性。

### Decision 6：docs/sidecar-api.md 一次完整 sync，不局部 patch

**選擇**：CR-4 + CR-5 + CR-6 + CR-7 四條 docs 改動在同一個 task block 完成，而不是分四個 commit。完整重寫 §三 / §三-bis / §四 三段。

**對比方案 A（reject）**：每條 CR 一個獨立 task / 一個獨立 commit。

**理由**：
- 四條 CR 在 `docs/sidecar-api.md` 同一個檔、區段相鄰（§三 endpoint 表 / §三-bis ERROR_CODES / §四 SSE event）。
- 局部 patch 風險：改 §四 `qa_answer` 時可能漏改 §四 `usage_summary`（同一 SSE 表內），跨 patch reviewer 要重新 grep 一次。
- 一次重寫對 reviewer 友善，diff 可讀（同一 hunk）。

**取捨**：commit 可以細分為「docs/sidecar-api.md sync」+「docs/qa-agent.md sync」兩個（task 級別仍是同一個 stage），不會破壞 commit hygiene。

## Risks / Trade-offs

- **`upsert_chunk` API 簽名 break**：caller 只有 `add_to_kb.py` 一處（grep 確認過），但未來若有人在 review 期間動到 KnowledgeBase 其他地方 import upsert_chunk 並呼叫，會 silently 拿 tuple 當 string 操作。**Mitigation**：本 change 完成後 `upsert_chunk` callsite grep 加進 defensive test（spec scenario `Dedup token format reserved` 改寫成 tuple-based assertion），CI 永遠擋住。
- **Hash dedup path 多一次 backend round-trip**：`exists_by_hash` 後再 `search_points` 查 hash filter，等於跑兩次 backend call。**Mitigation**：可以接受 —— `add_to_kb` 不是 hot path（一次 chunk add 跑一次），網路 latency 可忽略；未來想優化走 Decision 3 方案 A（擴 Protocol）。
- **docs 完整重寫風險**：§三 / §三-bis / §四 一次性重寫可能不小心動到本 change 範圍外的條目。**Mitigation**：tasks 內每條 CR 列具體 section heading + line range；review 階段對照 git diff 跟 success criteria 逐條核對。
- **`KB_EMBED_FAILED` rename 對非 production 客戶端的 impact**：codebus 是桌面 app、無第三方 SDK，但若有 dev tool / smoke test script 寫死了 `KB_EMBED_FAILED` 字面量（Bash test 文字 match），會炸。**Mitigation**：grep `KB_EMBED_FAILED` 在整個 repo（含 scripts/、docs/、tests/、tauri/、web/），rename 全擊穿。

## Migration Plan

無 backward-incompatible 對外影響。檢查清單：

- 既有 token_usage.jsonl / kb_growth.jsonl / sanitize_audit.jsonl 內舊行為產生的紀錄不變（`"rules-unknown"` 字串、`"dedup:hash"` entry_id、`KB_EMBED_FAILED` error code）—— 本 change 只影響「未來新寫入」的紀錄。
- 既有 KB collections 不需 re-embed（`upsert_chunk` 簽名改但 dedup 行為一致、Qdrant 點位不變）。
- 既有 SSE 訂閱者 / 前端 mock 不影響（前端尚未動工）。
- 全 suite baseline 823 passed → 預期 ~825-828 passed（本 change 預估 ~3-5 新測 + ~5-10 個 KB_EMBED_FAILED 字面量 rename 不變數量）。
