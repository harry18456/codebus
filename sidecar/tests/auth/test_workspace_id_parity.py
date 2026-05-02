"""Frontend ↔ sidecar parity for ``workspace_id_for_path``.

Backs SHALL clauses in
``openspec/changes/entry-workspace-onramp/specs/workspace-onramp/spec.md``
  Requirement: Folder picker invocation flow
    Scenario: Selected path produces deterministic workspace_id

The fixture table here MUST mirror ``PATH_FIXTURES`` in
``web/tests/utils/workspace-id.spec.ts``. If either side bumps the
canonicalization algorithm, both tables MUST be regenerated in lockstep
(see the comment header on the frontend test for the regeneration
recipe). This test exists specifically to catch silent algorithm drift
between the two implementations.
"""
from __future__ import annotations

from pathlib import PureWindowsPath

import pytest

from codebus_agent.auth.service import workspace_id_for_path


# (input_path, expected_workspace_id)
#
# Same exact pairs the frontend asserts in
# ``web/tests/utils/workspace-id.spec.ts::PATH_FIXTURES``. The Windows
# fixture (`C:\\Users\\harry\\Code\\demo`) is materialized as a
# ``PureWindowsPath`` so ``Path.as_posix()`` performs the
# backslash-to-slash conversion the canonicalization rule expects on a
# real Windows host — wrapping in ``Path()`` on a POSIX runner would
# leave the backslashes untouched.
PARITY_FIXTURES: list[tuple[object, str]] = [
    ("/abs/path", "ws_6d80187b4541"),
    (PureWindowsPath("C:\\Users\\harry\\Code\\demo"), "ws_b3e6cc56defb"),
    ("c:/users/harry/code/demo", "ws_b3e6cc56defb"),
    ("C:/Users/Harry/Code/Demo", "ws_b3e6cc56defb"),
    ("/home/alice/projects/foo-bar", "ws_bb0b84426459"),
]


@pytest.mark.parametrize("input_path,expected", PARITY_FIXTURES)
def test_workspace_id_parity(input_path: object, expected: str) -> None:
    actual = workspace_id_for_path(input_path)  # type: ignore[arg-type]
    assert actual == expected, (
        f"workspace_id_for_path({input_path!r}) = {actual!r}, "
        f"but frontend asserts {expected!r}. The two implementations "
        "have drifted; bump both tables together."
    )
