# Spec / Plan Review Lessons

Cross-phase persistent notes from review iterations. Should outlive any
single phase plan/spec archival. Contributors and future reviewers
should read this before starting a new spec → plan → implementation
cycle.

## Lessons from CodeBus v2 phase 1 review (2026-05-04)

1. **Spike summaries must quote transcript lines, not just paraphrase.**
   Iter-3 review caught spike #1 summary saying "permission_denials=[]"
   without showing the `tool_use(Read)` / `tool_result` events that made
   the conclusion meaningful. Future spike commits must include the
   relevant transcript excerpts inline.

2. **Don't conflate `-p` mode with permission mode.** Spike B
   originally concluded "default mode + Write = baseline-deny";
   actually it's "-p mode (no interactive user) + default permission
   mode = no one to approve permission requests". Naming the layers
   precisely matters when designing around them.

3. **`--add-dir` is widen, not narrow.** This caused two iterations of
   wrong sandbox claims. Always re-read CLI flag docs (or spike) before
   asserting "X limits scope to Y".

4. **Severity column in risk tables.** Iter-3 review noted §3.2.1 had
   goals.jsonl (vault-killer) listed parallel to raw/code/ pollution
   (1-page impact). Risk tables should mark blast radius explicitly so
   readers can prioritize.

5. **Phase 2 unblock items belong in §15 (Open Questions), not buried
   in prose.** When deferring something "phase 2 will handle", put it
   explicitly in the Open Questions list so future ingest pass doesn't
   lose it.

6. **Don't default-defer without measuring cost.** Iter-4 reviewer
   caught "cwd = .codebus/" was deferred to phase 2 without ever
   running the cost spike. Turned out cost is ~5 lines change + a 30-
   minute spike, and benefit is system-level user-repo isolation.
   Before defer/include calls, spike the cost when it's that cheap to
   measure.

7. **Convergence-rate as stopping signal.** Iter-4 reviewer's framing:
   round 1 finds structural defects (high ROI), round 2 correctness
   (high ROI), round 3 framing/precision (medium), round 4 edge cases
   (medium-low). When defect rate per round drops + nature shifts from
   "must fix" to "could improve", that's the signal to stop reviewing
   and start implementing. Real-world feedback from animation > more
   thinking on the same artifact.

8. **Spec convergence ≠ plan convergence.** Iter-1 through iter-7
   stayed in spec namespace and reviewer + AI both declared "stable,
   ready for execution" twice. Iter-8 dropped into plan code and found
   3 critical bugs nobody had caught: parser schema fictional (would
   produce empty terminal output), enrichSourceMetadata silently broke
   stale-detect (compared same-hash-vs-same-hash), SIGINT handler TDZ
   race. For multi-doc deliverables (spec + plan), the review cycle
   isn't done until BOTH have been audited at their level of detail.
   Add an explicit "now drop into plan code" iteration before any
   "ready for execution" claim. The plan code review iteration ROI
   was as high as round 5 (cwd spike) — finding bugs that would have
   shown up day 1 of execution.

9. **Wrap-up 階段 reviewer 容易 diff misread.** Iter-7 跟 iter-9 兩次
   都同類錯誤：盯著 unified diff 的 `-` 跟 `+` 行下結論，沒 verify
   final file state。Iter-7 claim §3.2.1 blockquote 整段被刪除（實際
   是 single-line replacement，blockquote 仍在）；iter-9 claim 5 個
   stale tests 仍存在於 plan（實際整個 describe block 已被替換）。
   Pattern: review 後期 reviewer 已熟悉文件，傾向看「what changed」
   而非 re-read「what it now says」。Wrap-up 階段（轉折信號：「沒新
   議題要追了」/ convergence verdict）reviewer 應 explicit re-read
   final file state，特別對結構性改動（整段 block replace、test
   rewrite、section move、re-order）。跟 lesson #1 同 family — 不要
   trust paraphrase / partial signal，要 ground 到 source-of-truth。

10. **Sandbox spike 必須測「不該跑的工具有沒有被擋」，不只測「需要的工具能不能跑」。**
    Iter-9（implementation 後 live test 才發現）：spec §3.2 早期 spike B 測
    「default mode + Write」確認 Write deny → 推出「acceptEdits 必設」結論
    → spec 寫「`--allowedTools` whitelist 是 sandbox」。但只測了 baseline
    layer（要的工具能不能跑），完全沒驗 acceptEdits 在 -p mode 下對
    「未在 allowedTools」的工具實際 behavior。Live test against buddy-gacha
    暴露 agent 用 Bash 跑 ls/wc 沒被擋。**根因**：`--allowedTools` 是
    auto-approval 列表不是 toolset 限制；`--tools` 才是 toolset gate。
    Sandbox spike 的成功標準必須對稱：(a) 要的工具 work，(b) 不要的工具
    擋住。少了 (b) sandbox 可能根本沒成立。寫 spec 時若只能 cite (a)
    的 spike 結果，就應該標明 sandbox 強度未驗證。

## Lessons from obsidian-clickable-wikilinks (2026-05-08)

11. **Obsidian URI `vault=<id>` 認 SHA-256 id 是 undocumented 行為。**
    Spike during obsidian-clickable-wikilinks 把 `obsidian://open?vault=<sha256-prefix>&file=<rel>`
    跟 `obsidian://open?path=<abs>` 兩種變體實機點，**兩種都跳對檔**。但官方
    Obsidian docs 的 `obsidian://` URI 只描述 `vault=<vault-name>`（即 path
    末段 / user 在介面看到的 display name）。實測認 id 可能是 Obsidian 內部
    fallback 順序「name → id」造成的，未公開。spec 已標為 Risk + 保留 path
    變體當逃生口。**lesson**：當實機行為比文件聲明更寬鬆時（特別是「我們
    依賴一個沒寫進文件的 case」），spec/design 必須記為 risk + 寫好 fallback
    路徑，不要讓未來 Obsidian 版本變更打死整個 feature。

12. **Race-overwrite 風險寬窄要看「user 是否操作 shared resource」，不是「process 是否在跑」。**
    Spike 觀察：Obsidian 在跑時 codebus 寫 obsidian.json 後，**user 不主動
    操作 vault list（切換、增刪 vault）的情況下 Obsidian 不會回寫 obsidian.json**——
    我們的 entry 安全。但只要 user 動 vault list，整份 in-memory 版本就會回
    寫覆蓋。**「skip-on-running」是廉價安全網**而非 race-proof 保證；spec
    寫成「偵測 + skip + hint」採務實取捨，不嘗試在 Obsidian 跑著時 race-safe
    寫入（複雜度高、收益低）。**lesson**：對「shared mutable state with
    external owner」的 race，先觀察「common case 是否 race」再決定 mitigation
    強度——多數時間 common case 安全的話，cheap detection + skip 比 file lock
    / 重啟 process 等強 mitigation 划算。

## How to add a lesson

When a review iteration surfaces a process insight (not a content
fix), add it here as a numbered item with:
- short imperative title
- 1-3 sentence explanation including which review iteration / commit
  caused the lesson
- (optional) example code snippet or counterexample

Don't put content fixes here — those go in the spec/plan they belong to.
