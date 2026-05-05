## Problem

`lintWiki` 對 wiki page body 跟 nav files 做 `[[wikilink]]` 掃描時，**對合規 Obsidian markdown 誤報為「broken wikilink」**。uv repo spike（4 goal / 14 page）暴露 6 個 warning，其中 6/6 都是這類 false positive：

1. **Inline code 內的 `[[...]]` 被誤報**
   ```markdown
   每個 wiki 頁面是一個獨立主題，透過 `[[wikilink]]` 互相串接。
   ```
   反引號內的 `[[wikilink]]` 是 syntax 範例，Obsidian 不解析為連結。但 lint 仍掃描並 emit `broken wikilink in body: [[wikilink]]`。spike 中 1 條 warn 屬此類。

2. **Markdown table 內 escaped pipe 把 `\` 吃進 slug**
   ```markdown
   | Entry point | [[resolver-resolve\|`Resolver`]] (含 ...) |
   ```
   `\|` 是 markdown table escape — 防 `|` 撞 column 分隔符。Obsidian 處理 table 後再解析 wikilink，最終 link 為 `resolver-resolve`，alias 為 `Resolver`。lint 的 slug class `[^\]|#\s]+` 不排除反斜線，把 `\` 收進 slug → slug 變 `resolver-resolve\` → 找不到 `resolver-resolve\.md` → 誤報 `broken wikilink in body: [[resolver-resolve\]]`。spike 中 5 條 warn 屬此類，全來自單一 page 的 5 列表格。

兩類都是 lint 邏輯誤判 valid Obsidian markdown，非 wiki 內容真的有問題。

## Root Cause

`src/core/wiki/lint.ts` 的兩個獨立 bug：

- **Bug A — scanBodyWikilinks 不剝離 code region**
  Helper 在 line 54-73 直接對整份 markdown body 跑 `BODY_WIKILINK_REGEX.matchAll()`，沒先處理 inline code（backticks）跟 fenced code block。Obsidian render 時這兩種 region 內的 `[[…]]` 都是 literal text，但 lint regex 一視同仁。

- **Bug B — slug class 缺 backslash 排除**
  `BODY_WIKILINK_REGEX = /\[\[([^\]|#\s]+)(?:#[^\]|]+)?(?:\|[^\]]+)?\]\]/g` 中 slug capture group `[^\]|#\s]+` 排除 `]` `|` `#` whitespace，**未排除 `\`**。當 markdown table 用 `\|` escape pipe，`\` 落入 slug capture，slug 失真。

兩 bug 同 root：lint regex 不是 markdown-aware，把 raw bytes 當 wikilink 候選。

## Proposed Solution

`src/core/wiki/lint.ts` 兩處精準修：

1. **scanBodyWikilinks 開掃前先 strip code regions**：新增 helper `stripCodeRegions(content)`，做兩個 replace（fenced ```` ```...``` ````、inline `` `...` ``）後把結果餵給 regex matchAll。**只在 scanBodyWikilinks 內使用** — frontmatter `related[]` 不走 markdown，繼續用原 regex 直接比對。

2. **slug class 加反斜線排除 + alias 接受 `\|`**：
   - slug class `[^\]|#\s]+` 改 `[^\]|#\s\\]+`（slug 不可含 `\`）
   - alias 部分 `(?:\|[^\]]+)?` 改 `(?:\\?\|[^\]]+)?`（alias 分隔符接受 `\|` 或 `|`）
   - 整體 regex：`/\[\[([^\]|#\s\\]+)(?:#[^\]|]+)?(?:\\?\|[^\]]+)?\]\]/g`

新增 4 個 test case 鎖契約，1 個既存 regression check 確保不傷正常 `[[slug]]`。

## Non-Goals

- **不改 error-severity 規則**：`related[]` 的 not-in-`[[...]]`-format 跟 broken-slug 仍是 error；body broken-wikilink 仍是 warn。本 change 只縮小「誤判」範圍，不調整 severity contract
- **不支援其他 markdown 語法 escape**：unicode escape、HTML entity、`<code>` tag、4-space-indent code block 等其他「該被忽略 wikilink」的場景不在本 change 範圍。Spike 只暴露 inline-code + table-escape 兩類；其他場景未實證需要時再開 change
- **不重新實作 markdown parser**：用 minimal regex strip 處理 code region 即可，不引入 markdown AST 套件（commonmark 等）。Trade-off：少數極端 markdown（譬如 inline code 跨行、巢狀 backtick fence）lint 仍會誤判，但 phase 1 不為這類 corner case 加 dependency
- **不動 frontmatter `related[]` 的 wikilink validation**：那段走 `RELATED_STRIP_REGEX` 不是 body scan path
- **不改 wikilink catalog 構成**：catalog source（5 type folder + index/log）跟 wiki-taxonomy-realign 後相同，本 change 不動
- **不加 inline lint suppression 機制**：不引入 `<!-- lint-disable-next-line -->` 之類 directive。修 regex 已能解 spike 觀察到的全部 false positive

## Success Criteria

- spike vault `D:/side_project/uv/.codebus/` 跑 `node dist/cli.js --check` 後：
  - `wiki/overview.md` 的 1 個 `[[wikilink]]` warn **消失**（已被 wiki-taxonomy-realign 改成 root-non-special warn，本 change 不再額外觸發 broken-wikilink）
  - `wiki/modules/uv-resolver.md` 的 5 個 `[[resolver-*\]]` warn **全消失**
  - 整體 warning 數從目前 10 變至少少 5 條（純此 change 直接砍的 5 條 table-escape；overview 的部分由 wiki-taxonomy-realign 已處理）
- 新增 4 個 test case 全 pass：
  - inline code 內 `\`[[foo]]\`` 對未被 catalog 收錄的 slug `foo`，**不**報 broken-wikilink
  - fenced code block 內 `\`\`\`...[[foo]]...\`\`\`` 同上
  - table cell `[[slug\|alias]]` 解析為 slug `slug` + alias `alias`，slug 在 catalog 內就**不**報 broken
  - table cell `[[ghost\|alias]]` slug 不在 catalog 內**仍**正常報 broken（regression：誤判修了但真壞 link 仍要抓）
- 既存 24 個 lint test 全 pass（不破壞 wiki-taxonomy-realign 後的 lint 行為契約）

## Impact

- Affected specs:
  - Modified: `openspec/specs/wiki-lint/spec.md`
- Affected code:
  - Modified: `src/core/wiki/lint.ts` (add `stripCodeRegions` helper; update `BODY_WIKILINK_REGEX` slug + alias classes; route scanBodyWikilinks through stripped content)
  - Modified: `tests/core/wiki/lint.test.ts` (+5 tests: 4 new contract + 1 regression on table-escape with broken slug)
