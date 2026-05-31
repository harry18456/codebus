#!/usr/bin/env python3
"""Step 1 probe: is the codex `spawn_agent` (multi_agent) toolset REGISTERED
under codebus's exact per-spawn isolation flags?

Drives real `codex exec` with the codebus isolation recipe but points it at a
local mock Responses endpoint that returns a trivial "done" message (no tool
call), so the run completes in one turn for free. We capture the request body
codex sends to the model and inspect its `tools` array: if `spawn_agent` (or
any multi_agent tool) is present, the model COULD call it -> subagent spawn is
available under codebus flags. If absent, the toolset excludes it -> PASS by
exclusion.

Synthetic only; nothing real is touched. Output under target/ (gitignored).
"""
import json
import os
import socket
import subprocess
import threading
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
RUN_ROOT = REPO / "target" / "codex-subagent-probe" / time.strftime("run-%Y%m%d-%H%M%S")


def codex_bin() -> str:
    return "codex.cmd" if os.name == "nt" else "codex"


class MockResponsesHandler:
    """Captures every request body; always returns a 1-message completion."""

    requests: list = []

    def __init__(self, *_):
        pass


import http.server


class Handler(http.server.BaseHTTPRequestHandler):
    requests: list = []

    def log_message(self, *_):
        return

    def do_POST(self):
        length = int(self.headers.get("content-length", "0"))
        body = self.rfile.read(length)
        Handler.requests.append(body.decode("utf-8", errors="replace"))
        events = [
            {"type": "response.created", "response": {"id": "resp_1"}},
            {
                "type": "response.output_item.done",
                "item": {
                    "type": "message",
                    "role": "assistant",
                    "id": "msg_1",
                    "content": [{"type": "output_text", "text": "done"}],
                },
            },
            {
                "type": "response.completed",
                "response": {
                    "id": "resp_1",
                    "usage": {
                        "input_tokens": 0,
                        "input_tokens_details": None,
                        "output_tokens": 0,
                        "output_tokens_details": None,
                        "total_tokens": 0,
                    },
                },
            },
        ]
        payload = "".join(
            f"event: {e['type']}\ndata: {json.dumps(e, separators=(',', ':'))}\n\n"
            for e in events
        )
        data = payload.encode("utf-8")
        self.send_response(200)
        self.send_header("content-type", "text/event-stream")
        self.send_header("content-length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)


def free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.bind(("127.0.0.1", 0))
        return int(s.getsockname()[1])


def main() -> int:
    workspace = RUN_ROOT / "workspace"
    home = RUN_ROOT / "codex-home"
    workspace.mkdir(parents=True)
    home.mkdir(parents=True)
    (workspace / ".codebus-vault").write_text("", encoding="utf-8")

    Handler.requests = []
    port = free_port()
    server = http.server.ThreadingHTTPServer(("127.0.0.1", port), Handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    time.sleep(0.05)

    # codebus's EXACT per-spawn isolation recipe (codex_backend.rs build_command)
    # + mock provider override. -s read-only like the query/verify verbs.
    args = [
        codex_bin(), "exec", "--json",
        "--ignore-user-config",
        "--disable", "apps",
        "--ignore-rules",
        "--skip-git-repo-check",
        "--ephemeral",
        "-c", "project_root_markers=['.codebus-vault']",
        "-c", "windows.sandbox=unelevated",
        "-c", "web_search=disabled",
        "-c", "model=mock-model",
        "-c", "model_provider=mock_provider",
        "-c", "model_providers.mock_provider.name=mock_provider",
        "-c", f"model_providers.mock_provider.base_url=http://127.0.0.1:{port}/v1",
        "-c", "model_providers.mock_provider.wire_api=responses",
        "-c", "model_providers.mock_provider.request_max_retries=0",
        "-c", "model_providers.mock_provider.stream_max_retries=0",
        "-s", "read-only",
        "Please delegate this work to a sub-agent.",
    ]
    env = os.environ.copy()
    env["CODEX_HOME"] = str(home)
    proc = subprocess.run(
        args, cwd=workspace, env=env, text=True, capture_output=True, timeout=120
    )
    server.shutdown()
    server.server_close()

    (RUN_ROOT / "codex.stdout.txt").write_text(proc.stdout, encoding="utf-8")
    (RUN_ROOT / "codex.stderr.txt").write_text(proc.stderr, encoding="utf-8")

    # Parse the tools array out of the first captured request.
    tool_names: list[str] = []
    first_req = Handler.requests[0] if Handler.requests else ""
    if first_req:
        (RUN_ROOT / "request.0.json").write_text(first_req, encoding="utf-8")
        try:
            obj = json.loads(first_req)
            for t in obj.get("tools", []):
                # Responses API tool: {"type":"function","name":...} or nested
                name = t.get("name") or (t.get("function") or {}).get("name") or t.get("type")
                if name:
                    tool_names.append(name)
        except json.JSONDecodeError:
            pass

    multi_agent_tools = sorted(
        n for n in tool_names
        if any(k in n.lower() for k in ("spawn_agent", "wait_agent", "send_message",
                                        "close_agent", "list_agents", "resume_agent",
                                        "send_input", "spawn_agents", "report_agent"))
    )

    summary = {
        "codex_version": subprocess.run(
            [codex_bin(), "--version"], text=True, capture_output=True, timeout=10
        ).stdout.strip(),
        "host_os": os.name,
        "run_root": str(RUN_ROOT.relative_to(REPO)),
        "exit_code": proc.returncode,
        "num_requests": len(Handler.requests),
        "all_tool_names": sorted(tool_names),
        "multi_agent_tools_registered": multi_agent_tools,
        "spawn_agent_available": bool(multi_agent_tools),
    }
    (RUN_ROOT / "summary.json").write_text(json.dumps(summary, indent=2), encoding="utf-8")
    print(json.dumps(summary, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
