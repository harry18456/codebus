# codex + Azure `response.failed` 診斷：deployment TPM 上限 + 端點限額執行差異

> 2026-06-04 ｜ 內部 troubleshooting 紀錄（非對外文件）。
> 結論：**這不是 codebus bug，是使用者 Azure deployment 的速率上限（TPM）設定問題。** 修法在 Azure 端。

## 摘要（TL;DR）

`agent.active_provider: codex` + codex azure profile（`gpt-5.4`）時，**每一次 codex 呼叫都失敗**，CLI/GUI 看起來像「沒回應」或靜默結束。

真因：該 `gpt-5.4` deployment 的 **token 速率上限 `x-ratelimit-limit-tokens = 4000`（TPM=4000）**。codex 的完整 agentic 請求輸入約 6–11K tokens，**單一請求就比每分鐘 token bucket（4000）還大、永遠塞不進去** → Azure 在 `/openai/responses` 串流中途丟 `response.failed` 並斷線（非串流則回乾淨的 `429`），codex 重試 5 次後 `turn.failed`。

**修法：到 Azure AI Foundry → `gpt-5.4` deployment → Edit → 把 Tokens-Per-Minute 調高（建議 ≥30K，多輪會累加）。**

跟 codex 版本、API key、codebus 設定、Azure 內容過濾**都無關**（下面有逐一排除的硬證據）。

## 症狀

- `codebus query` / `chat` 在 codex azure 下，RunLog `outcome` 可能誤標 `succeeded` 但 `tokens` 全 0、wiki 沒動、`events-*.jsonl` 只有 `spawn_start` → `spawn_end exit_code:1`，中間沒有任何 stream/turn event。
- 同一個 vault、同一把 key，**claude azure 正常**、只有 codex 壞。

## 環境與時間軸（local +0800）

- codex CLI：**0.137.0**，npm 套件 mtime `2026-06-04 20:55`（當天才升級）。
- Azure 資源：`2026msf13.cognitiveservices.azure.com`（East US 2），**claude 與 codex 是同一個資源、不同路徑/deployment**：
  - claude：`/anthropic/v1/messages`，deployment `claude-opus-4-6-2026V2`
  - codex：`/openai/responses`，deployment `gpt-5.4`
- 第一次失敗 `20:53`（**早於** codex 升級的 20:55）→ 不是 0.137 升級造成的。

## 診斷過程：三條走錯的線（留存以免重踩）

### 走錯的線 1 — `models_manager: missing field 'models'`（非致命雜訊）

`CODEBUS_FORWARD_AGENT_STDERR=1` 把 codex stderr 轉出來，第一眼會看到：

```
ERROR codex_models_manager::manager: failed to refresh available models:
  failed to decode models response: missing field `models`; body: {"data":[...],"object":"list"}
```

codex 自 v0.125 起「provider 自負 model discovery」，會打 `{base_url}/models`；Azure 回標準 `{"data":[...]}`，codex 想要頂層 `models` 欄位 → decode 失敗。

**但這是非致命雜訊**：看 codex 的 **stdout** 會發現它有走過它到 `turn.started`（issue openai/codex#11213 也指出此 refresh non-fatal）。真正的致命錯誤在後面。**教訓：別追第一個顯眼的 stderr error，先看 stdout 的 turn 結果。**

### 走錯的線 2 — codex 版本（降版無效）

逐版 `npm i -g @openai/codex@X` 後用真 codebus 重跑：**0.135.0 / 0.136.0 / 0.137.0 三版全部一樣失敗**。→ 不是版本 regression、降版不是解。

### 走錯的線 3 — 內容過濾 / Prompt Shield（不是）

把 codex **原版 14732 字 instructions** 用乾淨 UTF-8 bytes 直接打 Azure → **HTTP 200、`blocked:false`**（串流非串流都過）。再用「**內容無害**但塞到同樣 ~24KB」的請求 → **一樣失敗**。→ 跟內容無關、不是內容過濾。

## 真正的錯誤與真因

看 codex **stdout** 的真實序列：

```
thread.started → turn.started
error: Reconnecting... 1/5 (stream disconnected before completion: response.failed event received)
... 2/5 3/5 4/5 5/5
turn.failed: "stream disconnected before completion: response.failed event received"
```

Azure 在 `/openai/responses` 串流中送出 `event: response.failed` 後**直接斷線、不附原因**。架本機 logging proxy（codex base_url 指 `http://127.0.0.1`）抓到 codex 真實 request body（raw 44852 bytes）後逐欄剝離 + 換無害內容測，收斂出：

- **跟內容無關**：無害內容、同大小一樣掛。
- **跟大小有關**：成功的請求都 ~15KB（~3.7K tok）、失敗的都 ~24–45KB（~6–11K tok）。
- **非串流會回乾淨的 `429 Too Many Requests`（Retry-After=1）**。
- **capping `max_output_tokens=50` 沒用**（一樣 429）→ 限額算的是**輸入** token 數。

從 inference 回應 header 讀到 deployment 限額（inference key 讀不到 control-plane 設定，但 header 會吐）：

```
x-ratelimit-limit-tokens: 4000      # gpt-5.4 TPM
x-ratelimit-renewalperiod-tokens: 60
x-ratelimit-limit-requests: 40      # RPM
```

門檻精準吻合：~3.7K 過、~6K 掛、limit=4000。**codex 完整 agentic 請求**（系統提示 14.7KB + 自帶 developer/skills 訊息 ~10KB + tools + environment_context）輸入約 6–11K tokens，**一發就超過 4000 的 bucket 上限、永遠塞不進去** → 串流 `response.failed` / 非串流 429。

## 為什麼 claude 能動（同資源的反例）

這是最反直覺、也最關鍵的一段。

- codebus RunLog 顯示 **claude 首發 prompt ≈ 13,647 tokens**（input 3 + cache_write 13,644，opus chat）——**比 codex 還大**。
- 但 claude deployment `claude-opus-4-6-2026V2` 的 header 顯示 **TPM 只有 2000、RPM 2**（比 gpt-5.4 還低）。
- claude 用**更低的 TPM、送更多 token 卻成功**。

決定性測試（排除「是 caching 在幫忙」的可能）：送一個**全新、零快取**的請求到 claude `/anthropic`（TPM=2000）：

```
input_tokens = 23006, cache_creation = 0, cache_read = 0  →  HTTP 200 成功
```

一個 23,006 fresh tokens 的請求在 2000 的 bucket 下照過。→ **坐實：雖然是同一個 Azure 資源，兩個 API 介面層的限額執行語意真的不同：**

| 端點 | deployment TPM | 測試請求 | 結果 |
|---|---|---|---|
| `/anthropic/v1/messages`（claude）| 2000 | 23,006 fresh tokens | ✅ 200 |
| `/openai/responses`（codex）| 4000 | ~6K tokens | ❌ 429 |

- **`/openai/responses`（codex）**：硬性把「單一請求輸入 vs 每分鐘 token bucket」當門檻 —— 輸入 > bucket 就拒。
- **`/anthropic/v1/messages`（claude）**：**不**按輸入大小擋。

所以 claude 能動不是因為 TPM 比較高（其實更低）、也不是 caching，而是 **API 介面層的限額執行方式不同**。

## 修法

到 **Azure AI Foundry → `gpt-5.4` deployment → Edit → Tokens per Minute Rate Limit (TPM)**，調高到能容納 codex 單一請求的輸入量（6–11K，含多輪累加，建議 **≥30K**）。bucket 一旦 ≥ codex 單發輸入，`/openai/responses` 端點就不會再拒。

> 註：`claude-opus-4-6-2026V2` 的 TPM 也只有 2000，但因 Messages 端點不按輸入大小擋、目前能動；若日後 claude 改走別的端點或被收緊，同樣要看這個。

## 診斷配方（工具與雷，供下次重用）

- **看 codex 真錯誤**：`CODEBUS_FORWARD_AGENT_STDERR=1` 轉出 stderr。注意 stderr **不是完全黑洞**——`agent-run-integrity`（`913af73`）的 classifier 會掃每行、把一組精選的 sandbox/permission denial marker（`access is denied` / `permission denied` 等 5 個）計入 RunLog 的 `sandbox_denial_count`（與 forward 旗標無關，見 `stream/sandbox_signal.rs`、`claude_cli.rs:192-223`）。**但它只認那組 denial marker、且只計數不留原文**：本 bug 的 `response.failed` / `429` / `models_manager missing field 'models'` 一個都不匹配 → 計數 0、幫不上忙，原始錯誤文字仍被丟進 `io::sink()`、要 `CODEBUS_FORWARD_AGENT_STDERR=1` 才看得到（provider 端 HTTP/串流錯誤的觀測性目前是缺口）。而且**真正的致命錯誤其實在 stdout 的 `turn.failed`**，stderr 那段 `models_manager` 只是雜訊。
- **拿 deployment 限額數字**：直接打一發 inference，讀回應 header `x-ratelimit-limit-tokens` / `-limit-requests` / `-renewalperiod-tokens`（data-plane key 就讀得到；control-plane 的 TPM 設定本身要 Azure portal / ARM）。
- **拿 prompt 大小**：codebus RunLog 的 `tokens.input_tokens`（+ `cache_write`/`cache_read`）就是實際送出的 prompt 量，不必攔截。
- **抓 codex 真實送出的 request body**：架本機 http proxy 轉發（codex 的 `model_providers.azure.base_url` 吃 `http://127.0.0.1:<port>`）。**不要**用「log → 檔 → 重放」的方式，會把多位元組字元變 `?` 造成假的 400 parse error。
- **claude 攔截**：claude CLI **不吃 http 的 `ANTHROPIC_BASE_URL`**（要 https，攔截得 MITM 憑證），所以改用 RunLog 拿 prompt 量、用直接打 Messages API 讀 header 拿 TPM。
- **其它雷**：① 直接跑 `codex exec` 會 `Reading additional input from stdin...` 卡住 → 必須把 stdin 導向空檔（codebus 給的是 EOF）。② PowerShell `Start-Process -ArgumentList` 會把含空格的 prompt 拆成多 argv（`unexpected argument 'with'`）→ 用單字 prompt 或別走 Start-Process。③ keyring key 在 target `default.codebus-codex-azure` / `default.codebus-claude-azure`，**UTF-16LE**，CredRead x64 結構 offset：BlobSize@32、Blob@40。④ 此 deployment 額度低，反覆測試很快會把每分鐘額度打到 429。

## 附錄：關鍵數據

| 請求 | 輸入規模 | 串流 | 非串流 |
|---|---|---|---|
| codex 原版 instructions（minimal）| ~15KB / ~3.7K tok | ✅ completed | ✅ completed |
| 無害內容 + 小 input | ~15KB | — | ✅ completed |
| 無害內容 + 大 input | ~24KB / ~6K tok | ❌ response.failed | ❌ 429 |
| codex 完整請求 | raw 44852 bytes / ~6–11K tok | ❌ response.failed | (429) |
| capped `max_output_tokens=50` + ~6K | ~24KB | — | ❌ 429（無效）|
| claude 全新零快取 | 23,006 tok | — | ✅ 200 |

codex 完整 request 的頂層欄位：`model / instructions(14732 字) / input(developer 訊息含 permissions/apps/skills ~10KB + AGENTS.md + environment_context) / tools(多個) / tool_choice / parallel_tool_calls / reasoning / store / stream / include:[reasoning.encrypted_content] / prompt_cache_key / text:{verbosity} / client_metadata`。
