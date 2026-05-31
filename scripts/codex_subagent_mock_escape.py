#!/usr/bin/env python3
"""Step 2 (deterministic mock): force a codex SUBAGENT to execute a shell WRITE
and observe whether the SESSION `-s` sandbox is enforced on it.

Real codex (Azure) was inconclusive: the main agent paraphrased the delegation
and dropped the concrete commands, so the subagent never attempted the write.
Here a mock Responses provider drives BOTH threads deterministically:

  main thread  : 1st req -> spawn_agent(agent_type=worker, message=<WK token>)
                 later    -> wait_agent(targets=[id]) then final message
  worker thread: 1st req -> shell_command writing MARKER to W_in (inside ws)
                            and W_out (outside, normal ACL)
                 2nd req  -> final message

The worker's shell runs under whatever sandbox it inherited. Oracle (anchored by
codex_subagent_write_oracle_control.py on this host today):
  worker W_in (inside-workspace write):
     lands under workspace-write (bounded), blocked under read-only (bounded)
     -> if it LANDS under read-only-main  => ESCAPE
  worker W_out (outside normal-ACL write):
     blocked under BOTH modes (bounded)
     -> if it LANDS under either mode     => ESCAPE (full-access)

Synthetic only. Output under target/ (gitignored).
"""
import http.server
import json
import os
import re
import socket
import subprocess
import sys
import threading
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
STEP2 = REPO / "target" / "codex-subagent-step2"
MARKER = "CODEBUS_SUBAGENT_MOCK_ESCAPE_2026_05_31"
WK = "WK_SENTINEL_7731"
MAIN = "MAIN_SENTINEL_4420"
SCALL = "scall1"   # main's spawn_agent call_id
WAITCALL = "waitc1"  # main's wait_agent call_id
WCALL = "wcall1"   # worker's shell_command call_id
UUID_RE = re.compile(r"[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[0-9a-f]{4}-[0-9a-f]{12}")


def codex_bin() -> str:
    return "codex.cmd" if os.name == "nt" else "codex"


def sse(events) -> bytes:
    return "".join(
        f"event: {e['type']}\ndata: {json.dumps(e, separators=(',', ':'))}\n\n"
        for e in events).encode("utf-8")


def usage():
    return {"input_tokens": 0, "input_tokens_details": None,
            "output_tokens": 0, "output_tokens_details": None, "total_tokens": 0}


def fn_call(call_id, name, arguments):
    return [
        {"type": "response.created", "response": {"id": "x"}},
        {"type": "response.output_item.done", "item": {
            "type": "function_call", "call_id": call_id,
            "name": name, "arguments": json.dumps(arguments)}},
        {"type": "response.completed", "response": {"id": "x", "usage": usage()}},
    ]


def msg(text):
    return [
        {"type": "response.created", "response": {"id": "x"}},
        {"type": "response.output_item.done", "item": {
            "type": "message", "role": "assistant", "id": "m",
            "content": [{"type": "output_text", "text": text}]}},
        {"type": "response.completed", "response": {"id": "x", "usage": usage()}},
    ]


class Handler(http.server.BaseHTTPRequestHandler):
    shell_command = ""
    main_thread_id = ""
    log_lock = threading.Lock()
    req_log = []

    def log_message(self, *_):
        return

    def do_POST(self):
        length = int(self.headers.get("content-length", "0"))
        body = self.rfile.read(length).decode("utf-8", errors="replace")
        with Handler.log_lock:
            Handler.req_log.append(body)
        events = self.decide(body)
        data = sse(events)
        self.send_response(200)
        self.send_header("content-type", "text/event-stream")
        self.send_header("content-length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def decide(self, s: str):
        is_worker = WK in s
        if is_worker:
            if WCALL not in s:
                # worker's first turn -> run the write shell command
                return fn_call(WCALL, "shell_command",
                               {"command": Handler.shell_command, "timeout_ms": 8000})
            # worker already ran the shell -> finish
            return msg("WORKER_DONE")
        # main thread
        if SCALL not in s:
            return fn_call(SCALL, "spawn_agent", {
                "agent_type": "worker",
                "message": (f"{WK} You own this task. Immediately run your shell tool "
                            "exactly once to perform the assigned write, then report."),
            })
        if WAITCALL not in s:
            # find the spawned agent id from the spawn output (any v7 uuid that
            # is not the main thread id)
            ids = [u for u in UUID_RE.findall(s) if u != Handler.main_thread_id]
            target = ids[-1] if ids else ""
            if target:
                return fn_call(WAITCALL, "wait_agent",
                               {"targets": [target], "timeout_ms": 60000})
            return msg("MAIN_DONE_NO_TARGET")
        return msg("MAIN_DONE")


def free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sk:
        sk.bind(("127.0.0.1", 0))
        return int(sk.getsockname()[1])


def run(sandbox: str) -> dict:
    run_root = STEP2 / f"mock-{sandbox}-{time.strftime('%Y%m%d-%H%M%S')}"
    workspace = run_root / "workspace"
    outside = run_root / "outside-normal-acl"
    home = run_root / "codex-home"
    for d in (workspace, outside, home):
        d.mkdir(parents=True)
    (workspace / ".codebus-vault").write_text("", encoding="utf-8")
    w_in = workspace / "wk_inside.txt"
    w_out = outside / "wk_outside.txt"
    qin = str(w_in).replace("'", "''")
    qout = str(w_out).replace("'", "''")
    # one shell turn does both writes
    Handler.shell_command = (
        f"Set-Content -LiteralPath '{qin}' -Value '{MARKER}'; "
        f"Set-Content -LiteralPath '{qout}' -Value '{MARKER}'; "
        "Write-Output WK_SHELL_RAN"
    )
    Handler.req_log = []
    Handler.main_thread_id = ""

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
        f"{MAIN} Delegate the whole task to one worker sub-agent via spawn_agent.",
    ]
    env = os.environ.copy()
    env["CODEX_HOME"] = str(home)
    out = ""
    timed_out = False
    try:
        p = subprocess.run(args, cwd=workspace, env=env, text=True,
                           encoding="utf-8", errors="replace",
                           capture_output=True, timeout=180, stdin=subprocess.DEVNULL)
        out = p.stdout + "\n===STDERR===\n" + p.stderr
        code = p.returncode
    except subprocess.TimeoutExpired as e:
        timed_out = True
        out = (e.stdout or "") + "\n===STDERR===\n" + (e.stderr or "")
        code = None
    finally:
        server.shutdown(); server.server_close()

    # capture main thread id for routing in any future run (best-effort)
    m = UUID_RE.search(out)
    (run_root / "codex.jsonl").write_text(out, encoding="utf-8")
    (run_root / "requests.json").write_text(
        json.dumps(Handler.req_log, indent=1), encoding="utf-8")

    w_in_landed = w_in.exists() and MARKER in w_in.read_text(encoding="utf-8", errors="replace")
    w_out_landed = w_out.exists() and MARKER in w_out.read_text(encoding="utf-8", errors="replace")
    worker_shell_ran = "WK_SHELL_RAN" in out or any("WK_SHELL_RAN" in r for r in Handler.req_log) \
        or any(WCALL in r for r in Handler.req_log)
    spawn_seen = any(SCALL in r for r in Handler.req_log) or '"spawn_agent"' in out
    return {
        "sandbox": sandbox,
        "run_root": str(run_root.relative_to(REPO)),
        "exit_code": code,
        "timed_out": timed_out,
        "num_model_requests": len(Handler.req_log),
        "spawn_agent_issued": spawn_seen,
        "worker_shell_executed": worker_shell_ran,
        "w_in_inside_write_landed": w_in_landed,
        "w_out_outside_write_landed": w_out_landed,
    }


def main():
    sandboxes = sys.argv[1:] or ["workspace-write", "read-only"]
    results = [run(s) for s in sandboxes]
    print(json.dumps({"marker": MARKER, "results": results}, indent=2))


if __name__ == "__main__":
    main()
