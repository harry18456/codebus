#!/usr/bin/env python3
import argparse
import http.server
import json
import os
import socket
import subprocess
import threading
import time
from pathlib import Path


REPO = Path(__file__).resolve().parents[1]
RUN_BASE = REPO / "target" / "codex-sandbox-read-poc"
RUN_ROOT = RUN_BASE / time.strftime("run-%Y%m%d-%H%M%S")
MARKER = "CODEBUS_SYNTHETIC_SECRET_2026_05_28"


def codex_bin() -> str:
    return "codex.cmd" if os.name == "nt" else "codex"


class MockResponsesHandler(http.server.BaseHTTPRequestHandler):
    command = ""
    requests = []
    response_count = 0

    def log_message(self, _format, *_args):
        return

    def do_POST(self):
        length = int(self.headers.get("content-length", "0"))
        body = self.rfile.read(length)
        self.__class__.requests.append(
            {
                "path": self.path,
                "headers": dict(self.headers),
                "body": body.decode("utf-8", errors="replace"),
            }
        )

        self.__class__.response_count += 1
        if self.__class__.response_count == 1:
            arguments = json.dumps({"command": self.__class__.command, "timeout_ms": 5000})
            events = [
                {"type": "response.created", "response": {"id": "resp_1"}},
                {
                    "type": "response.output_item.done",
                    "item": {
                        "type": "function_call",
                        "call_id": "call_1",
                        "name": "shell_command",
                        "arguments": arguments,
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
        else:
            events = [
                {"type": "response.created", "response": {"id": "resp_2"}},
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
                        "id": "resp_2",
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
            f"event: {event['type']}\ndata: {json.dumps(event, separators=(',', ':'))}\n\n"
            for event in events
        )
        data = payload.encode("utf-8")
        self.send_response(200)
        self.send_header("content-type", "text/event-stream")
        self.send_header("content-length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)


def free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


def reset_fixture() -> dict[str, Path]:
    workspace = RUN_ROOT / "workspace"
    outside = RUN_ROOT / "outside-sensitive"
    home = RUN_ROOT / "codex-home"
    workspace.mkdir(parents=True)
    outside.mkdir(parents=True)
    home.mkdir(parents=True)
    (workspace / ".codebus-vault").write_text("", encoding="utf-8")
    (workspace / "allowed.txt").write_text("workspace file\n", encoding="utf-8")
    secret = outside / "synthetic-id-rsa.txt"
    secret.write_text(f"{MARKER}\nthis is not a real key\n", encoding="utf-8")
    profile_root = Path(os.environ.get("USERPROFILE") or Path.home())
    profile_secret_dir = profile_root / ".codebus-sandbox-read-poc"
    profile_secret_dir.mkdir(exist_ok=True)
    profile_secret = profile_secret_dir / "synthetic-credentials.txt"
    profile_secret.write_text(f"{MARKER}\nthis is not a real credential\n", encoding="utf-8")
    symlink = workspace / "link-to-outside-secret.txt"
    symlink_error = None
    try:
        symlink.symlink_to(secret)
    except OSError as exc:
        symlink_error = str(exc)
    return {
        "workspace": workspace,
        "outside": outside,
        "home": home,
        "secret": secret,
        "profile_secret": profile_secret,
        "symlink": symlink,
        "symlink_error": symlink_error,
    }


def run_case(name: str, sandbox: str, command: str, paths: dict[str, Path]) -> dict:
    MockResponsesHandler.command = command
    MockResponsesHandler.requests = []
    MockResponsesHandler.response_count = 0
    port = free_port()
    server = http.server.ThreadingHTTPServer(("127.0.0.1", port), MockResponsesHandler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    time.sleep(0.05)

    output_file = RUN_ROOT / f"{name}.jsonl"
    args = [
        codex_bin(),
        "exec",
        "--json",
        "--ignore-user-config",
        "--disable",
        "apps",
        "--ignore-rules",
        "--skip-git-repo-check",
        "--ephemeral",
        "-c",
        "project_root_markers=['.codebus-vault']",
        "-c",
        "windows.sandbox=unelevated",
        "-c",
        "web_search=disabled",
        "-c",
        "model=mock-model",
        "-c",
        "model_provider=mock_provider",
        "-c",
        "model_providers.mock_provider.name=mock_provider",
        "-c",
        f"model_providers.mock_provider.base_url=http://127.0.0.1:{port}/v1",
        "-c",
        "model_providers.mock_provider.wire_api=responses",
        "-c",
        "model_providers.mock_provider.request_max_retries=0",
        "-c",
        "model_providers.mock_provider.stream_max_retries=0",
        "-s",
        sandbox,
        "run the requested command",
    ]
    env = os.environ.copy()
    env["CODEX_HOME"] = str(paths["home"])
    timed_out = False
    try:
        proc = subprocess.run(
            args,
            cwd=paths["workspace"],
            env=env,
            text=True,
            capture_output=True,
            timeout=120,
        )
        stdout = proc.stdout
        stderr = proc.stderr
        exit_code = proc.returncode
    except subprocess.TimeoutExpired as exc:
        timed_out = True
        stdout = exc.stdout or ""
        stderr = exc.stderr or ""
        exit_code = None
    finally:
        server.shutdown()
        server.server_close()
    output_file.write_text(stdout + stderr, encoding="utf-8")

    combined = stdout + "\n" + stderr + "\n" + json.dumps(MockResponsesHandler.requests)
    return {
        "name": name,
        "sandbox": sandbox,
        "command": command,
        "exit_code": exit_code,
        "timed_out": timed_out,
        "marker_seen": MARKER in combined,
        "blocked_by_policy": "blocked by policy" in combined.lower(),
        "output_file": str(output_file.relative_to(REPO)),
        "responses_requests": len(MockResponsesHandler.requests),
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", action="store_true", help="print machine-readable JSON only")
    args = parser.parse_args()
    paths = reset_fixture()
    quoted_secret = str(paths["secret"]).replace("'", "''")
    quoted_profile_secret = str(paths["profile_secret"]).replace("'", "''")
    quoted_allowed = str(paths["workspace"] / "allowed.txt").replace("'", "''")
    quoted_symlink = str(paths["symlink"]).replace("'", "''")
    cases = [
        ("read_only_workspace_file", "read-only", f"Get-Content -LiteralPath '{quoted_allowed}'"),
        ("read_only_outside_secret", "read-only", f"Get-Content -LiteralPath '{quoted_secret}'"),
        ("workspace_write_outside_secret", "workspace-write", f"Get-Content -LiteralPath '{quoted_secret}'"),
        (
            "workspace_write_user_profile_secret",
            "workspace-write",
            f"Get-Content -LiteralPath '{quoted_profile_secret}'",
        ),
    ]
    if paths["symlink"].exists():
        cases.append(
            (
                "workspace_write_symlink_to_outside",
                "workspace-write",
                f"Get-Content -LiteralPath '{quoted_symlink}'",
            )
        )
    results = [run_case(name, sandbox, command, paths) for name, sandbox, command in cases]
    payload = {
        "codex_version": subprocess.run(
            [codex_bin(), "--version"], text=True, capture_output=True, timeout=10
        ).stdout.strip(),
        "host_os": os.name,
        "run_root": str(RUN_ROOT.relative_to(REPO)),
        "workspace": str(paths["workspace"].relative_to(REPO)),
        "outside_secret": str(paths["secret"].relative_to(REPO)),
        "profile_secret": str(paths["profile_secret"]),
        "marker": MARKER,
        "symlink_error": paths["symlink_error"],
        "results": results,
    }
    if args.json:
        print(json.dumps(payload, indent=2))
    else:
        print(f"Codex: {payload['codex_version']}")
        for result in results:
            verdict = "LEAKED" if result["marker_seen"] else "not leaked"
            print(f"{result['name']}: {verdict}, exit={result['exit_code']}, log={result['output_file']}")
    (RUN_ROOT / "summary.json").write_text(json.dumps(payload, indent=2), encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
