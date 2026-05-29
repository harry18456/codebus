# TODO · Claude Code 4.8 + ultracode 對 codebus 開發流程的影響評估

**Date:** 2026-05-28
**Severity:** workflow / tooling observability（不阻塞功能）
**Status:** open（待研究）

## 動機

2026-05-28 期間 Claude Code 升級：

- **Claude Opus 4.8**（current model）—— 取代先前 4.x
- **effort 機制推出 ultracode**（最高 effort tier）

想看這兩個變化對 codebus 既有開發流程（spectra propose / apply / archive + 長 prompt + memory + CDP smoke）有何影響、是否該調整 workflow。

## 想看什麼

| 維度 | 問題 |
|---|---|
| **ultracode 是什麼** | effort tier 的最高檔？跟既有 effort（low / medium / high）差異？token / latency / 品質 trade-off？ |
| **對長 prompt pattern 影響** | 本 session 大量用 200+ 行 propose prompt；4.8 / ultracode 是否讓 prompt 可以更短（model 自己補 context）？還是無關？|
| **對 spectra apply 影響** | apply agent 跑 TDD + CDP smoke + scope check；ultracode 是否讓單 session 能塞更多工作量（少 fresh-session 接力，如 Phase 5.4 那種）？|
| **對 grep 校準必要性影響** | 本 session 反覆踩「propose 假設 vs 實機脫節」（5→12 / topbar / i18n JSON / wiki 5 衝突）；更強 model 是否減少這類失誤、還是一樣要 grep 校準？|
| **對 memory pattern 影響** | 累積 30+ 條 memory；4.8 是否更會主動套用 memory、還是仍需 prompt 明確提醒？|
| **跟 claude-trace 分析的關係** | `docs/2026-05-28-claude-trace-prompt-analysis-todo.md` 想量 token；4.8 / ultracode 的 token 行為可能跟前代不同、兩個 todo 可合併做 |

## 與既有 todo 的關係

- 跟 `claude-trace-prompt-analysis-todo.md` 高度重疊（都關心 prompt 用量 / model 行為）
- 建議：**兩個合併做**——一次 trace 分析同時涵蓋「4.8 / ultracode 的 token 行為」+「long prompt 用量」

## 建議起手

1. 查 Claude Code 4.8 + ultracode 官方 release notes / docs（用 claude-code-guide agent 或 WebSearch）
2. 釐清 ultracode 的 effort 定位 + token / 品質 trade-off
3. 對照本 session 的 workflow pain points（長 prompt / fresh-session 接力 / grep 校準）、評估 4.8 是否減輕哪些
4. 若 ultracode 適合 codebus 的 propose / apply 階段、評估何時開、成本如何
5. 結論寫 finding doc

## Priority

低-中（observability + workflow optimization、不阻塞功能）。

跟 claude-trace 分析同 batch、有空檔再跑。

## 不在 scope

- 改 codebus 程式（純 tooling / workflow 評估）
- 強制換 effort tier（先看 data）

## 注意

本 todo 的「想看什麼」是 2026-05-28 當下的好奇、實際 4.8 / ultracode 行為以官方 docs + 實測為準、不要憑印象寫結論。
