# LLM Provider 抽象介面

> 來自 D-003：MVP 只接指定 LLM 供應商 API，但留 Provider 抽象層讓 Phase 2 可切 Ollama / 企業自架。
> 實作位置：Python Sidecar `agent/providers/`。

---

## 一、為什麼要抽象

1. **資安 claim 精確性**（D-009）：之後要支援「code 完全不送第三方」時可切本地模型
2. **LLM 供應商 API 變動風險**：若 rate limit 或計費變動，可臨時切備援（D-007 cost 保底）
3. **Golden sample regression**：同一 prompt 對多 provider 跑，比品質

**不做**：MVP 不實作多 provider，只實作指定 LLM 供應商 API 的 Adapter，但介面要先定好。

---

## 二、介面定義（Python `Protocol`）

```python
from typing import Protocol, AsyncIterator, Literal
from dataclasses import dataclass
from enum import StrEnum

@dataclass
class Message:
    role: Literal["system", "user", "assistant", "tool"]
    content: str
    tool_call_id: str | None = None


class ProviderRole(StrEnum):
    """呼叫端語意分類（llm-role-routing change，2026-04-20 落地）。

    四值固定，不含 vision / multimodal 維度（D-028）。呼叫端不再
    直接抓「chat provider」，而是 `registry.get(ProviderRole.X)`。
    """
    REASONING = "reasoning"  # Explorer ReAct / Tutorial Generator（Module 5）— Opus 等級
    JUDGE     = "judge"      # Relevance Judge / Coverage Checker — Haiku 等級
    CHAT      = "chat"       # Q&A ReAct — Sonnet 等級
    EMBED     = "embed"      # Scanner / Q&A embedding — 獨立 embedding model


@dataclass(frozen=True)
class RoleConfig:
    """綁一個 `ProviderRole` 到具體的 provider + 預設參數。

    Role 級預設值由 llm-role-routing design §2 定死：

    | Role      | temperature | max_tokens |
    |-----------|------------:|-----------:|
    | REASONING |         0.2 |       8192 |
    | JUDGE     |         0.0 |        256 |
    | CHAT      |         0.3 |       4096 |
    | EMBED     |           — |          — |

    `temperature=0.2` / `max_tokens=None` 為欄位層級 fallback；
    載入 config 時若未指定則套 role 預設值。
    """
    provider_id: str
    model: str
    temperature: float = 0.2
    max_tokens: int | None = None

@dataclass
class ToolSpec:
    name: str
    description: str
    parameters: dict  # JSON schema

@dataclass
class ToolCall:
    id: str
    name: str
    arguments: dict

@dataclass
class ChatResponse:
    content: str | None
    tool_calls: list[ToolCall]
    usage: "Usage"
    finish_reason: Literal["stop", "tool_calls", "length", "content_filter"]

@dataclass
class Usage:
    """每次 provider call 的用量 — 所有 chat / structured / embed 都必回。

    由 `UsageTracker`（`agent-core.md` §十三）統一收集、寫 `token_usage.jsonl`、
    SSE emit `usage_delta`、Budget 控制即時反饋。詳見 D-021。
    """
    call_type: Literal["chat", "chat_structured", "chat_stream", "embed"]
    model: str                          # provider 回的 model id（e.g. "gpt-4o", "text-embedding-3-small"）
    prompt_tokens: int = 0
    completion_tokens: int = 0
    embed_tokens: int = 0               # embed 專用；chat 為 0
    cost_usd: float | None = None       # provider 有回才填
    estimated: bool = False             # True = 本地 tiktoken 估算（provider 沒回實際 token）

@dataclass
class EmbedResponse:
    """embed() 回傳 vector + usage。"""
    vectors: list[list[float]]
    usage: Usage


class LLMProvider(Protocol):
    name: str  # "contest-openai" / "contest-claude" / "ollama-local"

    async def chat(
        self,
        messages: list[Message],
        *,
        tools: list[ToolSpec] | None = None,
        temperature: float = 0.2,
        max_tokens: int | None = None,
        response_format: dict | None = None,  # JSON schema mode
    ) -> ChatResponse: ...

    async def chat_stream(
        self,
        messages: list[Message],
        **kwargs,
    ) -> AsyncIterator[str]: ...

    async def chat_structured(
        self,
        messages: list[Message],
        *,
        response_model: type["BaseModel"],
        max_retries: int = 2,
        temperature: float = 0.2,
    ) -> "BaseModel":
        """回傳 Pydantic model；schema 驗證失敗自動重試。
        MVP 由 Instructor 實作（見 D-012 / agent-core.md §五）"""
        ...

    async def embed(
        self,
        texts: list[str],
        *,
        model: str | None = None,
    ) -> EmbedResponse:
        """回 vectors + usage。若 provider API 沒回 token 數，
        用 tiktoken 本地估算後 `usage.estimated=True`（D-021）。"""
        ...

    @property
    def context_window(self) -> int: ...

    @property
    def embedding_dim(self) -> int: ...
```

---

## 三、MVP 實作：`ContestProvider`

```python
class ContestProvider(LLMProvider):
    name = "contest"

    def __init__(self, config: ContestConfig):
        self._chat_client = ...  # openai.AsyncClient(base_url=config.chat_endpoint, api_key=config.api_key)
        self._embed_client = ...
        self._chat_model = config.chat_model
        self._embed_model = config.embed_model
```

**關鍵責任**
- 統一把 provider 原生 response 轉回 `ChatResponse`
- 錯誤碼映射：429 → `LLM_RATE_LIMIT`、500 → `LLM_UPSTREAM`、超 context → `LLM_CONTEXT_OVERFLOW`
- 自動 retry（exponential backoff），3 次失敗往上拋
- 輸出不含 API key 的 log

---

## 四、Phase 2 預留：`OllamaProvider`

不在 MVP，但介面已定，未來可做：

```python
class OllamaProvider(LLMProvider):
    name = "ollama"

    def __init__(self, base_url: str = "http://127.0.0.1:11434"):
        ...
```

**評估點**（啟用前必過）
- Golden sample 召回率 ≥ 主 Provider 的 80%
- 平均 Explorer Agent 一輪步數 ≤ 主 Provider 的 1.5 倍（否則太笨繞路）
- Demo 機實測記憶體占用可接受

---

## 五、Provider 選擇規則（role map — llm-role-routing）

`config.json` 採 `llm.roles` 映射（llm-role-routing design §4）：每個
`ProviderRole` 綁一個 `provider_id` + `model`，呼叫端用
`registry.get(ProviderRole.X)` 分發，不再走平面的
`chat_provider` / `embed_provider` 欄位。

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

- `llm_disabled: true` → kill switch，所有 provider call 直接丟 `LLMDisabledError`（本節下方 §六 還有說明）
- M1 四 role 全指向 `mock`；真 vendor adapter（M3 前另開 change）接上後只需改 `provider_id` 與 `model`，registry 結構不動
- Role 級預設 `temperature` / `max_tokens` 定義見 §二 `RoleConfig`；payload 可 override
- 未知 role key（e.g. `"vision"`）parse 時 raise `ValueError` 並列出四個合法值（D-028 不預埋 capability 維度）
- Phase 2 `OllamaProvider`：`"chat": { "provider_id": "ollama", "model": "..." }` 即可切本地，不需動 registry API

---

## 六、與資安連動（D-011）

### PII / Secret pre-flight（D-015）
**所有 provider 的 `chat` / `embed` 呼叫前**，統一過 D-015 Sanitizer 的 pre-flight 層（詳見 `sanitizer.md` §三第二段）：

```python
clean_messages = sanitizer.scrub(messages)  # 把 email / IP / token / PEM 替換成 <REDACTED:kind#N>
response = await provider.chat(clean_messages, ...)
```

這層在 Provider 之外，所以換 provider 不會漏掉；Sanitizer 另有 Scanner 入庫前（第一段）與 Q&A `add_to_kb` 前（第三段）兩層。

### Kill switch
`config.llm_disabled = true` 時，provider call 直接丟 `LLMDisabledError`，Agent 層需 handle 成友善 UI。

### 稽核 log
每次呼叫寫 `audit.jsonl`：時間、provider name、token 數、（hash 過的）prompt 摘要。**不存原文**。

---

## 七、測試契約

- [ ] `MockProvider`（測試用，吐預設 response），用在單元測試
- [ ] `ContestProvider` 有 integration test（環境變數注 key）
- [ ] Provider 切換 smoke test（相同 prompt，不同 provider，output schema 一致）

---

## 八、MVP 不做

| 項 | 原因 |
|---|---|
| 真的做 OllamaProvider | D-003 已決 |
| 多 provider 動態 fallback | 複雜度 vs MVP 價值 |
| Token 級 streaming tool call | LLM 供應商 API 支援度不一，MVP 等整段回 |
| Fine-tune / LoRA 管理 | 超出範圍 |
| Vision / 多模態 | 延後至 Phase 2，見 D-028（Scanner 已保留圖片 metadata、Protocol 擴充為 additive） |
