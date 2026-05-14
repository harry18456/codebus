# Backlog: OpenAI Privacy Filter 整合（local 語意層 PII）

**Date:** 2026-05-14
**Surfaced during:** backlog 討論（v3-app-chat-cmdk apply 期間）
**Severity:** PII 保護強化
**Owner:** harry
**Status:** parked

---

## 觀察

現有 `RegexBasicScanner` 只能抓 pattern-based PII（email、phone、固定格式 ID）。
語意層的 PII（人名、公司名、職稱、含 PII 的自然語言段落）完全 bypass。

例：

```
"這是 John 在 2024-03-15 發給 Acme Corp 的 message" 
→ RegexBasicScanner: 只 mask 日期，名字 / 公司名漏掉
```

## OpenAI Privacy Filter 技術細節

| 項目 | 規格 |
|------|------|
| 模型大小 | 1.5B 參數（MoE；active 50M） |
| 部署方式 | **純本地**，無雲端 API |
| 授權 | Apache 2.0 |
| Context | 128K tokens |
| PII 類別 | 8 類：name / email / phone / address / date / account number / URL / secret |
| 格式 | SafeTensors（F32 / BF16）、ONNX、Transformers.js |
| 推理速度 | 可在 laptop 上跑（transformer blocks 8 層，banded attention） |

完全符合 codebus local-first 設計——資料不出境。

## Proposed fix

新提一條 change：`v3-pii-semantic`

### 架構

```
raw_sync → RegexBasicScanner (fast, pattern) → SemanticPiiScanner (slow, semantic)
                                                     │
                                         openai/privacy-filter model
                                         (ONNX runtime via cxx-ort)
```

- **兩層互補**：regex 快速抓已知格式；semantic 補語意層漏網之魚
- Semantic scanner 可設定為 opt-in（預設 off，因為推理有 latency）
- raw_sync 後跑，不影響 agent stream pipeline（離線批次）

### 整合方案

**方案 A（推薦）**：ONNX runtime
- `codebus-core` 加 `ort` crate（ONNX Runtime Rust binding）
- model weights 隨 app bundle 或首次啟動自動下載到 `~/.codebus/models/`
- 跨平台（Windows / macOS / Linux）支援良好

**方案 B**：Python subprocess
- 呼叫 `opf` CLI（openai privacy-filter 的 CLI 工具）
- 不需 Rust binding，但需要 Python 環境
- 不推薦（增加用戶環境依賴）

### Config

```yaml
pii:
  semantic_scanner:
    enabled: false   # opt-in
    model_path: null  # null = auto-download to ~/.codebus/models/
    threshold: 0.85   # 信心分數門檻
```

### Tasks（粗估）

1. spec MODIFIED `pii-filter`：加 SemanticPiiScanner 規格
2. `ort` crate 整合（build dependency）
3. `codebus-core/src/pii/scanners/semantic.rs`：ONNX 推理 + post-process
4. model 首次下載 / cache 邏輯（`~/.codebus/models/privacy-filter/`）
5. Config schema 加 `pii.semantic_scanner.*`
6. Settings UI 加 toggle（Opt-in，含「first time 下載模型」進度提示）
7. Integration test：語意 PII round-trip（人名 + 公司名 mask 正確）

工程量：重（3-5 個半天；ONNX runtime 整合有未知風險）。

## Out of scope

- 不替換 RegexBasicScanner（兩層並存，互補）
- 不支援 fine-tuning 自訂模型（使用 openai/privacy-filter weights 原版）
- 不做 cloud API fallback

## 依賴

- `pii-settings-ui` backlog（extra regex rules）可以同批做，UI 共用 Settings 區塊
- 獨立於 `git-context-tool` 但邏輯互補（git context 過 semantic scanner 效果更好）

## 何時動

優先序低於 F。建議在 F archive 之後，確認 app 穩定後再引入重型 ONNX 依賴。
先做 spike：ONNX Runtime 在 Windows MSVC + macOS arm64 的 build 可行性。
