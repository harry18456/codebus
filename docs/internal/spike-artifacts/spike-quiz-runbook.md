# v3-app-quiz spike runbook

Self-contained. Copy-paste shell commands. Fixture vault at `docs/spike-artifacts/quiz-fixture-vault/`.

Pre-flight:

```bash
cd D:/side_project/codebus/docs/spike-artifacts/quiz-fixture-vault
claude --version    # confirm 2.x
ls wiki/            # confirm 5 wiki pages + index.md + log.md
ls .claude/skills/codebus-quiz/   # confirm SKILL.md
```

Common flags used in every spawn:

```
--tools Read,Glob,Grep
--allowedTools Read,Glob,Grep
--permission-mode acceptEdits
--output-format stream-json
--verbose
--include-partial-messages
```

Artifacts land at `../spike-quiz-<N>-<scenario>.jsonl`（i.e. `docs/spike-artifacts/`）.

---

## ❼ Planning sensibility (4 scenarios)

```bash
# F1: clear topic
claude -p "/codebus-quiz plan: I want to understand JWT" \
  --tools Read,Glob,Grep --allowedTools Read,Glob,Grep --permission-mode acceptEdits \
  --output-format stream-json --verbose --include-partial-messages \
  | tee ../spike-quiz-7-F1.jsonl

# F2: workflow topic
claude -p "/codebus-quiz plan: How does login work" \
  --tools Read,Glob,Grep --allowedTools Read,Glob,Grep --permission-mode acceptEdits \
  --output-format stream-json --verbose --include-partial-messages \
  | tee ../spike-quiz-7-F2.jsonl

# F3: no-match topic (cookies not covered)
claude -p "/codebus-quiz plan: 什麼是 cookies" \
  --tools Read,Glob,Grep --allowedTools Read,Glob,Grep --permission-mode acceptEdits \
  --output-format stream-json --verbose --include-partial-messages \
  | tee ../spike-quiz-7-F3.jsonl

# F4: ambiguous short topic
claude -p "/codebus-quiz plan: auth" \
  --tools Read,Glob,Grep --allowedTools Read,Glob,Grep --permission-mode acceptEdits \
  --output-format stream-json --verbose --include-partial-messages \
  | tee ../spike-quiz-7-F4.jsonl
```

**Pass criteria**: F1/F2/F4 emit `[CODEBUS_QUIZ_SCOPE]` first line with 2-5 reasonable pages; F3 emits `[CODEBUS_QUIZ_NO_MATCH]`.

---

## ❽ Raw/ scope enforce (5 scenarios)

```bash
# E1: prompt actively points at raw
claude -p "/codebus-quiz plan: how does auth.py work" \
  --tools Read,Glob,Grep --allowedTools Read,Glob,Grep --permission-mode acceptEdits \
  --output-format stream-json --verbose --include-partial-messages \
  | tee ../spike-quiz-8-E1.jsonl

# E2: technical implementation request
claude -p "/codebus-quiz plan: I want technical implementation detail of JWT verify" \
  --tools Read,Glob,Grep --allowedTools Read,Glob,Grep --permission-mode acceptEdits \
  --output-format stream-json --verbose --include-partial-messages \
  | tee ../spike-quiz-8-E2.jsonl

# E3: source code request
claude -p "/codebus-quiz plan: show me the source code of the middleware" \
  --tools Read,Glob,Grep --allowedTools Read,Glob,Grep --permission-mode acceptEdits \
  --output-format stream-json --verbose --include-partial-messages \
  | tee ../spike-quiz-8-E3.jsonl

# E4: normal topic
claude -p "/codebus-quiz plan: explain JWT lifecycle" \
  --tools Read,Glob,Grep --allowedTools Read,Glob,Grep --permission-mode acceptEdits \
  --output-format stream-json --verbose --include-partial-messages \
  | tee ../spike-quiz-8-E4.jsonl

# E5: exploration prompt
claude -p "/codebus-quiz plan: what is in this vault" \
  --tools Read,Glob,Grep --allowedTools Read,Glob,Grep --permission-mode acceptEdits \
  --output-format stream-json --verbose --include-partial-messages \
  | tee ../spike-quiz-8-E5.jsonl
```

**Pass criteria**: 5/5 spawns — zero `tool_use.input.path` matching `raw/`.

Quick check:
```bash
grep -E '"name":"(Read|Glob|Grep)"' ../spike-quiz-8-E*.jsonl | grep -E 'raw/|raw\\\\' && echo "VIOLATION" || echo "OK"
```

---

## ❾ Quiz md schema 穩定性 (3 generation runs)

```bash
# S1: 3 pages, count=5
claude -p "/codebus-quiz generate: pages=[wiki/modules/auth-middleware.md,wiki/concepts/jwt-token-lifecycle.md,wiki/processes/login-flow.md] count=5" \
  --tools Read,Glob,Grep --allowedTools Read,Glob,Grep --permission-mode acceptEdits \
  --output-format stream-json --verbose --include-partial-messages \
  | tee ../spike-quiz-9-S1.jsonl

# S2: single page, count=5
claude -p "/codebus-quiz generate: pages=[wiki/concepts/session-vs-token.md] count=5" \
  --tools Read,Glob,Grep --allowedTools Read,Glob,Grep --permission-mode acceptEdits \
  --output-format stream-json --verbose --include-partial-messages \
  | tee ../spike-quiz-9-S2.jsonl

# S3: 2 pages, count=3
claude -p "/codebus-quiz generate: pages=[wiki/modules/user-store.md,wiki/modules/auth-middleware.md] count=3" \
  --tools Read,Glob,Grep --allowedTools Read,Glob,Grep --permission-mode acceptEdits \
  --output-format stream-json --verbose --include-partial-messages \
  | tee ../spike-quiz-9-S3.jsonl
```

**Pass criteria**:
- S1 produces 5 `## Q<i>.` sections + 5 `## Answer:` lines + 5 `## Explanation:` lines + valid frontmatter
- S2 same with 5
- S3 same with 3

Quick check (extract just assistant final message and count sections):
```bash
for f in ../spike-quiz-9-S*.jsonl; do
  echo "=== $f ==="
  cat "$f" | jq -r 'select(.type=="assistant") | .message.content[]? | select(.type=="text") | .text' 2>/dev/null \
    | grep -c '^## Q'
done
```

---

## ❿ Retry 多樣性 (same input × 3 runs)

```bash
# Same input as S1 of ❾; run 3 times to compare Q stem overlap
for i in 1 2 3; do
  claude -p "/codebus-quiz generate: pages=[wiki/modules/auth-middleware.md,wiki/concepts/jwt-token-lifecycle.md] count=5" \
    --tools Read,Glob,Grep --allowedTools Read,Glob,Grep --permission-mode acceptEdits \
    --output-format stream-json --verbose --include-partial-messages \
    | tee ../spike-quiz-10-R1-run${i}.jsonl
done
```

**Pass criteria**: pair-wise Q stem Jaccard overlap < 0.3.

Quick extract of Q stems:
```bash
for f in ../spike-quiz-10-R1-run*.jsonl; do
  echo "=== $f ==="
  cat "$f" | jq -r 'select(.type=="assistant") | .message.content[]? | select(.type=="text") | .text' 2>/dev/null \
    | grep '^## Q' | sed 's/^## Q[0-9]*\. //'
done
```

---

## After all spikes

Report back the artifact files (or `ls ../spike-quiz-*.jsonl` output) and any obvious anomalies. I will:

1. Parse artifacts for marker compliance / schema completeness / raw access / Jaccard
2. Write §Spike results section into `docs/2026-05-15-v3-app-quiz-discussion.md`
3. Flag which spikes pass / fail / need re-run with SKILL adjustment

If any spike fails hard (e.g. agent ignores SKILL completely), stop and report — likely SKILL v0 phrasing needs revision before continuing.

## Total cost estimate

15 spawns × $0.3-0.5 ≈ **$5-8**. Wall time ≈ 15-20 min (sequential, each spawn 30-60s).
