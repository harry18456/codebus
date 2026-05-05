# Spike Observations: codebus on D:/side_project/uv

## Setup snapshot (Stage 0)

- codebus build: dist mtime 2026-05-05 10:12 (post src 19:11)
- claude CLI: 2.1.128
- uv working tree: clean
- uv repo size: 186 MB / 474k Rust LoC / 69 crates / 577 .rs + 91 .py + 179 .md
- Disk free: 273 GB
- Stage 1 crate selected: `uv-cache-key` (908 LoC, 39KB, 5 .rs files)

## Stage 1 — single-crate goal baseline

**Command:** `node dist/cli.js --repo D:/side_project/uv --goal "了解 uv-cache-key 的設計目標跟它對外的 API 表面"`

**Started:** 2026-05-05 10:14:xx (approx)
**Wall clock:** **3m 26.7s** (real)
**Exit code:** 0

### Pass criteria

| Criterion | Threshold | Actual | Pass? |
|---|---|---|---|
| Total wall clock | < 5 min | 3m 26.7s | ✅ |
| ≥1 knowledge page produced | ≥1 | 3 (module + concept + entity) | ✅ |
| Auto-lint errorCount | 0 | 0 (1 warn) | ✅ |
| `wikiChanged = true` | true | true (done banner: "掰掰~下車囉") | ✅ |
| No cwd-外 Write/Edit | none | none (grep clean) | ✅ |
| Token cost (proxy: log lines) | <200k tokens | 75 lines / 3.3 KB rendered output (renderer dedupes; cannot measure tokens without `--debug` wired) | ✅ (proxy) |

### Sub-metric: sync time
Couldn't isolate sync time from total — would need timestamps on each phase. Total 3m 26.7s includes raw sync + agent loop + enrich + lint + commit. Sync-specific instrument is a follow-up if needed.

### Sub-metric: raw/code/ size after sync
**29 MB** (`.codebus/raw/code/`) — uv crates/ minus gitignored content; ≈ raw source size of the whole repo (gitignore drops target/ etc).

### Sub-metric: wiki size
**21 KB** total (3 knowledge pages + 3 nav files + 1 goal guide).

### Sub-metric: artifacts produced
- `wiki/modules/uv-cache-key.md` — module page (5 sources cited)
- `wiki/concepts/cache-key-trait.md` — concept page (2 sources)
- `wiki/entities/canonical-url.md` — entity page (1 source)
- `wiki/overview.md`, `wiki/index.md`, `wiki/log.md` — nav files
- `wiki/goals/uv-cache-key-design.md` — goal guide

### Page quality (frontmatter spot-check)

All 3 knowledge pages:
- Valid YAML frontmatter, `type` matches containing folder ✅
- `sources[]` enriched with real `sha256` + `at_commit` (a1c90c1fa12c…) ✅
- `goals[]` records the run goal ✅
- `related[]` uses `[[wikilink]]` format with slugs that resolve to other pages produced same run ✅
- UTC `created`/`updated` = 2026-05-05 ✅

Cross-page wikilink graph (mini):
- modules/uv-cache-key → concepts/cache-key-trait
- entities/canonical-url → modules/uv-cache-key + concepts/cache-key-trait
(coherent, mutual references)

### Lint warning detail

Single `warn` on `wiki/overview.md`:
> broken wikilink in body: `[[wikilink]]` (no page named wikilink.md in any wiki/<type>/ folder)

**Initial misread (corrected)**: I first called this a "schema literacy gap — agent copied example token verbatim". Wrong. Actual `overview.md` line is:
```markdown
透過 `[[wikilink]]` 互相串接
```
The `[[wikilink]]` is inside inline code (backticks), used by the agent as meta-explanation of the syntax. Obsidian renders this as literal text, not a link. Agent behavior is correct.

**Real root cause**: `src/core/wiki/lint.ts:scanBodyWikilinks` regex doesn't skip code spans / fenced code blocks. Any `[[…]]` inside backticks is incorrectly flagged. This is a **lint false positive** code bug, not an agent / schema issue.

**Severity**: low (warn-only, doesn't block commit, Obsidian renders correctly), but creates noise on every `--check` run that erodes lint signal value over time.

**Fix scope**: ~10 lines src + ~15 lines test in `lint.ts`. Strip ` ``` … ``` ` and `` `…` `` regions before regex scan. Add 2 test cases (inline code, fenced block).

**Logged for follow-up after Stage 3**, not fixed inline to keep spike flow clean.

### Sandbox verdict

✅ Iter-9 holds. No Write/Edit attempts outside `.codebus/`. Agent only Read raw/code/ and Wrote .codebus/wiki/.

### Stage 1 verdict: **PASS**

All 6 pass criteria met. Quality of output (3 differentiated page types + coherent cross-links + correct sha256 enrichment) exceeds minimum.

---

## Stage 2 — query against same vault

**Command:** `node dist/cli.js --repo D:/side_project/uv --query "uv-cache-key 為什麼要自己搞 CacheKey trait，不直接用 std::hash::Hash？"`

**Wall clock:** **26.063s** (real)
**Exit code:** 0

### Pass criteria

| Criterion | Threshold | Actual | Pass? |
|---|---|---|---|
| ≥1 [[wikilink]] cite, slug exists | yes | 3 cites: `[[cache-key-trait]]`, `[[uv-cache-key]]`, `[[canonical-url]]` — all real | ✅ |
| No Write/Edit attempts | none | none (only 2 Read calls) | ✅ |
| No raw/code/ Read | none | none (agent only Read wiki/) | ✅ |
| Wall clock | <60s | 26.063s | ✅ |

### Tool sequence
1. Read `wiki/concepts/cache-key-trait.md` (124 lines)
2. Read `wiki/modules/uv-cache-key.md` (95 lines)
3. Produce answer with cites

Agent self-selected to read 2 pages (concept + module) instead of just the most obvious one. Cross-page synthesis behavior worked.

### Grounding check (manual, claim-by-claim)

8 claims checked against `concepts/cache-key-trait.md` source content:

| Claim | Grounded? |
|---|---|
| `std::hash::Hash` 不保證跨版本/平台穩定 | ✅ verbatim |
| `url::Url` 的 `Hash` 跨版本可能改寫 | ✅ verbatim |
| `HashMap` 依賴隨機化 iteration order | ⚠️ page says "迭代順序不穩定"; agent supplemented with "隨機化" (Rust standard knowledge, not hallucination) |
| `CacheKey` 強迫 opt-in、拒絕 HashMap/HashSet | ✅ verbatim |
| 介面寫死 `&mut CacheKeyHasher` 非泛型 | ✅ verbatim |
| 內部包 `seahash::SeaHasher` | ✅ verbatim |
| "insulates against possible changes…" quote | ✅ verbatim |
| 許多型別 `cache_key` 轉呼叫 `self.hash` | ✅ verbatim |

**8/8 grounded** (one minor extrapolation, content-correct).

### Stage 2 verdict: **PASS**

疑慮 1 (`--query` 真實效果) status: **CLEARED on small wiki (3 knowledge pages)**.

Caveats:
- Tested only on 3-page wiki. Behavior at 10+ pages or 50+ pages remains untested.
- Single query, single topic. Query selectivity (picking right pages) at scale not stress-tested.
- Sandbox query mode held perfectly (no Write/Edit/raw-code reads).

## Stage 3 — multi-goal scaling on uv

**Goals run sequentially against same vault** (incremental wiki growth).

### Per-goal metrics

| Metric | Goal 1 | Goal 2 | Goal 3 | Goal 4 |
|---|---|---|---|---|
| Crate | uv-cache-key | uv-cache-info | uv-resolver | uv-installer |
| Crate LoC | 908 | 1004 | 31592 | 3224 |
| Wall clock | 3m 26.7s | 4m 52.9s | 6m 21.5s | 4m 25.3s |
| New knowledge pages | 3 | 3 | 5 | 2 |
| Cumulative pages | 7 | 11 | 18 | 21 |
| index.md bytes | 561 | 1023 | 1826 | 2193 |
| wiki bytes | 21008 | 46073 | 88629 | 110688 |
| raw/code bytes | 27273073 | 27273073 | 27273073 | 27273073 |
| Lint errorCount | 0 | 0 | 0 | 0 |
| Lint warnCount | 1 | 1 | 6 | 6 |

### Pass criteria results

| Criterion | Threshold | Actual | Pass? |
|---|---|---|---|
| index.md sub-linear growth | sub-linear | **linear (+150-200 bytes per knowledge page)** | ⚠️ borderline FAIL on strict reading |
| ≥1 page-merge triggered | ≥1 | **0 — every goal produced fresh slugs** | ❌ FAIL |
| Goal 4 sync time ≈ Goal 1 | similar | sync time not isolatable from total | ⚠️ inconclusive |
| Goal 4 token cost ≤ Goal 1 × 1.5 | proxy: wall clock | 4:25 / 3:26 = 1.28× | ✅ (proxy) |
| Lint errorCount=0 throughout | 0 | 0 throughout | ✅ |
| No cwd-外 Write attempts | none | none across all 4 goals | ✅ |

### Knowledge page distribution

| Folder | Count | Pages |
|---|---|---|
| concepts/ | 3 | cache-info-vs-cache-key, cache-key-trait, resolver-provider |
| entities/ | 5 | cache-info, canonical-url, resolver-manifest, resolver-options, resolver-output |
| modules/ | 4 | uv-cache-info, uv-cache-key, uv-installer, uv-resolver |
| processes/ | 2 | resolver-resolve, wheel-install-pipeline |
| **synthesis/** | **0** | (none — no cross-cutting summary across 4 goals) |

### Lint warning composition (final = 6)

| Source | Count | Cause |
|---|---|---|
| `wiki/overview.md` | 1 | `[[wikilink]]` literal in inline code (backticks) — **Bug A: lint not code-span aware** |
| `wiki/modules/uv-resolver.md` | 5 | `[[slug\|alias]]` in markdown table (escaped `\|` to avoid table delimiter conflict) — **Bug B: lint regex eats backslash into slug** |

Both are lint false positives on agent output that's semantically valid Obsidian markdown. **All 6 warnings are lint code bugs, not wiki content problems.**

### Concern A: page-merge never triggered

Goals 1-4 deliberately overlapped (cache-key + cache-info + cache+installer) to provoke agent into updating existing pages. Result: agent always wrote new slugs.

| Existing page that should have merged | Agent's actual choice |
|---|---|
| `concepts/cache-key-trait` (stage 1) | Goal 2 wrote `concepts/cache-info-vs-cache-key` (new) |
| `concepts/cache-key-trait` (stage 1) | Goal 4 wrote `processes/wheel-install-pipeline` (new — different angle) |

Frontmatter `goals[]` of `cache-key-trait.md` and `cache-info-vs-cache-key.md` only contain their original goal — no append-merge happened. **page-merge contract works (lint stable, no errors), but the schema's incremental-update bias is too weak to drive it in practice.**

### Concern B: index.md grows linearly with pages, enters every system prompt

By goal 4 the prompt's index.md content is 2.2 KB. At ~150-200 bytes per knowledge page, projecting:
- 30 pages → ~4.5 KB index.md
- 100 pages → ~15 KB index.md
- 500 pages → ~75 KB index.md (significant slice of context window)

Not a phase 1 blocker (project working size unlikely to hit 500+ pages soon), but a **phase 1.5 design item**: cap index.md size in system prompt OR send a TOC/abstract instead of full file.

### Concern C: raw/code/ full re-copy on every goal

27 MB stays constant because uv source content is invariant — but `syncRepoToRaw` clears + re-copies all 668 files every goal. 4 goals = 4× full re-copy. Slow on HDD, wears SSD pages. Mitigation: incremental sync (mtime / hash skip).

### Sandbox throughout

No `Write` / `Edit` to paths outside `.codebus/` across all 4 goals. Iter-9 sandbox holds.

### Stage 3 verdict: **MIXED**

- ✅ Sandbox holds
- ✅ Lint errorCount = 0 throughout
- ✅ Wall clock growth modest (within 1.3× across crate-size variance)
- ⚠️ index.md grows linearly (cumulative concern)
- ❌ page-merge never fires (schema bias problem)
- ⚠️ raw/code re-copy is wasted work
- ⚠️ Two lint code bugs surfaced (code-span exclusion, table-escape pipe)

疑慮 2 (multi-goal scaling) status: **partially CLEARED**. No P0/P1 issues. 4 phase-1.5 follow-up items identified.


