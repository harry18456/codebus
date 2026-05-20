# 既有 user 升級：claude_code.system.verify / claude_code.azure.verify（verify-stage-independent-model）

**Date:** 2026-05-21
**Related change:** `openspec/changes/archive/2026-05-21-verify-stage-independent-model/`（archive 後位置）
**Related spec:** `openspec/specs/claude-code-config/spec.md` — Requirement `Endpoint Profile Schema`（加 `verify` 為第四個必填 verb 子塊）

---

## 為什麼需要這個升級

`verify-stage-independent-model` change 讓 quiz 跟 goal 的 content verify spawn 使用獨立的 model 與 effort（透過新加的 `Verb::Verify` 解析），不再沿用各 verb 自己的 model。

實際使用情境：「便宜 model 出題 + 貴 model 審查」cost 控制：
- 既有：quiz 設 `haiku-4-5` → 出題用 haiku，verify 也用 haiku（reasoning 不夠強抓不到 hallucination）
- 升級後：quiz 設 `haiku-4-5` + verify 設 `opus-4-6` → 出題快、驗證有 reasoning

`~/.codebus/config.yaml` 的 `claude_code.system` 與 `claude_code.azure` 區塊都加上 **必填** 第四個子塊 `verify`。**既有 user 第一次升級執行 codebus 時，會看到 fail-loud parse error**：

```
error: claude_code config parse failed at ~/.codebus/config.yaml:
config file yaml parse: claude_code.system.verify: required when active=system
```

或 active=azure 對應：

```
config file yaml parse: claude_code.azure.verify: required when active=azure
```

這是設計選擇（fail-loud-on-config-parse-error philosophy + 避免 cost surprise），不是 bug。處理方式如下：

## 方案 A：手動加 yaml 區塊（推薦）

打開 `~/.codebus/config.yaml`，找到 `claude_code.system` 區塊，**在最後加一個 verify 子塊**：

```yaml
claude_code:
  active: system
  system:
    goal:
      model: opus-4-6
      effort: high
    query:
      model: haiku-4-5
      effort: low
    fix:
      model: sonnet-4-6
      effort: medium
    verify:                # ← 新增
      model: opus-4-6      # ← 「貴審」設計意圖：最強 reasoning
      effort: high         # ← 配合最高 effort
```

如果你**也有 `claude_code.azure` 區塊**（active=azure 或當 cold storage 用），azure 也要加 verify：

```yaml
  azure:
    base_url: https://<your-resource>.cognitiveservices.azure.com/anthropic
    keyring_service: codebus-azure
    goal:
      model: <your-opus-deployment-name>
      effort: high
    query:
      model: <your-haiku-deployment-name>
      effort: low
    fix:
      model: <your-sonnet-deployment-name>
      effort: medium
    verify:                              # ← 新增
      model: <your-opus-deployment-name> # ← Azure 用 deployment name 字串
      effort: high
```

存檔。下次跑 `codebus query / goal / quiz` 等任何 verb，parse 通過。

### 預期 cost 行為

預設值 `opus-4-6 / high` 是「最強 reasoning model + 最高 effort」，對應「便宜出 + 貴審」的設計意圖。**對既有 user 的影響：**

- 如果你**沒有開啟** `quiz.content_verify: true` 或 `goal.content_verify: true`，verify spawn 不會跑，這條 yaml 區塊只是被 parse 認識但永遠不會觸發 → cost 無影響
- 如果你**有開啟** content verify，每次 quiz / goal verb 多跑一次 opus-4-6 spawn → cost 上升明顯
- 想用便宜 verify：把 verify 改成 `haiku-4-5 / low` 也合法（但失去 reasoning 強度，verify 等於沒做）

### 想用便宜 verify 的話：

```yaml
    verify:
      model: haiku-4-5  # 跟 query 一樣便宜
      effort: low
```

這樣等效於 change 前的行為，但**請理解：本 change 的設計重點是 verify 有 reasoning 把關**，把 verify 設成跟 query 一樣會讓 verify 階段失去意義。

## 方案 B：re-init（會丟失現有 config）

只在你**沒有**對 `~/.codebus/config.yaml` 做任何自訂時可用：

1. 刪掉 `~/.codebus/config.yaml`
2. 重新跑 `codebus init`，會寫一份新的 starter config 含完整 verify 區塊（含註解）

`codebus init` 用 write-if-missing 寫入新版（含 verify 預設）。**不推薦**這條路徑除非你確認沒有自訂任何 setting，因為 init 不會 diff merge，會用 default template 取代。

## 為什麼不自動 migrate

設計上保留 `write_starter_config_if_missing` 的 if-missing byte-identical 不覆寫契約：

- User 可能對 `~/.codebus/config.yaml` 加了自己的 azure cold-storage 設定或其他 codebus 不認識的區塊
- 自動 diff merge yaml 物件非平凡工程，且任何 merge 行為都有「破壞 user 客製化」的風險
- 走 release note 引導 user 手動加，user 對 config.yaml 的內容有完全掌控
- fail-loud parse error 訊息直接點出該加什麼 field，引導性比靜默自動加區塊更明確

未來如果出現 `codebus config migrate` 子命令會走獨立 change，本 change 範圍不包含。

## 升級需要做的事，盤點一次

- [ ] 在 `~/.codebus/config.yaml` 的 `claude_code.system` 區塊加 `verify` 子塊
- [ ] 如果有 `claude_code.azure` 區塊（不論 active 是 system 或 azure），同樣加 `verify`
- [ ] 跑 `codebus query "ping"` 確認 parse 通過
- [ ] 視 cost 敏感度決定 verify model：要 reasoning 強就走預設 `opus-4-6 / high`；要省錢就改 `haiku-4-5 / low`（但失去設計意圖）
- [ ] 開過 `quiz.content_verify` / `goal.content_verify` 的 user，下次跑 quiz / goal 觀察 events.jsonl 確認 verify spawn 確實用 opus（model 欄記主 spawn 不記 verify，但 `events.jsonl` 的 SpawnStart 事件會帶實際 model）
