## 1. Add failing tests for new contract

對應 spec MODIFIED requirement **Lint scans body wikilinks in nav files and goal guides** 新加 5 個 scenario：(a) inline code wikilink 不報、(b) fenced code wikilink 不報、(c) table escape `\|` slug 解析正確、(d) table escape 仍抓真壞 slug、(e) 既存 `[[slug]]` regression。tests 應 fail（regex 還沒改）。本 §1 + §2 全段共同落地 spec requirement「Lint scans body wikilinks in nav files and goal guides」的修訂內容（markdown-aware code-region exclusion + slug class 排除反斜線 + alias 接受 `\|`）。

- [x] 1.1 更新 `tests/core/wiki/lint.test.ts`：
  - 新增 test「does not flag wikilink inside inline code」：page body `透過 \`[[wikilink]]\` 互相串接`，slug `wikilink` 不在 catalog → 期待 0 warning
  - 新增 test「does not flag wikilink inside fenced code block」：page body 含三引號圍住的 code block 內含 `[[ghost]]`，slug 不在 catalog → 期待 0 warning
  - 新增 test「resolves table-cell wikilink with escaped alias separator」：兩 page 互相 link、其中一邊用表格 cell `[[other-page\|顯示名]]` → 期待 0 warning（slug 解析為 `other-page` 命中 catalog）
  - 新增 test「table-cell wikilink with escaped separator still flags broken slug」：body `[[ghost\|alias]]` slug 不在 catalog → 期待 1 warn，message 含 `[[ghost]]`（slug 不帶反斜線）
  - 新增 test「regression: plain [[slug]] still resolves and is not affected by markdown-aware changes」：用既有 cross-folder link 場景再驗一次
  - tests 此時應 fail
  - 對應 spec MODIFIED requirement：「Lint scans body wikilinks in nav files and goal guides」

## 2. Implement markdown-aware lint scan

`src/core/wiki/lint.ts` 兩處精準動：

- [x] 2.1 Lint scans body wikilinks in nav files and goal guides — 更新 `BODY_WIKILINK_REGEX`：slug class 從 `[^\]|#\s]+` 改 `[^\]|#\s\\]+`（排除反斜線）；alias 段從 `(?:\|[^\]]+)?` 改 `(?:\\?\|[^\]]+)?`（接受 `\|` 或 `|` 作分隔符）。整體改為 `/\[\[([^\]|#\s\\]+)(?:#[^\]|]+)?(?:\\?\|[^\]]+)?\]\]/g`
- [x] 2.2 在 `lint.ts` 內新增私有 helper `stripCodeRegions(content: string): string`，做兩個 replace：
  - fenced：`/```[\s\S]*?```/g` → `''`
  - inline：`/`[^`\n]+`/g` → `''`
  順序：先 fenced 後 inline（避免 fenced 內的單 backtick 被 inline regex 抓壞）
- [x] 2.3 修改 `scanBodyWikilinks(content, relPath, pageSlugs, issues)`：第一行先 `const stripped = stripCodeRegions(content)`，然後對 `stripped` 跑 `matchAll`，其餘邏輯不動。**注意：只 scanBodyWikilinks 走 stripped，§3 page-walk 內的 `parsed.body` 也是經 `scanBodyWikilinks` 入口走，正確套用；§5 nav files 的 `scanBodyWikilinks(content, sf, ...)` 同樣走 stripped**。Frontmatter `related[]` validation 不經 stripCodeRegions（YAML 內的 wikilink 不是 markdown 上下文）

## 3. Verify

- [x] 3.1 跑 `npm test -- tests/core/wiki/lint.test.ts` 確認新 5 個 test 全 pass，既存 24 個 test 也全 pass（總 29）
- [x] 3.2 跑 `npm run build` 確認 tsc 過
- [x] 3.3 對 spike vault 跑 `node dist/cli.js --check --repo D:/side_project/uv` 確認：
  - `wiki/modules/uv-resolver.md` 的 5 個 `[[resolver-*\]]` warn 全消失
  - 整體 warning 數從目前 10 變 5（剩下的 5 是 wiki/overview.md 的 wiki/ root 警告 + index.md 的 4 個 broken-wikilink 指 retired goal-guide slug，那些是 wiki-taxonomy-realign 預期的 forward-compat signal，不該被本 change 動）
- [x] 3.4 跑 `spectra validate lint-markdown-aware-scan` 確認 valid
- [x] 3.5 跑 `spectra analyze lint-markdown-aware-scan --json` 確認 0 critical+warning
- [ ] 3.6 commit：訊息標明 (a) 兩個 false-positive bug 的 root cause、(b) 對 spike vault 的具體 warning 數變化（10 → 5）、(c) Non-Goals 中提到的「不引入 markdown AST 套件」trade-off
