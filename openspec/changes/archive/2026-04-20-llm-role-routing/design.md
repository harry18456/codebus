## Context

M1「power-on」封存後（`openspec/changes/archive/2026-04-19-m1-power-on/`），LLM Provider 層已具備：

- `LLMProvider` Protocol（`docs/llm-provider.md` §二）
- `MockProvider`（zero outbound 不變式靠它守）
- `TrackedProvider` wrapper（呼叫 `UsageTracker` → `token_usage.jsonl` + `LLMCallLogger` → `llm_calls.jsonl`）
- `ProviderRegistry` 實例化 guard（拒絕 unwrapped provider）

但 registry 現階段只支援「單一 chat provider + 單一 embed provider」平面結構（見 `docs/llm-provider.md` §五 config）。即將進入的 M2 / M3 / M4 會帶來六類新 call site（Explorer ReAct、Tutorial Generator、Relevance Judge、Coverage Checker、Q&A ReAct、Embedding），各有不同模型等級需求；若維持平面結構，任何 role 級的調整都會擴散到所有呼叫端。

本次重構的切入時機是 M2 Sanitizer 開工前——Pass 2 的 hook 點就在 provider 呼叫前，若先把 role 分發做完，Pass 2 的包裹鏈只需實作一次。

另外 D-028（2026-04-20）已決議 vision 延後至 Phase 2、介面不預埋 Capability enum，本次同步補齊 D-028 在 `docs/llm-provider.md` 與 `docs/module-5-generator.md` 的連動註記。

## Goals / Non-Goals

**Goals:**

- 將 `ProviderRegistry` 從「chat / embed 平面」升級為「role 分發」，呼叫端改為 `registry.get(ProviderRole.REASONING)`
- 新增 `RoleConfig` 帶每 role 預設參數（`temperature` / `max_tokens`），呼叫端可 override
- 保留 TrackedProvider 不變式：registry 實例化時每個 role 的 provider 都必須經 TrackedProvider 包裹，否則 raise
- 延伸 M1 既有的 zero outbound 不變式至 role 分發：所有 role 的 provider 在 M1 / M2 仍只能是 MockProvider（真 vendor adapter 由後續 change 接手）
- 確定 `config.json` 的 `llm.roles` map schema，為後續 config loader 鋪路

**Non-Goals:**

- ❌ 不實作 `Capability` enum（含 vision）— D-028 已決
- ❌ 不實作真 vendor adapter（如 `ContestProvider`）— M3 前另開 change
- ❌ 不做動態 fallback / cost-based routing — `docs/llm-provider.md §八` 已列 MVP 不做
- ❌ 不支援 runtime 動態切 role — role 由呼叫端在編寫時決定
- ❌ 不做 role-level rate limit / quota — 若有需求另開 change

## Decisions

### 1. ProviderRole 用四值 enum（不含 vision / multimodal 維度）

選 `StrEnum` 四值：`REASONING` / `JUDGE` / `CHAT` / `EMBED`。

| 替代方案 | 不採用原因 |
|---|---|
| 二值（CHAT / EMBED，維持現狀） | 無法區分 Explorer 強推理與 Judge 輕量判斷；呼叫端升級必須改 N 處 |
| 三值（REASONING / CHAT / EMBED） | Judge 合併到 REASONING 就用 Opus 跑單句判斷，成本浪費 |
| 五值以上（ADD COVERAGE / GENERATOR 等） | 過度切分，Coverage 用 JUDGE 夠用、Generator 用 REASONING 夠用；保留概念層，實作層不多分 |
| Capability-based routing（動態依任務能力需求選） | 複雜度高、動態決策難稽核；D-028 已決不預埋 Capability enum |

**四值對應到 call site**：

| Role | 典型 call site | 預設模型意圖 |
|---|---|---|
| `REASONING` | Explorer ReAct、Tutorial Generator（Module 5） | 強推理（Opus 等級） |
| `JUDGE` | Relevance Judge、Coverage Checker | 輕量判斷（Haiku 等級） |
| `CHAT` | Q&A ReAct | 中等（Sonnet 等級） |
| `EMBED` | Scanner / Q&A embedding | 獨立 embedding model |

### 2. RoleConfig 欄位與預設值

```python
@dataclass(frozen=True)
class RoleConfig:
    provider_id: str              # e.g. "mock" / "contest-openai"
    model: str                    # e.g. "claude-opus-4-7" / "text-embedding-3-small"
    temperature: float = 0.2      # chat 類 role 預設
    max_tokens: int | None = None # None = 依 provider 預設
```

每 role 的預設值由 spec 定死（見 specs/llm-provider/spec.md delta），避免配置隨人改漂移：

| Role | `temperature` | `max_tokens` |
|---|---|---|
| `REASONING` | 0.2 | 8192 |
| `JUDGE` | 0.0 | 256 |
| `CHAT` | 0.3 | 4096 |
| `EMBED` | — | — |

**替代方案**：

- 不定預設、全靠呼叫端帶參數 → 呼叫端容易忘、不一致；Judge 用預設的 `0.2` 會輸出不穩
- 預設寫死在 provider class → provider 要跨 role 重用就打架；留在 role 層更乾淨

### 3. Registry 仍維持實例化階段 guard、不在 runtime 檢查

M1 既有 `ProviderRegistry` 在「註冊時」就會檢查 provider 是否已被 TrackedProvider 包裹，拒絕 unwrapped 註冊。本次延伸規則：

- Registry 接受 `dict[ProviderRole, LLMProvider]`
- 每個 role 的 value 都必須先經 `TrackedProvider.wrap(provider)` 處理，否則 `__init__` raise
- runtime `get(role)` 只做 dict lookup，不重檢（效能、避免誤判）

**替代方案**：

- runtime 每次呼叫都檢查 → 額外開銷、重複工作
- 放寬 guard、允許 runtime 包裹 → TrackedProvider 套用漏洞風險（可能某條路徑忘記包）

### 4. Config schema 採 role map，保留 `llm_disabled` kill switch

```json
{
  "llm": {
    "llm_disabled": false,
    "roles": {
      "reasoning": { "provider_id": "mock", "model": "mock-reasoning" },
      "judge":     { "provider_id": "mock", "model": "mock-judge" },
      "chat":      { "provider_id": "mock", "model": "mock-chat" },
      "embed":     { "provider_id": "mock", "model": "mock-embed" }
    }
  }
}
```

M1 階段四個 role 全指向 `mock`；真 vendor 接上後（另一個 change）只需改 `provider_id` 與 `model`。

**替代方案**：

- 沿用現有 `chat_provider` / `embed_provider` 兩欄位 → 無法表達 role 差異
- 把 `RoleConfig` 所有欄位展平到頂層 `llm.*` → 四 role 五欄位 = 20 個鍵，可讀性差

### 5. MockProvider 支援多 role 識別

`MockProvider` 新增 `role: ProviderRole` 屬性，讓 `llm_calls.jsonl` 記錄可反查「這筆 call 走哪個 role」。測試端可以驗「Judge 的 call 應該走 `mock-judge` 不走 `mock-reasoning`」。

**替代方案**：

- 每 role 各一個 Mock class → 類別爆炸
- 單一 Mock 不帶 role → 稽核無法反查，之後 debug 痛

### 6. TrackedProvider 自動感知 role（不改呼叫端簽章）

`TrackedProvider.wrap(provider, role)` 在包裹時綁定 role；`llm_calls.jsonl` 的每筆紀錄自動帶 `role` 欄位，不需呼叫端主動傳。

審計欄位擴充（additive，不破壞既有 log 格式）：

```json
{
  "timestamp": "...",
  "role": "judge",
  "provider_id": "mock",
  "model": "mock-judge",
  "sanitizer_pass2_applied": false,
  "...": "..."
}
```

**替代方案**：

- 每次呼叫由呼叫端塞 role → 容易漏；稽核鏈可能斷
- 不記 role → 未來 debug / cost attribution 拿不出資料

## Risks / Trade-offs

- **[Risk] 呼叫端簽章改動擴散** → Mitigation：M1 尚未有生產呼叫端，僅測試碼；本次一次改完、tests/providers 覆蓋 100%
- **[Risk] Role 粒度不夠細，未來要分出 `COVERAGE` / `GENERATOR` 等新 role** → Mitigation：`ProviderRole` 為 enum，新增 role 是 additive；後續 change 可加、不 break 既有 call site
- **[Risk] 四 role 的預設 `temperature` / `max_tokens` 不合用** → Mitigation：呼叫端可 override；且這些值在 spec 裡明示、可日後 propose 調整
- **[Risk] D-028 連動更新漏掉** → Mitigation：tasks.md 明列兩條 doc 更新、具體引用行數
- **[Trade-off] RoleConfig 欄位固定在 dataclass，未來要加欄位需改 spec** → 接受：當前需求尚不清楚，硬加欄位反而做錯

## Migration Plan

- **M1 → 本 change**：registry 從 `{chat: ..., embed: ...}` 重構為 `{role: ...}` map；既有 MockProvider 實例重新註冊為四個 role；全測通過
- **無 rollback**：M1 尚未有生產呼叫端，失敗直接丟棄 branch 重來
- **後續 change 銜接**：真 vendor adapter change 只需為每個 role 的 `provider_id` 塞新 class 名即可；不再動 registry 結構

## Open Questions

- **`EMBED` role 的 `temperature` 欄位** → 目前 `RoleConfig` 一視同仁帶 `temperature`，但 embed 不吃此參數。是否分 `ChatRoleConfig` / `EmbedRoleConfig`？
  - **暫定**：維持單一 `RoleConfig`，embed 呼叫端忽略 `temperature` 欄位；簡化實作。若未來證明痛點再拆
- **config loader 在哪實作** → `config.json` schema 已定，但實際載入邏輯（`pydantic-settings` / 手寫）由未來 change 做。本 change 只給 schema + dataclass 定義
