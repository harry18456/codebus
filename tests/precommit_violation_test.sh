#!/usr/bin/env bash
# Asserts that stage-0 pre-commit hooks BLOCK a deliberately-violating
# commit attempt (negative counterpart to precommit_gate_test.sh).
#
# Backs the SHALL clause in
# `openspec/changes/m1-power-on/specs/repo-layout/spec.md`
#   Requirement: Pre-commit stage-0 hooks configured
#   Scenario:    Stage-0 hooks run on commit
# and serves Phase 9 task 9.5 ("trailing whitespace + broken JSON must
# be blocked by the hook, proving the gate actually fires").
#
# The script writes two decoy files under `tests/fixtures/precommit-violations/`,
# tries to commit them, and fails if the hooks DID NOT reject them.
# Cleans up on exit regardless of outcome.
set -uo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

if ! command -v pre-commit >/dev/null 2>&1; then
  echo "FAIL: pre-commit binary not in PATH" >&2
  exit 1
fi

violations_dir="tests/fixtures/precommit-violations"
mkdir -p "$violations_dir"

whitespace_file="$violations_dir/trailing.txt"
badjson_file="$violations_dir/broken.json"

cleanup() {
  git restore --staged "$whitespace_file" "$badjson_file" 2>/dev/null || true
  rm -f "$whitespace_file" "$badjson_file"
  rmdir "$violations_dir" 2>/dev/null || true
  # Attempt to remove the fixtures parent only if it is empty (do not
  # clobber any real fixtures the repo already has).
}
trap cleanup EXIT

# Deliberate violations:
#   - trailing whitespace on line 1 (caught by trailing-whitespace hook)
#   - malformed JSON (caught by check-json hook)
printf 'line with trailing ws   \n' > "$whitespace_file"
printf '{ "broken": true,,, }\n'     > "$badjson_file"

git add "$whitespace_file" "$badjson_file"

set +e
pre-commit run --files "$whitespace_file" "$badjson_file" > /tmp/precommit_violation.log 2>&1
hook_exit=$?
set -e

if [[ $hook_exit -eq 0 ]]; then
  echo "FAIL: pre-commit returned 0 — the hook DID NOT block the violating files" >&2
  cat /tmp/precommit_violation.log >&2
  exit 1
fi

if ! grep -qiE 'trailing|whitespace' /tmp/precommit_violation.log; then
  echo "FAIL: pre-commit blocked but did not mention trailing-whitespace" >&2
  cat /tmp/precommit_violation.log >&2
  exit 1
fi

if ! grep -qiE 'json' /tmp/precommit_violation.log; then
  echo "FAIL: pre-commit blocked but did not mention JSON validation" >&2
  cat /tmp/precommit_violation.log >&2
  exit 1
fi

echo "PASS: stage-0 hooks correctly blocked trailing-ws + broken-json"
