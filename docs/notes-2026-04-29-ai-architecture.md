# 2026-04-29 AI 架構討論整理

> 討論紀錄（**非 spec**）。背景是 qa-overlay-p0 archive 後評估 Phase 7 該怎麼走，從「Phase 7 跟 AI 有關嗎」一路聊到 ReAct 本質、能力 tier、provider routing、prompt caching。
>
> 這份紀錄的目的：把當下達成的共識凍結下來，避免之後決策時忘記為什麼這樣選。**不是規範**，spec 該有的決策請各自走 `/spectra-propose` 開新 change。

---

## 1. Phase 7 怎麼安排（先跳過嗎？）

**問題**：Phase 7 demo prep 跟 AI 相關嗎？要不要直接跳過 Phase 6 step 29 + D-033 Change B 先跑 Phase 7？

**結論**：Phase 7 是**混合**（不純 UI）。最 AI 相關的子步驟是「跑一輪真 e2e」— 起 sidecar + OpenAI key + 跑全鏈。**建議：不整個跳，但「e2e 驗證」這一個子步驟先插隊跑**（~1d，~$1 OpenAI 開銷）。

**Takeaway**：前面 8 個 change 都是 vitest / pytest 全綠 + 0 OpenAI call，等於「單元 / 整合面驗過、production traffic 完全沒驗」。e2e 結果會回頭重塑 step 29 / D-033 B / Generator verifier 的設計優先序。**不做 e2e 等於閉眼設計下一步**。

---

## 2. AI 使用是不是已定義好

**問題**：怎麼使用 AI 應該都在 spec 定義好了吧？

**結論**：分三層 — **「契約」定義齊（spec）+ 「prompt 文字」已寫（code）+ 「真流量品質」完全沒驗（Phase 7 才知道）**。

**Takeaway**：spec 跟 unit test 一樣，寫好不代表跑過會 pass。Phase 7 prompt 調整不是「定義 AI 怎麼用」（那 spec 寫了），而是「真 LLM 看了 prompt 真的有照做嗎」的實證調校。

---

## 3. 能用 Skill 模式控制 CodeBus AI 嗎

**問題**：能不能像 Claude Code 用 spectra-apply skill 那樣，用 Skill markdown 控制 CodeBus 的 AI？

**結論**：三個落點，**Skill 的「verify-and-loop 模式」最值得套**：

1. **Light：prompt 改 markdown 格式**（純重構，相容 D-012）
2. **Medium：user-extensible workflow**（要重設信任邊界，Phase 2）
3. **Strong：以 skill 取代 ReAct loop**（跟 D-012 對撞，不建議）
4. **Verify-and-loop**（**最適合 Generator**）：LLM 生 → verifier.py 驗 → 不過退回讓 LLM 修

**Takeaway**：Generator 寫 station markdown 的格式錯（漏 frontmatter 欄、`<Checkpoint>` 拼錯、`### Quiz` 階層錯、`related_stations` 引用不存在的站）都是**結構性錯**，verifier.py 100ms 抓得完。靠 prompt 多寫幾句約束不可靠，verify-and-loop 才 deterministic。

---

## 4. 開 propose / apply 會不會燒錢

**問題**：開 propose 會不會用 `.env` 的 OpenAI key 額外花費？

**結論**：**完全不會**。propose / apply 只走 Claude Code 的 token，**0 OpenAI cost**。OpenAI key 只在 sidecar 真的跑 agent 時才用。

**Takeaway**：成本分清 — **Claude Code 對話成本**（訂閱付，每個 propose / apply 工作流）vs **CodeBus 跑 demo 成本**（OpenAI key 燒，只在真實 e2e 才花）。前者大、後者小但 Phase 7 會花到。

---

## 5. ReAct + Agentic AI 的本質

**問題**：ReAct 是什麼？LLM Provider 怎麼做到「規劃 → 執行 → 確認 → 結束」？

**結論**：**LLM 本身沒做到，是外面的 loop 做的**。LLM 是無狀態接龍機（input messages → output text），「規劃 / 行動」是 application code 包的 while loop（呼 LLM → parse 行動 → 跑工具 → 加歷史 → 重複）。

```python
# CodeBus Explorer 的 loop 骨架（agent/explorer.py）
while True:
    if _should_stop(state, cancel_event):  # 終止 1：budget / cancel
        break
    thought, tool_calls = await _think(state, provider, prompt)
    if not tool_calls:                     # 終止 2：LLM 主動 finish
        break
    results = await _execute_tools(tool_calls, tools)
    _append_observations(state, tool_calls, results)
    if judge.approve(state) and coverage.satisfied(state):  # 終止 3：判官
        break
    state.step_count += 1
```

**Takeaway**：「Agentic」不是 LLM 屬性、是 loop 架構屬性。同樣 LLM 給不同 loop wrap 出來行為差很多。CodeBus 自寫 ReAct loop（D-012）就是因為這樣：終止條件 / 工具白名單 / 可審計都自家鎖死，不靠框架（LangChain / AutoGen）。

---

## 6. Claude Code 也是 Agentic AI

**問題**：Claude Code 跟 CodeBus 的 agent 是同個概念嗎？

**結論**：**完全一樣**。Claude Code = LLM（Anthropic Claude）+ 外面 loop（Claude Code harness）+ tool 整合。`/spectra-apply` 那個 session 就是經典 ReAct：每輪看歷史 → 預測下一個 tool call → harness 執行 → 結果回灌 → 再呼 LLM。

**核心差異**：判官誰當。

| 機制 | CodeBus Explorer | Claude Code |
|---|---|---|
| LLM | OpenAI gpt-4o-mini | Anthropic Claude |
| Loop 在哪 | sidecar/.../agent/explorer.py | Claude Code harness（內部） |
| Tool 機制 | Instructor 鎖 Pydantic schema | Anthropic 原生 tool_use API |
| 終止 - 自然 | `tool_calls=[]` | 純文字 response（無 tool call） |
| 終止 - 強制 | step / token budget | context window 滿 |
| 終止 - 判官 | Judge + Coverage 兩層 | **user 是判官** |

**Takeaway**：「user-in-the-loop ReAct」（Claude Code）vs「自主 ReAct」（Explorer）是兩種同源不同 setup 的 agent。前者便宜安全（user 看到歪掉會中斷）、後者靠 deterministic 護欄（Budget / Judge / Coverage / Sandbox）撐住。

---

## 7. LLM 太笨跑不起來 vs 工程護欄

**問題**：LLM 太笨是不是就跑不起來？

**結論**：**對，有能力門檻**。失敗模式：格式崩潰 / Tool 亂選 / 不會收尾 / Long-context 失憶 / Observation 幻覺 / Self-correction 失能 / Judge 失準。

**能力 tier**：

| Tier | 範例 model | ReAct 結果 |
|---|---|---|
| 0 | GPT-3、Llama-2 7B | 跑不起來 |
| 1 | GPT-3.5、Haiku 3 | 簡單 5 步 OK |
| 2 | **gpt-4o-mini ← CodeBus 在這層**、Sonnet 3.5 | 10-20 步 reliable |
| 3 | gpt-4o、Sonnet 4.6、Opus 4.7 | 50+ 步、能自我糾錯 |

**推低門檻的工程手段**：

| 手段 | CodeBus 是否在用 |
|---|---|
| Structured output (Instructor / Pydantic) | ✅ D-012 主軸 |
| Verify-and-loop | 部分（Sanitizer 算一種；Generator 還沒） |
| Tool 數量精簡 | ✅ D-017 |
| 多 agent（planner / executor / critic） | ❌ 沒做 |
| 強 model 跑判官、弱 model 跑執行 | ⚠️ 可選但目前都 mini |
| Few-shot examples | 部分 |

**Takeaway**：CodeBus 用 gpt-4o-mini 在 Tier 2 邊界，**正是因為這樣才需要 deterministic 護欄補智商**。Phase 7 e2e 真正要驗的是「這些護欄夠不夠補 mini」。如果不夠：
- 加 verifier loop（Generator 提案）
- Judge role 升 gpt-4o（一行配置）
- 整體升 Anthropic Haiku 4.5 / Sonnet 4.6

---

## 8. 擴充 LLM Provider 要注意什麼

**問題**：擴充其他 LLM provider 時要看相容性、context window、需要的參數對嗎？

**結論**：對，**而且還要看更多**。完整檢查清單分四大塊：

### 8.1 技術相容（會直接 crash）

| 項 | 範例差異 |
|---|---|
| API 認證 | OpenAI: `Authorization: Bearer`；Anthropic: `x-api-key`；Google: `?key=` query；Azure: 自家 deployment |
| Endpoint | `https://api.openai.com/v1/chat/completions`、`https://api.anthropic.com/v1/messages`、本地 `http://localhost:11434/v1` |
| Tool calling | OpenAI: `tools=[{"type":"function",...}]` → 回 `tool_calls`；Anthropic: `tools=[{...}]` → 回 `content: [{"type":"tool_use",...}]`；本地 model 通常無原生 tool calling |
| Streaming | OpenAI SSE: `data: {...}\n\n`；Anthropic: `event: ... data: ...` |
| Response 結構 | OpenAI: `choices[0].message.content`；Anthropic: `content[0].text` |
| System prompt | role=system；Gemini 用 `systemInstruction` 獨立欄 |

### 8.2 能力相容（會變慢變爛）

| 項 | 為何重要 |
|---|---|
| Context window | gpt-4o-mini 128K / Haiku 4.5 200K / Sonnet 4.6 1M / 本地 Llama-3 70B 8K-32K |
| Tool 同時數量 | 本地 model 5+ 工具就常選錯 |
| Output token 上限 | Generator 寫一站需要 4K+ max_tokens；某些 model 默認 1K 截斷 |
| Structured output 穩定度 | Llama-3 8B 完全不行 + Instructor 也救不了 |
| 指令遵從度 | Anthropic 對 negative constraint（「不要做 X」）服從更高 |
| 多輪 reasoning | Tier 1/2/3 差異 |

### 8.3 成本計費

| 項 | 細節 |
|---|---|
| Pricing 表 | `pricing.py` 要有 model 對應 USD per 1M |
| Tokenization | OpenAI tiktoken / Anthropic 自家 / Google sentencePiece，同一段字 token 數差 10-30% |
| Rate limit | 各家 Tier 1 vs Tier 4 差 100x |
| 費差 | gpt-4o-mini $0.15/$0.60、Haiku 4.5 $1/$5、Sonnet 4.6 $3/$15、Opus 4.7 $15/$75 |

### 8.4 安全治理（CodeBus 特有）

| 項 | 細節 |
|---|---|
| Sanitizer Pass 2 相容 | 新 provider inner 要納入 `ALLOWED_INNER_TYPES` 白名單（D-033） |
| Outbound endpoint 列入 grant | 換 provider 等於換 endpoint，bump rules_version 重新取得 user 同意 |
| Privacy 政策 | 各家「不訓練資料」承諾條款不同；Azure 給 BAA；本地 model 完全不出去 |
| PII Provider 是否獨立 | D-033 Change A 拆出去了，換 LLM provider 不影響 PII Provider |

**Takeaway**：D-033 Change A 把架構 prep 好了（LLMProvider Protocol + ALLOWED_INNER_TYPES allowlist），新 provider 接口工程量小（~200 LoC）；**真正大頭是 model qualification**（手動驗 1-2 天確認穩定性、cost 計算、long context 不退化）。

---

## 9. Per-Task 異質 Routing

**問題**：可以針對不同任務設不同 provider 嗎？（研究最好、檢查次好、RAG 普通）

**結論**：**完全可以、架構已 prep**。`_make_chat_provider_factory` 已把 7 個 lane 拆開（reasoning / judge / coverage / chat / generate / qa_agent / kb_*）。差的只是 config UI 跟 keyring 多 key。

**推薦 routing**：

| Role | 推薦 model | Provider |
|---|---|---|
| reasoning（Explorer 想）| Sonnet 4.6 / Opus 4.7 | **Anthropic** |
| generate（Generator 寫）| Sonnet 4.6 | Anthropic |
| judge（評答案）| gpt-4o | **OpenAI**（**故意跨家**避免 reasoning + judge 同腦背書）|
| coverage / qa / chat | gpt-4o-mini | OpenAI |
| embedding（KB build/query）| text-embedding-3-small | OpenAI（鎖死，換 model = KB 重建）|

**為何 Judge 故意跨 provider**：Judge 就是抓 reasoning 的錯。同 model 同 prompt 互看會腦補一致性（model bias 一致）。換不同 vendor 的 LLM 當 judge，**等於用兩個不同的「LLM 直覺」交叉驗證** — 這是 LLM eval 圈的 best practice。

**成本試算**（一輪完整 demo）：

| 配置 | 單次成本 |
|---|---|
| 全部 gpt-4o-mini（現況）| ~$0.30 |
| **異質 routing（推薦）** | ~$1.50 |
| 全部 gpt-4o | ~$3.00 |
| 全部 Sonnet 4.6 | ~$5.00 |

異質 routing 比全 mini 貴 5 倍，但**主要 cost 落在「最該花錢」的 reasoning + Generator**；比全 gpt-4o 省 50%、比全 Sonnet 省 70%。Demo 場景甜蜜點。

**Takeaway**：D-033 Change B（Settings）原本只規劃「single provider」，**應該擴大成「per-role profile」**。預設給三個 profile：保守（全 mini）/ 推薦（異質 routing）/ 最強（全 Sonnet/Opus），第一次裝完 wizard 直接選一個；Advanced mode 才開 per-row 自選。

---

## 10. Prompt Caching

**問題**：cache hit 是不是不會花錢？API 也有？其他家也有？

**結論**：**是大幅打折，不是免費**。各家對比：

| Provider | 折扣 | TTL | 觸發 |
|---|---|---|---|
| **Anthropic** | 寫 1.25x、**讀 0.1x（90% off）** | 5 min（beta 1 hour） | 顯式 `cache_control: {"type":"ephemeral"}` 標記，最多 4 個斷點 |
| **OpenAI** | **讀 0.5x（50% off）** | 5-10 min | **自動**（從 prompt 開頭算同前綴）|
| Google Gemini | 50%+ off + 儲存費 | 你選 | 顯式創 cache resource、reference cache_id |
| xAI Grok | ~75% off | 自動 | 自動 |
| 本地（vLLM / Ollama）| **真免費** | GPU 記憶體 | vLLM prefix caching；Ollama 同 conversation 自動續用 |

**對 CodeBus 的影響**：

| Module | 重複率 | Cache 效益 |
|---|---|---|
| Explorer ReAct | 一輪 10-20 step、同 system prompt 重複 | ★★★★★ |
| Generator | 一個教材 5-15 站、每站獨立 call | ★★★★ |
| Q&A ReAct | 5-10 step | ★★★ |
| Judge / Coverage | 每輪 1 次、prompt 短 | ★★（門檻可能還沒過 1024） |
| KB embedding | n/a | n/a |

**現況**：CodeBus 在 OpenAI gpt-4o-mini 上**已經自動有 50% 折扣**，但 token_usage.jsonl **沒記 `cached_input_tokens` 欄** → cost_usd 高估。

**改 Anthropic 後必須顯式標 cache_control**：

```python
system = [{
    "type": "text",
    "text": SYSTEM_PROMPT,
    "cache_control": {"type": "ephemeral"}
}]
```

**Takeaway**：**`token_usage.jsonl` schema 該補 cache 欄**：

```jsonl
{
  ...,
  "prompt_tokens": 5234,
  "cached_input_tokens": 4500,    // ← 新欄：本次命中 cache 的 input tokens
  "cache_creation_tokens": 0,     // ← 新欄：本次寫入 cache 的 input tokens (Anthropic 才有)
  "cost_usd": 0.00X              // 計算時要扣 cache 折扣
}
```

`pricing.py` 公式跟著改：

```python
cost = (prompt_tokens - cached_input_tokens) * input_rate \
     + cached_input_tokens * cached_rate \
     + cache_creation_tokens * write_rate \
     + completion_tokens * output_rate
```

**雷**：

- TTL 過期：long-running session 跑超過 5 min cache 失效，下一輪付寫入費（更貴）
- cache 內容變動：system 內含「目前時間」這種會變的字串 → 整個 cache 失效
- 量太小不快取：< 1024 tokens 不享 cache（Judge / Coverage 短 prompt 可能算不到）
- 不同 model 不共用 cache：跨 model routing 等於每 lane 各自快取
- cache cost 不是 0：寫入比正常還貴（Anthropic 1.25x），同 prompt 至少重用幾次才回本

---

## 出現過但還沒決定的 Action Items

| # | 動作 | 規模 | 影響的 module / spec |
|---|---|---|---|
| **A1** | **Phase 7 e2e 跑一輪驗 mini** | 1d, ~$1 OpenAI | 全鏈、決定 A2 / A4 / A5 優先序 |
| **A2** | **Generator verify-and-loop 提案** | propose 1d、apply 1d | Module 5 |
| **A3** | **token_usage.jsonl 補 cache 欄** | propose+apply 0.5d，0 OpenAI | D-021 schema、pricing.py |
| **A4** | **D-033 Change B 設計改 per-role**（不是 retrofit）| propose 1d | Settings UI / keyring / O-01 grant modal |
| **A5** | **Judge role 升 gpt-4o**（如果 e2e 證明 mini 不夠）| 1 行 config | Explorer / Coverage |
| **A6** | **AnthropicChatProvider class** | ~200 LoC + qualification 1-2d | provider 抽象、D-033 |
| **A7** | **Phase 6 step 29 三介入點** propose | propose 0.5d | UX |

**建議優先序**：A1 → 看結果決定 → A3（不看結果都該做）→ 看 e2e 結果分支決定 A2 / A4 / A5。A6 / A7 看 demo 死線壓力。

---

## 給未來自己的提醒

1. **把這份當 reference**，不是 spec。spec 該寫的決策走 `/spectra-propose` 開新 change，避免這份檔內容跟 D-XXX 漂移
2. **D-033 Change B 提案開出來時直接套 per-role profile 設計**（A4），不要先做 single provider 再 retrofit — retrofit 通常多花一倍工
3. **Phase 7 e2e 結果出來時回頭看這份**，特別是第 7 / 9 節 — 真實 mini 表現會打破或印證這裡的假設
4. **如果一年後重看這份**：CodeBus 的 LLM provider 應該已經多家、token_usage 應該已經有 cache 欄；如果沒有，問題出在哪裡值得回看
