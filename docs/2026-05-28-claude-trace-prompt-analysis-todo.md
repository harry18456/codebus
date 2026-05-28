# TODO · Claude trace 分析初始 prompt 用量

## 動機

本 session（2026-05-25 到 2026-05-28）大量用「我寫長 prompt → user 貼進 /spectra-propose → 另一邊 spectra-apply agent 跑」的 pattern。長 prompt 包含：

- Pre-apply checklist · ground truth grep（10+ 條 anchor）
- Scope 區塊（4-6 個拆解）
- 驗收條件
- Memory 提醒（5-10 條 cross-reference）
- 不要的事

單個 prompt ~150-300 行。一 session 寫 13+ change × 平均 200 行 = 2600+ 行 prompt body。

## 想看什麼

| 維度 | 問題 |
|---|---|
| Token 消耗 | 這種長 prompt 在 Claude API 端實際吃多少 token？input vs output 比例？ |
| Prompt cache | Memory + AUDIT + spec 段落多次出現、cache hit rate 有沒有節省？ |
| Context window | 寫 prompt 時佔用 main session 多少 context？下次 fresh session 跑 apply 時又吃多少？ |
| Limitation | prompt 寫太長有沒有效益遞減？哪些 section 真的有用、哪些是 noise？ |
| 跨 session | propose / apply / archive 三段 session 各自的 token 用量是否合理？ |

## 工具候選

- **Claude trace**（Anthropic 官方 tracing/observability，若有 SDK 整合）
- **claude-code-cli** 內建 trace flag（如果有）
- **token counting API**（提前算 input token 數）
- 手動 inspect Claude Code session log（`~/.claude/projects/.../<session>.jsonl`）

## 建議實作步驟（時間到再做）

1. 找 Claude trace 工具的入口 / docs
2. 對最近一個 propose（如 `vault-scoped-active-runs` 那個）做 trace 抓 baseline
3. 對 apply session 同樣 trace、看 input token 主要來自哪
4. 嘗試「精簡 prompt」變體（砍 memory 提醒區、砍重複段）→ 比 token / 結果差異
5. 結論寫 `docs/2026-05-XX-prompt-usage-finding.md`

## 不在 scope

- 改 codebus 的 prompt template（先看 data 再決定）
- 改 Claude Code 設定（observability 收集為主）

## Priority

**中-高**（修正 2026-05-28）。

原本標低是 lazy framing「不阻塞功能 = polish-ship」。實際上：

- 本 session 已用此 long-prompt pattern 跑 13+ change、每個 change 200+ 行 prompt × 2-3 session = 累積成本已不小
- Pattern 已內化、每個 future change 繼續用、若有 30% noise 是複利浪費
- Solo dev 自己付 token 帳、團隊 budget absorb 不適用
- Context pressure 已實證（Phase 5.4 Section 8 需 fresh session）—— 精簡 prompt 可能多塞工作量
- **不先測量、後續 bug 3/4/2/1 還會繼續用同 template 燒 token**

## 建議插入時機

彈性、不強排序。

理想：bug 3 archive 後做、給 bug 4/2/1 用「優化版 prompt template」、效益 propagate。

實際：user 決定（2026-05-28 取消強插入）—— 可以等 4 bug 都收完再做、不阻塞、複利浪費繼續累積但 manageable。
