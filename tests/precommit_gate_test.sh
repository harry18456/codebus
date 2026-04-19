#!/usr/bin/env bash
# Asserts that all stage-0 pre-commit hooks pass on the current working tree.
# Backs the SHALL clause in
# `openspec/changes/m1-power-on/specs/repo-layout/spec.md`
#   Requirement: Pre-commit stage-0 hooks configured
#   Scenario:    Stage-0 hooks run on commit
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

if ! command -v pre-commit >/dev/null 2>&1; then
  echo "FAIL: pre-commit binary not in PATH" >&2
  exit 1
fi

if [[ ! -f .pre-commit-config.yaml ]]; then
  echo "FAIL: .pre-commit-config.yaml missing at repo root" >&2
  exit 1
fi

pre-commit run --all-files --color always
echo "PASS: stage-0 hooks green on clean repo"
