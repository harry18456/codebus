"""SSE wire format end-to-end tests.

Backs ``openspec/changes/sidecar-sse-named-events-and-error-listener-fix``
spec delta in ``specs/sidecar-runtime/spec.md`` —
  Requirement: SSE event stream endpoint
  Scenario: Wire format includes both event and data lines per emission

Existing ``test_tasks_sse.py`` only asserts queue ordering at the
``TaskHandle`` API surface; it does NOT exercise the HTTP wire bytes
that ``EventSourceResponse`` produces. Without an ``event:`` line the
browser collapses every SSE message into the default ``message`` channel
and named ``EventSource.addEventListener("done", ...)`` / ``("progress", ...)``
listeners never fire. These tests lock the wire format down so a future
regression that drops the ``event:`` line is caught at unit-test time
instead of via manual smoke.

Implementation note — the task description references
``fastapi.testclient.TestClient.stream`` but the project's existing
SSE integration tests (``test_explore_sse_integration.py``) use
``httpx.AsyncClient`` + ``ASGITransport`` because we need to emit
events into the ``TaskHandle`` from a background coroutine while the
HTTP subscriber blocks on ``await queue.get()``. The async pattern
mirrors the established convention.
"""
from __future__ import annotations

import asyncio
import json
import secrets

import httpx
import pytest

from codebus_agent.api import create_app


async def _stream_with_background_emits(
    bearer: str, events_to_emit: list[dict]
) -> bytes:
    """Connect SSE, emit `events_to_emit` from a background coroutine, return raw wire bytes.

    Pattern: `gather()` the HTTP stream and the emit-coroutine. The emit
    coroutine sleeps briefly to let the subscriber connect, then drives
    the events through `handle.emit(...)`, then `close_subscribers()` to
    let the generator return cleanly.
    """
    app = create_app(bearer_token=bearer)
    handle = app.state.tasks.create("scan")
    assert handle is not None

    async def emit_in_background() -> None:
        # 50ms is enough for the AsyncClient to reach `await queue.get()`
        # — the existing test_explore_sse_integration tests rely on the
        # same ordering guarantee implicitly.
        await asyncio.sleep(0.05)
        for ev in events_to_emit:
            handle.emit(ev)
        handle.close_subscribers()

    async def fetch_wire() -> bytes:
        chunks: list[bytes] = []
        transport = httpx.ASGITransport(app=app)
        async with httpx.AsyncClient(
            transport=transport, base_url="http://test"
        ) as client:
            async with client.stream(
                "GET",
                f"/tasks/{handle.id}/events",
                headers={"Authorization": f"Bearer {bearer}"},
            ) as resp:
                assert resp.status_code == 200, await resp.aread()
                async for chunk in resp.aiter_raw():
                    chunks.append(chunk)
        return b"".join(chunks)

    wire_bytes, _ = await asyncio.gather(fetch_wire(), emit_in_background())
    return wire_bytes


@pytest.mark.asyncio
@pytest.mark.parametrize(
    "event,expected_event_line",
    [
        (
            {"type": "progress", "phase": "scanning", "current": 1, "total": 3},
            b"event: progress\r\n",
        ),
        ({"type": "done"}, b"event: done\r\n"),
        (
            {"type": "error", "code": "X", "message": "y"},
            b"event: error\r\n",
        ),
    ],
    ids=["progress", "done", "error"],
)
async def test_sse_wire_includes_event_and_data_lines_per_emission(
    event: dict, expected_event_line: bytes
) -> None:
    """Each emission MUST produce both an ``event: <type>`` line and a
    ``data: <json>`` line on the wire.

    Spec scenario sub-clauses (a) (b) (c): the named ``event:`` line for
    each of the three event types ``progress`` / ``done`` / ``error``
    MUST appear in the response body bytes.
    """
    bearer = secrets.token_urlsafe(32)
    wire = await _stream_with_background_emits(bearer, [event])

    # Sub-clause (a)/(b)/(c): named `event:` line present
    assert expected_event_line in wire, (
        f"missing {expected_event_line!r} in wire bytes: {wire!r}"
    )
    # Companion: `data:` line also present (already true pre-fix)
    expected_json = json.dumps(event, separators=(",", ":"), ensure_ascii=False)
    expected_data_line = f"data: {expected_json}".encode("utf-8")
    assert expected_data_line in wire, (
        f"missing {expected_data_line!r} in wire bytes: {wire!r}"
    )


@pytest.mark.asyncio
async def test_sse_wire_event_line_value_matches_inner_type_field() -> None:
    """Spec scenario sub-clause (d): the ``<type>`` value on the
    ``event:`` line MUST exactly equal the ``type`` field inside the JSON
    on the ``data:`` line. Drift between the two would mean a server-emitted
    ``event: error`` carrying inner ``"type":"done"`` (or vice versa),
    which would silently break consumer dispatch logic.
    """
    bearer = secrets.token_urlsafe(32)
    events = [
        {"type": "progress", "phase": "scanning", "current": 5, "total": 10},
        {"type": "done"},
    ]
    wire = await _stream_with_background_emits(bearer, events)

    # Walk through each SSE message block (separated by blank line) and
    # verify the event: line type matches the data: line JSON's `type`
    # field. We split on the SSE record separator b"\r\n\r\n".
    blocks = [b for b in wire.split(b"\r\n\r\n") if b.strip()]
    assert len(blocks) == len(events), (
        f"expected {len(events)} SSE blocks, got {len(blocks)}: {wire!r}"
    )
    for block, ev in zip(blocks, events, strict=True):
        lines = block.split(b"\r\n")
        event_line = next((l for l in lines if l.startswith(b"event: ")), None)
        data_line = next((l for l in lines if l.startswith(b"data: ")), None)
        assert event_line is not None, f"no event: line in block {block!r}"
        assert data_line is not None, f"no data: line in block {block!r}"

        wire_event_type = event_line[len(b"event: ") :].decode("utf-8").strip()
        inner_payload = json.loads(data_line[len(b"data: ") :].decode("utf-8"))
        assert wire_event_type == ev["type"]
        assert inner_payload["type"] == ev["type"]


@pytest.mark.asyncio
async def test_sse_wire_event_line_defaults_to_message_when_type_missing() -> None:
    """Spec scenario sub-clause (e): when an emitted dict lacks the
    ``type`` key, the ``event:`` line value MUST default to ``message``
    so the browser falls back to the default SSE channel rather than
    raising / dropping the frame.

    No production code path is expected to omit ``type`` today, but the
    fallback exists as a defensive guarantee — the alternative
    ``event["type"]`` would raise ``KeyError`` and abort the stream.
    """
    bearer = secrets.token_urlsafe(32)
    wire = await _stream_with_background_emits(
        bearer, [{"phase": "x"}]  # NO `type` key
    )
    assert b"event: message\r\n" in wire, (
        f"expected 'event: message' fallback in wire: {wire!r}"
    )
    assert b'data: {"phase":"x"}' in wire, (
        f"expected data line for typeless dict in wire: {wire!r}"
    )
