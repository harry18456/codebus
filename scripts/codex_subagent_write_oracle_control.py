#!/usr/bin/env python3
"""Step 2 ANCHOR (deterministic mock control): characterize the WRITE sandbox
oracle on THIS host today, for the MAIN agent (no subagent).

The codex READ sandbox is known-broken on Windows unelevated (both modes leak
reads outside the workspace), so a "read outside file" oracle cannot tell a
bounded subagent from an unbounded one. The WRITE sandbox IS enforced. This
control forces the MAIN codex agent (via a mock Responses provider) to attempt
writes, establishing the ground-truth verdicts the subagent test is read
against:

  read-only   + write inside workspace   -> EXPECT blocked
  workspace-write + write inside workspace -> EXPECT allowed
  read-only   + write outside (normal ACL)-> EXPECT blocked
  workspace-write + write outside (normalACL)->EXPECT blocked

Synthetic only. Output under target/ (gitignored).
"""
import http.server
import json
import os
import socket
import subprocess
import threading
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
RUN_ROOT = REPO / "target" / "codex-subagent-step2" / time.strftime("control-%Y%m%d-%H%M%S")
MARKER = "CODEBUS_SUBAGENT_WRITE_PROBE_2026_05_31"


def codex_bin() -> str:
    return "codex.cmd" if os.name == "nt" else "codex"


class Handler(http.server.BaseHTTPRequestHandler):
    command = ""
    response_count = 0

    def log_message(self, *_):
        return

    def do_POST(self):
        length = int(self.headers.get("content-length", "0"))
        self.rfile.read(length)
        Handler.response_count += 1
        if Handler.response_count == 1:
            args = json.dumps({"command": Handler.command, "timeout_ms": 5000})
            events = [
                {"type": "response.created", "response": {"id": "r1"}},
                {"type": "response.output_item.done", "item": {
                    "type": "function_call", "call_id": "c1",
                    "name": "shell_command", "arguments": args}},
                {"type": "response.completed", "response": {"id": "r1", "usage": {
                    "input_tokens": 0, "input_tokens_details": None,
                    "output_tokens": 0, "output_tokens_details": None,
                    "total_tokens": 0}}},
            ]
        else:
            events = [
                {"type": "response.created", "response": {"id": "r2"}},
                {"type": "response.output_item.done", "item": {
                    "type": "message", "role": "assistant", "id": "m1",
                    "content": [{"type": "output_text", "text": "done"}]}},
                {"type": "response.completed", "response": {"id": "r2", "usage": {
                    "input_tokens": 0, "input_tokens_details": None,
                    "output_tokens": 0, "output_tokens_details": None,
                    "total_tokens": 0}}},
            ]
        payload = "".join(
            f"event: {e['type']}\ndata: {json.dumps(e, separators=(',', ':'))}\n\n"
            for e in events).encode("utf-8")
        self.send_response(200)
        self.send_header("content-type", "text/event-stream")
        self.send_header("content-length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)


def free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.bind(("127.0.0.1", 0))
        return int(s.getsockname()[1])


def run_case(name, sandbox, target_file, workspace, home):
    # Force a write of MARKER to target_file via PowerShell Set-Content.
    q = str(target_file).replace("'", "''")
    Handler.command = f"Set-Content -LiteralPath '{q}' -Value '{MARKER}'"
    Handler.response_count = 0
    if target_file.exists():
        target_file.unlink()
    port = free_port()
    server = http.server.ThreadingHTTPServer(("127.0.0.1", port), Handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    time.sleep(0.05)
    args = [
        codex_bin(), "exec", "--json",
        "--ignore-user-config", "--disable", "apps", "--ignore-rules",
        "--skip-git-repo-check", "--ephemeral",
        "-c", "project_root_markers=['.codebus-vault']",
        "-c", "windows.sandbox=unelevated", "-c", "web_search=disabled",
        "-c", "model=mock-model", "-c", "model_provider=mock_provider",
        "-c", "model_providers.mock_provider.name=mock_provider",
        "-c", f"model_providers.mock_provider.base_url=http://127.0.0.1:{port}/v1",
        "-c", "model_providers.mock_provider.wire_api=responses",
        "-c", "model_providers.mock_provider.request_max_retries=0",
        "-c", "model_providers.mock_provider.stream_max_retries=0",
        "-s", sandbox,
        "run the requested command",
    ]
    env = os.environ.copy()
    env["CODEX_HOME"] = str(home)
    out = ""
    try:
        p = subprocess.run(args, cwd=workspace, env=env, text=True,
                           capture_output=True, timeout=120)
        out = p.stdout + "\n" + p.stderr
        code = p.returncode
    except subprocess.TimeoutExpired as e:
        out = (e.stdout or "") + "\n" + (e.stderr or "")
        code = None
    finally:
        server.shutdown(); server.server_close()
    (RUN_ROOT / f"{name}.log").write_text(out, encoding="utf-8")
    wrote = target_file.exists() and MARKER in target_file.read_text(encoding="utf-8", errors="replace")
    denied = any(s in out.lower() for s in ("access is denied", "blocked by policy",
                                            "permission denied", "sandbox", "denied"))
    return {"name": name, "sandbox": sandbox, "target": str(target_file),
            "write_succeeded": wrote, "denial_text_seen": denied, "exit_code": code,
            "log": str((RUN_ROOT / f"{name}.log").relative_to(REPO))}


def main():
    workspace = RUN_ROOT / "workspace"
    outside = RUN_ROOT / "outside-normal-acl"
    home = RUN_ROOT / "codex-home"
    for d in (workspace, outside, home):
        d.mkdir(parents=True)
    (workspace / ".codebus-vault").write_text("", encoding="utf-8")
    w_in = workspace / "inside_write.txt"
    w_out = outside / "outside_write.txt"
    cases = [
        ("ro_inside", "read-only", w_in),
        ("ww_inside", "workspace-write", w_in),
        ("ro_outside", "read-only", w_out),
        ("ww_outside", "workspace-write", w_out),
    ]
    results = [run_case(n, s, f, workspace, home) for n, s, f in cases]
    summary = {
        "codex_version": subprocess.run([codex_bin(), "--version"], text=True,
                                        capture_output=True, timeout=10).stdout.strip(),
        "marker": MARKER, "run_root": str(RUN_ROOT.relative_to(REPO)),
        "results": results,
    }
    (RUN_ROOT / "summary.json").write_text(json.dumps(summary, indent=2), encoding="utf-8")
    print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    main()
