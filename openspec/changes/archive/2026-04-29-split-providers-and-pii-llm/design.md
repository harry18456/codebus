## Context

D-033 ADR 已敲定方向，本 change 是 D-033 Change A — 純後端 Provider 抽象重整。

**現狀**：
- `sidecar/src/codebus_agent/providers/protocol.py:94-113` — `LLMProvider` Protocol 同時宣告 `chat()` 與 `embed()` 為 union
- `sidecar/src/codebus_agent/providers/openai_chat.py` — 只實作 `chat()`，缺 `embed()`
- `sidecar/src/codebus_agent/providers/openai_embedding.py` — 只實作 `embed()`，缺 `chat()`
- `@runtime_checkable` 的 isinstance 因此不可用，type narrowing 在 caller 端只能靠 `ProviderRole` 旁路
- `sidecar/src/codebus_agent/sanitizer/engine.py:95-101` — SanitizerEngine 直接持有 `list[Rule]`，rules 是 `RegexRule` + `DetectSecretsRule` Protocol
- `sidecar/src/codebus_agent/providers/tracked.py:68-70` — TrackedProvider `ALLOWED_INNER_TYPES = {MockProvider, OpenAIEmbeddingProvider, OpenAIChatProvider}` 強制所有 inner 過 Pass 2，無例外

**Constraints**：
- D-015：Sanitizer 單向不可逆、無 reverse mapping
- D-022：`llm_calls.jsonl` 記 post-Pass 2 版本
- 「TrackedProvider 必過 sanitizer」是不變式；任何例外要 spec 級鎖死，避免後門被濫用
- spec 與 code allowlist 必同步（CLAUDE.md 不變式 #4）

**Stakeholders**：
- Sanitizer 子系統（既有 Pass 1 / 2 / 3 三段）
- Provider registry / TrackedProvider / Audit jsonl 群（D-021 / D-022）
- 未來 D-033 Change B 的 setting page / multi-provider pool（不直接消費 PIIProvider，但會在 setting page 提供「PII 偵測模式」切換）

## Goals / Non-Goals

**Goals**：

- Protocol 拆三介面對應三條 trust boundary（一般 LLM 看 redacted、Embedding 看 redacted、PII 看原文）
- TrackedProvider 對 LLM / Embedding 強制 Pass 2 不變；對 PIIProvider marker class **自動** bypass，外部無 flag 可開
- 既有 Sanitizer 行為（規則表、placeholder 格式、audit schema）**完全不變**，純結構重整
- 為未來 LocalLLMPIIProvider / OpenAIPIIDetectionProvider 鋪好擴充契約：新增 PII provider 只需進 `PII_ALLOWED_INNER_TYPES` + 新 spec Requirement，不動 SanitizerEngine 或 TrackedProvider 主軸

**Non-Goals**：

- 不引入具體 LLM PII 服務（Local / OpenAI 兩條都留給後續另開 change）
- 不動 ProviderRegistry lifecycle（建構期 freeze 維持）
- 不動 Sanitizer rules 內容、不 bump rules version（使用者不需重取 grant）
- 不動 D-033 Change B 的 schema（`llm.providers[]` / `llm.bindings`）
- 不為 PIIProvider 加 ProviderRole enum 成員（理由見 Decision 6）

## Decisions

### Decision 1: PIIProvider Protocol 採 detect-shaped、回傳 spans

PIIProvider 介面只暴露 `detect(text) -> list[PIISpan]`，由 SanitizerEngine 接收 spans 後自行套 placeholder + 寫 audit。

**Alternative considered**：PIIProvider 直接做完整 sanitize，回傳 `SanitizedResult`。

**為什麼選 detect-shaped**：placeholder 格式（`<REDACTED:kind#N>`）與 audit schema（`AuditEntry` 欄位、JSONL key）是 Sanitizer 不變式（D-015 / sanitizer-safety-chain），不該讓每個 PIIProvider 各自實作。把 placeholder + audit 集中在 SanitizerEngine 同時讓「rule + LLM 混用」變自然 — 未來可以同時注入 RuleBasedPIIProvider 跟 LLMBasedPIIProvider，Engine 拿兩者 spans 做 union（雖然這個混用模式本 change 不實作，但介面預留空間）。

**PIISpan 結構**：reuse 既有 `sanitizer/rules.py:RuleMatch`（`rule_id, kind, start, end, value`）但改名為 `PIISpan` 並搬到共用位置（providers/pii.py 或 sanitizer/types.py — 見 Decision 8）。

### Decision 2: TrackedProvider 用 marker dispatch，不拆 TrackedPIIProvider

TrackedProvider 維持單一 class，在 `__init__` 看 `type(inner)` 是否在 `ALLOWED_INNER_TYPES` 還是 `PII_ALLOWED_INNER_TYPES` 決定 mode（`"llm"` / `"pii"`）。LLM mode 強制 Pass 2、PII mode 自動 bypass。

```python
class TrackedProvider:
    ALLOWED_INNER_TYPES: ClassVar[frozenset[type]] = frozenset(
        {MockProvider, OpenAIChatProvider, OpenAIEmbeddingProvider}
    )
    PII_ALLOWED_INNER_TYPES: ClassVar[frozenset[type]] = frozenset(
        {RuleBasedPIIProvider, MockPIIProvider}
    )

    def __init__(self, inner, *, sanitizer, sanitizer_audit, role, ...):
        inner_type = type(inner)
        if inner_type in self.ALLOWED_INNER_TYPES:
            self._mode = "llm"
            # 強制要求 sanitizer / sanitizer_audit 不為 None
        elif inner_type in self.PII_ALLOWED_INNER_TYPES:
            self._mode = "pii"
            # 不接受 sanitizer 注入（避免假裝過 Pass 2 的混淆）
        else:
            raise TypeError(...)
```

**Alternative considered**：拆 `TrackedPIIProvider` 獨立 class，僅暴露 `detect()`。

**為什麼選 marker dispatch**：
- TrackedProvider 是 audit 的單一入口，拆兩個 class 等於 audit 寫入路徑兩條 — 雖然「乾淨」但 maintenance 成本兩倍（pricing.py / llm_call_logger.py / usage_tracker.py 都要兼容兩個 wrapper）
- Marker dispatch 的「破口收縮」效果一樣強：`PII_ALLOWED_INNER_TYPES` 是一個獨立 frozenset，spec 級 enumerated；外部不能透過 flag 開洞、只能透過進這個 set 開洞，而進 set 必過 `/spectra-propose`
- ProviderRegistry 既有 wrapping 邏輯不需要為了 PII 多一條分支 — 雖然 PII provider **不會**進 ProviderRegistry（registry 是 caller-side dispatch、PII 不從 caller 來），但 TrackedProvider 仍然是 PII provider 的 audit wrapper

**Inner mode 校驗時機**：建構期 + 每次 `chat()` / `embed()` / `detect()` 呼叫前用 `_assert_mode("llm")` / `_assert_mode("pii")` guard，呼叫錯方法直接 raise。

### Decision 3: PIIProvider.detect() 一律 async

`async def detect(self, text: str) -> list[PIISpan]` — 即使 RuleBasedPIIProvider 內部是純 regex（無 IO）也宣告為 async。

**Alternative considered**：sync `detect()`，未來 LLMBasedPIIProvider 自己用 `asyncio.run()` 包。

**為什麼選 async-first**：
- SanitizerEngine.sanitize() 既有呼叫端：TrackedProvider._sanitize_messages（在 async chat 內）、Pass 1 Scanner（在 `asyncio.to_thread` 內）、Pass 3 Q&A `add_to_kb`（在 async FastAPI handler 內）— **三條都已是 async context**，sanitize 改 async 沒有破壞性
- 若 detect 是 sync、未來 LLMBasedPIIProvider 在 sanitize 流程中要打 LLM，要嘛用 `asyncio.run`（會炸 nested loop）要嘛用 thread pool（block event loop）— 都比 async-first 醜
- RuleBasedPIIProvider 加一個 `async def` 包 sync regex 沒有性能成本（無 await suspension point）

**SanitizerEngine.sanitize 也改 async**：caller 端要對齊 — `_sanitize_messages` 已在 async function 內，加 `await` 即可；Pass 1 / Pass 3 既有呼叫端跟著改。

### Decision 4: RuleBasedPIIProvider 包現有 default_rules()，rules 內容不動

`RuleBasedPIIProvider` 建構時接受 `rules: list[Rule] | None`，不傳就 `default_rules()`。`default_rules()` 函數本身留在 `sanitizer/rules.py` 不動（既有 RegexRule / DetectSecretsRule pattern 表全保留）。

```python
class RuleBasedPIIProvider:
    def __init__(self, rules: list[Rule] | None = None, *, config: SanitizerConfig | None = None):
        self._rules = list(rules) if rules is not None else default_rules()
        self._config = config

    async def detect(self, text: str) -> list[PIISpan]:
        # 把現有 SanitizerEngine._gather_matches 的邏輯搬過來
        # 回傳 PIISpan 而非 RuleMatch（同欄位、不同名）
        ...
```

**Alternative considered**：把所有規則邏輯搬進 `providers/pii.py`，廢掉 `sanitizer/rules.py`。

**為什麼選包裝**：
- `sanitizer/rules.py` 跟 sanitizer 子系統的 `audit.py` / `config.py` 緊耦合（共用 `RuleMatch` / `Rule` Protocol），整套搬會碰太多既有測試
- 「不動 rules 內容」是本 change 的 Goal — 包裝層級調整最低破壞
- 未來 LLMBasedPIIProvider 也是同層級實作（`providers/pii.py` 內），跟 RuleBasedPIIProvider 同一個檔案、同一個 Protocol

**SanitizerEngine 拿 PIIProvider 取代 rules**：

```python
# Before
class SanitizerEngine:
    def __init__(self, rules: list[Rule] | None = None, *, config=None):
        self._rules = list(rules) if rules is not None else default_rules()

# After
class SanitizerEngine:
    def __init__(self, pii_provider: PIIProvider, *, config=None):
        self._pii_provider = pii_provider
```

工廠 helper（`make_default_engine()`）回傳 `SanitizerEngine(RuleBasedPIIProvider())`，舊呼叫端改 import。

### Decision 5: PII 偵測 audit 共用 llm_calls.jsonl，加 role: "pii_detection"

未來 LLMBasedPIIProvider 內部呼叫 LLM 時，audit 走既有 `llm_calls.jsonl` + `token_usage.jsonl`，加兩個欄位值：

- `role: "pii_detection"` — 既有 role 欄位的新合法值
- `sanitizer_pass2_applied: false` — 既有 boolean 欄位的合法用法

本 change **不**新增 jsonl 檔案、**不**改 schema 結構（純 additive value）。Trust Layer UI（O-04 LLM Call Inspector）顯示時用 `role` 過濾分流 — 預設過濾掉 `role=pii_detection`，標小提示「另有 N 筆 PII 偵測 call」可展開。

**本 change 仍要寫的**：
- 留下 schema 級的「`role: pii_detection`是合法值」spec Requirement（未來 LLMBasedPIIProvider 直接用，不用再開 spec change）
- 但本 change **不**有任何寫入 `role: "pii_detection"` 的 audit code path（因為 RuleBasedPIIProvider 不打 LLM、不寫 llm_calls.jsonl；MockPIIProvider 也不寫，它是 pure mock）

**Alternative considered**：另開 `pii_provider_calls.jsonl` 獨立 audit lane。

**為什麼選共用**：
- 多一個 jsonl = 多一條 caller-side wiring + 多一個 path 不變式 + Trust Layer UI 多一個資料源
- `role` 欄位本來就是 schema 設計用來分流的（D-022），用既有欄位最自然
- 兩條 audit lane 不會減少 trust 透明度 — 實際透明度由 UI 預設過濾 + 提示「另有 N 筆」決定，跟檔案位置無關

### Decision 6: ProviderRole enum 不加 PII_DETECTION，audit role 走獨立 Literal

`ProviderRole` enum 維持四個成員（REASONING / JUDGE / CHAT / EMBED）不動。audit 的 `role` 欄位 type 改為獨立 Literal：

```python
AuditRole = Literal["reasoning", "judge", "chat", "embed", "pii_detection"]
```

**Alternative considered**：在 `ProviderRole` enum 加 `PII_DETECTION = "pii_detection"`。

**為什麼分開**：
- `ProviderRole` 是 **caller-side dispatch 軸**：Module 4 Explorer 拿 `registry.get(ProviderRole.REASONING)` 是業務語意。PII 偵測**不從 caller 來**、是 Sanitizer 內部子系統 — 它不該進 ProviderRole（業務層拿不到 / 不該拿到 PII provider）
- 加進 ProviderRole 會讓 ProviderRegistry 多一個 trap：caller 寫 `registry.get(ProviderRole.PII_DETECTION)` 就會炸（registry 不會 register PII provider）— 反而比現在多一道意外
- 兩個 enum / Literal 的命名分工：`ProviderRole`（caller-side dispatch）/ `AuditRole`（audit jsonl 欄位）— 有重疊但不相等，displayed 給 UI 的「role」面板可以同時消費

### Decision 7: MockProvider 同時實作 LLMProvider + EmbeddingProvider，不拆兩個 Mock class

`MockProvider` 維持單一 class，同時實作 `LLMProvider` + `EmbeddingProvider` 兩個窄 Protocol（一個 class 可以同時實作多個 Protocol，Python 結構性子型別無 nominal 限制）。

**Alternative considered**：拆 `MockChatProvider` + `MockEmbeddingProvider` 兩個 class。

**為什麼不拆**：
- 既有 MockProvider 已經同時提供兩個方法，拆會破壞 ~50 個既有測試
- 「同時實作多個窄 Protocol」是 Python idiom，沒有反 pattern
- 拆兩個 Mock 會讓測試 setup 多寫一段（兩個 mock 各自實例化），複雜度往呼叫端推
- TrackedProvider mode dispatch 看的是 `type(inner)`，MockProvider 同時被當 LLM 跟 Embedding inner 用 — 但因為呼叫端決定要 `chat()` 還是 `embed()`，誰錯誰自己 raise（TrackedProvider 的 mode guard 不會混淆）

**新增 MockPIIProvider** 是獨立 class（不跟 MockProvider 合併）— 因為 PIIProvider 走完全不同的 detect-shape API，硬塞 MockProvider 反而違反 Decision 1 的「detect-only」邊界。

### Decision 8: 檔案位置 — providers/pii.py（不放 sanitizer/）

PIIProvider Protocol、實作（RuleBasedPIIProvider / MockPIIProvider）、雙 allowlist 邏輯都放 `sidecar/src/codebus_agent/providers/pii.py`。

**Alternative considered**：放 `sidecar/src/codebus_agent/sanitizer/pii_provider.py`。

**為什麼選 providers/**：
- PIIProvider 是 Provider 抽象家族成員（與 LLMProvider / EmbeddingProvider 同層級）— 跟 TrackedProvider 雙 allowlist 邏輯緊耦合，放同個 package 比較好維護
- 「Provider」是 trust boundary 的單位，「Sanitizer」是消費 Provider 的 pipeline — providers/ 放介面，sanitizer/ 放使用
- 未來 LocalLLMPIIProvider 會 import OpenAI client / instructor — 跟 providers/openai_chat.py / openai_embedding.py 是同類資料流，放一起符合 cohesion

**`PIISpan` dataclass 共用位置**：放 `providers/pii.py`，`sanitizer/rules.py` 既有 `RuleMatch` 改為 PIISpan 的 alias（或保留 RuleMatch 作為 sanitizer 內部使用、PIISpan 作為跨子系統契約 — 二擇一在實作期決定，spec 不綁）。

## Risks / Trade-offs

- **Risk**：BREAKING change 對既有測試 / mock / type hint 的衝擊。
  → **Mitigation**：MockProvider 不拆（Decision 7）；Protocol 拆分用 import alias 過渡一個 commit（`from .protocol import LLMProvider, EmbeddingProvider`，舊的 union LLMProvider 直接刪），既有 `OpenAIChatProvider` / `OpenAIEmbeddingProvider` 實作不動 — type hint 改動範圍可控
- **Risk**：SanitizerEngine.sanitize() 改 async 影響三條既有呼叫端（TrackedProvider / Pass 1 Scanner / Pass 3 Q&A）。
  → **Mitigation**：三條都已在 async context（Decision 3 已驗證），改 async 是加 `await` 而非結構變動。寫測試覆蓋三條呼叫端的 async 路徑
- **Risk**：TrackedProvider mode dispatch 讓 class 職責變寬（同時管 LLM / PII）。
  → **Mitigation**：mode 由 `type(inner)` 在 `__init__` 一次決定後 frozen；`chat()` / `embed()` / `detect()` 各自 `_assert_mode()` guard；單元測試三條 mode 各自獨立覆蓋
- **Risk**：未來 LLM PII provider 需要打 OpenAI / 本地 LLM，那條 call 的 audit 是「半個 TrackedProvider」（要 audit、不要 Pass 2）— 邊界容易模糊。
  → **Mitigation**：本 change 留下 spec 級 `role: "pii_detection"` + `sanitizer_pass2_applied: false` 合法用法，但**不**實作。具體 LLMBasedPIIProvider 那條 change 屆時要決定是「PIIProvider 內部包另一個 TrackedProvider with mode='pii'」還是「PIIProvider 自己直接 IO + 自己寫 audit」— 本 change 不替它決定（Open Question 1）
- **Risk**：spec / code allowlist 同步漏改（CLAUDE.md 不變式 #4）。
  → **Mitigation**：寫一個防護測試（pattern 同既有 `tests/test_no_jsonl_literal_drift.py`）— 用 source-grep 鎖死 `PII_ALLOWED_INNER_TYPES` 的內容必須匹配 spec Requirement 列出的 class

## Migration Plan

純後端結構重整、無 schema migration、無資料遷移。

**步驟（建議按 tasks.md 逐項，但邏輯依賴順序如下）**：

1. **新增 PIIProvider Protocol + PIISpan dataclass + RuleBasedPIIProvider + MockPIIProvider**（providers/pii.py）— RED tests 先寫
2. **拆 LLMProvider Protocol** — `LLMProvider` 變窄（chat-only），新增 `EmbeddingProvider`（embed-only）；既有實作不動，只改 type hint
3. **TrackedProvider 加 PII_ALLOWED_INNER_TYPES + mode dispatch**（providers/tracked.py）— RED tests 先寫，紅綠循環
4. **SanitizerEngine 換 PIIProvider injection**（sanitizer/engine.py）— sanitize 改 async；既有測試 fixture 改用 `RuleBasedPIIProvider()` 替代 `default_rules()`
5. **Pass 1 / Pass 2 / Pass 3 三條呼叫端對齊 async**
6. **audit role / sanitizer_pass2_applied 欄位 spec 升級**（usage-tracking spec delta）
7. **Spec drift 防護測試** — `PII_ALLOWED_INNER_TYPES` 同步檢查
8. **文件對齊** — `docs/llm-provider.md`、`docs/sanitizer.md`、`CLAUDE.md`「三段 Sanitizer」段落

**Rollback strategy**：純後端 git revert 即可 — 無 schema / 無資料遷移、jsonl 格式向後相容。

## Open Questions

1. **未來 LLMBasedPIIProvider 內部如何 audit LLM call？** — 兩條候選：(a) PIIProvider 內部包另一個 TrackedProvider 用 `_mode="pii"`、(b) PIIProvider 自己直接呼 LLM client + 自己寫 audit jsonl。本 change 不決定，但留下 spec 級「`role: "pii_detection"` + `sanitizer_pass2_applied: false` 是合法值」，兩條都能對齊
2. **`PIISpan` vs 既有 `RuleMatch` 的命名**：實作期決定是「rename」還是「alias 兩名共存」。spec 只定義 PIISpan 介面（5 欄），不規定 sanitizer 內部要不要保留 `RuleMatch` 同義名
3. **Spec drift 防護測試的 source-grep 規則**：要不要也鎖死「`PII_ALLOWED_INNER_TYPES` 不能寫在 spec 以外的地方」？目前 tracked.py 與 spec 兩處有 enumeration — 跟既有 `ALLOWED_INNER_TYPES` pattern 對齊處理即可
