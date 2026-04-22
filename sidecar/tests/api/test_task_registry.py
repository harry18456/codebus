"""TDD red tests for the single-slot TaskRegistry — Section 2 of
openspec/changes/sse-progress-skeleton/tasks.md.

Backs openspec/changes/sse-progress-skeleton/specs/sidecar-runtime/spec.md
  Requirement: Single-slot in-memory task registry
  Requirement: task_id format

Design `Single-slot task store over dict-based pool`: registry holds a
single ``Optional[TaskHandle]`` rather than ``Dict[task_id, TaskHandle]``.
Once a task transitions to ``done`` or ``error`` the handle survives in
the slot until the next successful ``create`` overwrites it.
"""
from __future__ import annotations

import re

import pytest

from codebus_agent.api.tasks import TaskRegistry, _generate_task_id

_TASK_ID_RE = re.compile(r"^(scan|kb)_[0-9a-f]{8}$")


def test_registry_is_single_slot_and_overwrites_on_new_task() -> None:
    """After a task transitions to done, the next create() overwrites the slot.

    Spec: "After a task transitions to done or error, its handle and result
    SHALL remain reachable via the registry until a subsequent successful
    task creation overwrites the slot."
    """
    registry = TaskRegistry()
    first = registry.create("scan")
    assert first is not None
    first_id = first.id

    # Mark first task done so the slot is no longer "running".
    first.status = "done"
    first.result = {"workspace_root": "/tmp/x"}

    # Slot still reachable via get() before overwrite.
    assert registry.get(first_id) is first

    # Creating a new task overwrites the slot — the old handle must no
    # longer be reachable via get(); the new one must be.
    second = registry.create("kb")
    assert second is not None
    assert second is not first
    assert registry.get(second.id) is second
    assert registry.get(first_id) is None


def test_running_task_blocks_new_task_creation() -> None:
    """A second create() while another task is running MUST return None
    (the endpoint layer translates None → HTTP 409 / TASK_IN_FLIGHT).
    """
    registry = TaskRegistry()
    running = registry.create("scan")
    assert running is not None
    assert running.status == "running"
    assert registry.current_running() is running

    blocked = registry.create("kb")
    assert blocked is None, "create() MUST refuse while another task is running"
    # The original handle is still the only one in the registry.
    assert registry.current_running() is running
    assert registry.get(running.id) is running


def test_terminal_handle_survives_until_overwritten() -> None:
    """A done handle's result MUST stay reachable until a new create() lands.

    Spec: "After a task transitions to done or error, its handle and result
    SHALL remain reachable via the registry until a subsequent successful
    task creation overwrites the slot."
    """
    registry = TaskRegistry()
    handle = registry.create("scan")
    assert handle is not None

    # Simulate a successful background run.
    handle.status = "done"
    handle.result = {"workspace_root": "/tmp/x", "files": []}

    # Even though no task is "running", the terminal handle is still the
    # registry's slot — get() and result reachability MUST hold.
    assert registry.current_running() is None
    fetched = registry.get(handle.id)
    assert fetched is handle
    assert fetched.result == {"workspace_root": "/tmp/x", "files": []}


def test_task_id_format_matches_regex() -> None:
    """Generated ids MUST match `^(scan|kb)_[0-9a-f]{8}$` per spec
    `task_id format` and design `task_id 用前綴 + 8 字 hex random`.
    """
    for _ in range(50):
        scan_id = _generate_task_id("scan")
        kb_id = _generate_task_id("kb")
        assert _TASK_ID_RE.fullmatch(scan_id), f"bad scan id {scan_id!r}"
        assert _TASK_ID_RE.fullmatch(kb_id), f"bad kb id {kb_id!r}"

    with pytest.raises(ValueError):
        _generate_task_id("explore")  # type: ignore[arg-type]
