//! Test-only mock for `claude -p`. Replaces the real claude binary in
//! integration tests via the `CODEBUS_CLAUDE_BIN` env override hook on
//! `agent::claude_cli::invoke`. Behavior is controlled by env vars so a
//! single mock binary covers multiple test scenarios.
//!
//! Env contract:
//!   CODEBUS_MOCK_LOG       — path to write a structured args+cwd dump
//!                            (one `key=value` line per arg / cwd field);
//!                            unset → don't write log
//!   CODEBUS_MOCK_BEHAVIOR  — one of:
//!     `success-noop`              (default) exit 0 without touching files
//!     `success-write-page`        write `wiki/concepts/test.md` then exit 0
//!     `failure-write-then-exit-1` write `wiki/concepts/partial.md` then exit 1
//!     `success-stream-json`       emit canonical 5-line stream-json + exit 0
//!     `failure-stream-json`       emit partial stream + exit 1
//!     `success-with-stderr-denial` emit success stream on stdout + a
//!                                 sandbox-denial marker on stderr + exit 0
//!     `chat-init-success`         emit init(session_id) + simple result, exit 0
//!     `chat-emit-promote`         emit init + assistant text with marker + result
//!     `chat-multi-tool`           emit init + 2 tool_use events + result
//!   CODEBUS_MOCK_SESSION_ID  — session_id value for `chat-*` behaviors
//!                              (defaults to `mock-session-0001` when unset)
//!
//! Working directory at invocation time is whatever the parent `Command`
//! set via `current_dir()` — for goal verb integration tests this is the
//! `.codebus/` vault root, so the relative `wiki/concepts/...` writes land
//! in the test's temp vault.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let log_path = env::var("CODEBUS_MOCK_LOG").ok();
    let behavior = env::var("CODEBUS_MOCK_BEHAVIOR").unwrap_or_else(|_| "success-noop".to_string());
    let args: Vec<String> = env::args().skip(1).collect();
    let cwd: PathBuf = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    if let Some(path) = log_path.as_deref() {
        let mut log = String::new();
        log.push_str(&format!("cwd={}\n", cwd.display()));
        for a in &args {
            log.push_str(&format!("arg={a}\n"));
        }
        // Dump the env vars codebus is expected to scope-inject for the
        // azure profile. `claude-code-endpoint-profiles` change uses this
        // to assert `Command::envs` actually carries the 3 vars without
        // leaking to the parent shell. Missing var → omit line.
        for key in [
            "ANTHROPIC_BASE_URL",
            "ANTHROPIC_API_KEY",
            "CLAUDE_CODE_DISABLE_ADVISOR_TOOL",
        ] {
            if let Ok(v) = env::var(key) {
                log.push_str(&format!("env_{key}={v}\n"));
            }
        }
        let _ = fs::write(path, log);
    }

    match behavior.as_str() {
        "success-noop" => ExitCode::SUCCESS,

        // run-outcome-lifecycle-integrity: sleep far longer than any test's
        // configured `lifecycle.run_timeout_secs` so the per-run wall-clock
        // watcher terminates the process tree first. The ExitCode below is
        // never reached when the timeout fires (the tree is killed).
        "hang" => {
            std::thread::sleep(std::time::Duration::from_secs(30));
            ExitCode::SUCCESS
        }

        "success-write-page" => {
            if write_test_page("wiki/concepts/test.md", "test").is_err() {
                return ExitCode::from(2);
            }
            ExitCode::SUCCESS
        }

        "failure-write-then-exit-1" => {
            // Write a page (partial work) then exit non-zero so the test can
            // assert the codebus side commits the partial snapshot anyway
            // (v2 carry: commit on failure).
            let _ = write_test_page("wiki/concepts/partial.md", "partial");
            ExitCode::from(1)
        }

        // v3-run-log: emit 5 stream-json lines covering the full event taxonomy
        // (system → assistant text → assistant tool_use → user tool_result →
        // result with usage). Lets integration tests verify the parse + render
        // + RunLog-write pipeline end-to-end against a deterministic stream.
        "success-stream-json" => {
            emit_stream_json_success();
            ExitCode::SUCCESS
        }

        // Same as success-stream-json but truncated mid-flow (no result event)
        // and exits non-zero. Tests assert that the verb still writes a RunLog
        // entry with zero tokens (Usage event was never emitted).
        "failure-stream-json" => {
            emit_stream_json_partial();
            ExitCode::from(1)
        }

        // agent-run-integrity (vertical A): emit a normal successful
        // stream-json run on STDOUT (so the verb derives outcome=succeeded)
        // while printing a curated sandbox-denial marker on STDERR and
        // exiting 0. Models the codex "top-level exit 0 but an inner command
        // was blocked, surfacing only on stderr" case. The test asserts the
        // resulting RunLog carries sandbox_denial_count > 0 with
        // outcome=succeeded, and that codebus emits a `warning: sandbox-denial`
        // line.
        "success-with-stderr-denial" => {
            // A curated locale-independent marker (see
            // codebus-core stream::sandbox_signal::DENIAL_MARKERS).
            eprintln!("Set-Content : Access is denied.");
            emit_stream_json_success();
            ExitCode::SUCCESS
        }

        // v3-chat-verb: emit `{type:system,subtype:init,session_id:...}`
        // as the first line so chat-verb's `sniff_init_session_id` populates
        // `InvokeReport.session_id` deterministically. Followed by a tiny
        // assistant text + result.
        "chat-init-success" => {
            let sid = session_id();
            println!(
                r#"{{"type":"system","subtype":"init","session_id":"{sid}","tools":["Read","Glob","Grep"]}}"#
            );
            println!(
                r#"{{"type":"assistant","message":{{"content":[{{"type":"text","text":"hello back"}}]}}}}"#
            );
            println!(
                r#"{{"type":"result","usage":{{"input_tokens":5,"output_tokens":3}}}}"#
            );
            ExitCode::SUCCESS
        }

        // v3-chat-verb: same as chat-init-success but the assistant text
        // begins with the promote-suggestion line marker so chat-verb emits
        // a `VerbLifecycleEvent::PromoteSuggestion { reason }` event.
        "chat-emit-promote" => {
            let sid = session_id();
            println!(
                r#"{{"type":"system","subtype":"init","session_id":"{sid}","tools":["Read","Glob","Grep"]}}"#
            );
            println!(
                r#"{{"type":"assistant","message":{{"content":[{{"type":"text","text":"[CODEBUS_PROMOTE_SUGGESTION] mock topic worth promoting\n\nBody of the response."}}]}}}}"#
            );
            println!(
                r#"{{"type":"result","usage":{{"input_tokens":5,"output_tokens":3}}}}"#
            );
            ExitCode::SUCCESS
        }

        // v3-chat-verb: emit init + two tool_use events so chat-verb's
        // activity stream render prints two `→ Tool ...` lines.
        "chat-multi-tool" => {
            let sid = session_id();
            println!(
                r#"{{"type":"system","subtype":"init","session_id":"{sid}","tools":["Read","Glob","Grep"]}}"#
            );
            println!(
                r#"{{"type":"assistant","message":{{"content":[{{"type":"tool_use","name":"Glob","input":{{"pattern":"wiki/modules/*.md"}}}}]}}}}"#
            );
            println!(
                r#"{{"type":"user","message":{{"content":[{{"type":"tool_result","content":"results","is_error":false}}]}}}}"#
            );
            println!(
                r#"{{"type":"assistant","message":{{"content":[{{"type":"tool_use","name":"Read","input":{{"file_path":"wiki/modules/uv-lib.md"}}}}]}}}}"#
            );
            println!(
                r#"{{"type":"user","message":{{"content":[{{"type":"tool_result","content":"file contents","is_error":false}}]}}}}"#
            );
            println!(
                r#"{{"type":"assistant","message":{{"content":[{{"type":"text","text":"summary"}}]}}}}"#
            );
            println!(
                r#"{{"type":"result","usage":{{"input_tokens":10,"output_tokens":4}}}}"#
            );
            ExitCode::SUCCESS
        }

        // v3-chat-verb cancel test: emit init (flushed) so the parent can
        // capture session_id, then trickle further events with deliberate
        // sleeps between each so a caller-flipped cancel flag fires AFTER
        // the init line is processed but BEFORE the final result line.
        // Each emit is explicitly flushed so piped stdout buffering does
        // not defeat the timing window.
        "chat-trickle-cancel" => {
            let sid = session_id();
            emit_flushed(&format!(
                r#"{{"type":"system","subtype":"init","session_id":"{sid}","tools":["Read","Glob","Grep"]}}"#
            ));
            // Sleep so the cancel-flipping test thread has time to flip
            // the AtomicBool while the parent is blocked reading the
            // next line from the pipe.
            std::thread::sleep(std::time::Duration::from_millis(800));
            emit_flushed(
                r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"x"}}]}}"#,
            );
            std::thread::sleep(std::time::Duration::from_millis(800));
            emit_flushed(
                r#"{"type":"result","usage":{"input_tokens":1,"output_tokens":1}}"#,
            );
            ExitCode::SUCCESS
        }

        // v3-app-quiz: two-shot quiz. The behavior inspects the prompt
        // arg to tell the plan spawn (`/codebus-quiz plan:`) from the
        // generate spawn (`/codebus-quiz generate:`) since codebus spawns
        // claude once per phase with the same CODEBUS_MOCK_BEHAVIOR.
        "quiz-goal-match" => {
            let sid = session_id();
            match quiz_mode(&args) {
                QuizMode::Plan => {
                    emit_quiz_init(&sid);
                    emit_assistant_text(
                        "[CODEBUS_QUIZ_SCOPE] wiki/concepts/jwt-token-lifecycle.md, \
                         wiki/modules/auth-middleware.md",
                    );
                    emit_quiz_result();
                }
                QuizMode::Generate | QuizMode::Verify | QuizMode::Unknown => {
                    emit_quiz_init(&sid);
                    emit_assistant_text(MOCK_QUIZ_BODY);
                    emit_quiz_result();
                }
            }
            ExitCode::SUCCESS
        }

        // Plan spawn emits a no-match marker; run_quiz then performs no
        // generate spawn, so only the plan invocation is ever made.
        "quiz-no-match" => {
            let sid = session_id();
            emit_quiz_init(&sid);
            emit_assistant_text("[CODEBUS_QUIZ_NO_MATCH] mock: vault does not cover that topic");
            emit_quiz_result();
            ExitCode::SUCCESS
        }

        // Plan returns scope (like quiz-goal-match); generate wraps the
        // body in a ```markdown fence so the caller's tolerant fence
        // strip can be asserted end-to-end.
        "quiz-fenced" => {
            let sid = session_id();
            match quiz_mode(&args) {
                QuizMode::Plan => {
                    emit_quiz_init(&sid);
                    emit_assistant_text(
                        "[CODEBUS_QUIZ_SCOPE] wiki/concepts/jwt-token-lifecycle.md, \
                         wiki/modules/auth-middleware.md",
                    );
                    emit_quiz_result();
                }
                QuizMode::Generate | QuizMode::Verify | QuizMode::Unknown => {
                    emit_quiz_init(&sid);
                    // Use literal `\n` (JSON escape), not a raw newline —
                    // a raw newline inside the JSON string is invalid and
                    // the stream parser would skip the whole event.
                    emit_assistant_text(&format!("```markdown\\n{MOCK_QUIZ_BODY}\\n```"));
                    emit_quiz_result();
                }
            }
            ExitCode::SUCCESS
        }

        // quiz-validate-repair: generate emits a structurally valid body
        // with NO `[[wikilink]]` citations (a fresh-init vault has no
        // content pages, so any citation would be flagged broken). The
        // caller final-verify SHALL find zero issues → `validation: ok`.
        "quiz-clean-body" => {
            let sid = session_id();
            match quiz_mode(&args) {
                QuizMode::Plan => {
                    emit_quiz_init(&sid);
                    emit_assistant_text(
                        "[CODEBUS_QUIZ_SCOPE] wiki/concepts/jwt-token-lifecycle.md",
                    );
                    emit_quiz_result();
                }
                QuizMode::Generate | QuizMode::Verify | QuizMode::Unknown => {
                    emit_quiz_init(&sid);
                    emit_assistant_text(MOCK_QUIZ_CLEAN);
                    emit_quiz_result();
                }
            }
            ExitCode::SUCCESS
        }

        // quiz-validate-repair: generate emits a body whose only
        // question is missing its `## Answer:` line. Caller final-verify
        // SHALL produce a schema finding → quiz still persisted
        // best-effort with `validation: failed`, a non-fatal warning is
        // surfaced, the question is NOT dropped, verb still exits 0.
        "quiz-bad-body" => {
            let sid = session_id();
            match quiz_mode(&args) {
                QuizMode::Plan => {
                    emit_quiz_init(&sid);
                    emit_assistant_text(
                        "[CODEBUS_QUIZ_SCOPE] wiki/concepts/jwt-token-lifecycle.md",
                    );
                    emit_quiz_result();
                }
                QuizMode::Generate | QuizMode::Verify | QuizMode::Unknown => {
                    emit_quiz_init(&sid);
                    emit_assistant_text(MOCK_QUIZ_BAD);
                    emit_quiz_result();
                }
            }
            ExitCode::SUCCESS
        }

        // quiz-content-verify: plan→scope, generate→clean body, and the
        // independent verify spawn always reports CONTENT_OK → the
        // caller verify→repair loop exits immediately with
        // `content_review: ok`.
        "quiz-verify-clean" => {
            let sid = session_id();
            match quiz_mode(&args) {
                QuizMode::Plan => {
                    emit_quiz_init(&sid);
                    emit_assistant_text(
                        "[CODEBUS_QUIZ_SCOPE] wiki/concepts/jwt-token-lifecycle.md",
                    );
                    emit_quiz_result();
                }
                QuizMode::Verify => {
                    emit_quiz_init(&sid);
                    emit_assistant_text("CONTENT_OK");
                    emit_quiz_result();
                }
                QuizMode::Generate | QuizMode::Unknown => {
                    emit_quiz_init(&sid);
                    emit_assistant_text(MOCK_QUIZ_CLEAN);
                    emit_quiz_result();
                }
            }
            ExitCode::SUCCESS
        }

        // quiz-content-verify: the verify spawn ALWAYS flags Q1, so the
        // caller loop repairs (generate again) then re-verifies and is
        // flagged again, exhausting the cap → best-effort persist with
        // `content_review: flagged` listing Q1 + a non-fatal warning,
        // questions not dropped, exit 0.
        "quiz-verify-flag" => {
            let sid = session_id();
            match quiz_mode(&args) {
                QuizMode::Plan => {
                    emit_quiz_init(&sid);
                    emit_assistant_text(
                        "[CODEBUS_QUIZ_SCOPE] wiki/concepts/jwt-token-lifecycle.md",
                    );
                    emit_quiz_result();
                }
                QuizMode::Verify => {
                    emit_quiz_init(&sid);
                    emit_assistant_text(
                        "Q1 | answer-wrong | the marked option is not supported by the planned pages; pick the supported one",
                    );
                    emit_quiz_result();
                }
                QuizMode::Generate | QuizMode::Unknown => {
                    emit_quiz_init(&sid);
                    emit_assistant_text(MOCK_QUIZ_CLEAN);
                    emit_quiz_result();
                }
            }
            ExitCode::SUCCESS
        }

        // goal-content-verify task 4.1: goal ingest writes a wiki page;
        // the independent verify spawn always reports CONTENT_OK → the
        // shared verify→repair loop exits immediately with content-review
        // ok; auto_commit runs normally.
        "goal-verify-clean" => match goal_mode(&args) {
            GoalMode::Verify => {
                emit_quiz_init(&session_id());
                emit_assistant_text("CONTENT_OK");
                emit_quiz_result();
                ExitCode::SUCCESS
            }
            GoalMode::Repair => ExitCode::SUCCESS,
            GoalMode::Ingest | GoalMode::Unknown => {
                let _ = write_test_page("wiki/concepts/mock.md", "mock");
                ExitCode::SUCCESS
            }
        },

        // goal-content-verify task 4.1: verify ALWAYS flags the changed
        // page; the loop repairs (Write spawn rewrites the page) then
        // re-verifies and is flagged again, exhausting the cap → residual
        // content_review: flagged + a non-fatal warning, the page is NOT
        // reverted, exit unchanged, auto_commit still runs.
        "goal-verify-flag" => match goal_mode(&args) {
            GoalMode::Verify => {
                emit_quiz_init(&session_id());
                emit_assistant_text(
                    "wiki/concepts/mock.md | unfaithful | claim not grounded in raw/code",
                );
                emit_quiz_result();
                ExitCode::SUCCESS
            }
            GoalMode::Repair => {
                // Repair spawn is Write-capable: rewrite the flagged page
                // in place (still "flagged" on re-verify by design of
                // this mock, to exercise the cap path).
                let _ = write_test_page("wiki/concepts/mock.md", "mock-repaired");
                ExitCode::SUCCESS
            }
            GoalMode::Ingest | GoalMode::Unknown => {
                let _ = write_test_page("wiki/concepts/mock.md", "mock");
                ExitCode::SUCCESS
            }
        },

        // goal-content-verify task 4.1: verify emits prose with neither
        // CONTENT_OK nor a parseable defect line → unparseable → the
        // stage is conservatively flagged (never silently ok), non-fatal.
        "goal-verify-unparseable" => match goal_mode(&args) {
            GoalMode::Verify => {
                emit_quiz_init(&session_id());
                emit_assistant_text("I looked at the pages and they seem fine overall.");
                emit_quiz_result();
                ExitCode::SUCCESS
            }
            GoalMode::Repair => ExitCode::SUCCESS,
            GoalMode::Ingest | GoalMode::Unknown => {
                let _ = write_test_page("wiki/concepts/mock.md", "mock");
                ExitCode::SUCCESS
            }
        },

        other => {
            eprintln!("mock-claude: unknown behavior `{other}`");
            ExitCode::from(2)
        }
    }
}

#[derive(PartialEq)]
enum GoalMode {
    Ingest,
    Verify,
    Repair,
    Unknown,
}

/// Classify a goal spawn by scanning argv for the `/codebus-goal` slash
/// command's mode prefix (goal-content-verify design D3/D6).
fn goal_mode(args: &[String]) -> GoalMode {
    let joined = args.join(" ");
    if joined.contains("/codebus-goal verify:") {
        GoalMode::Verify
    } else if joined.contains("/codebus-goal repair:") {
        GoalMode::Repair
    } else if joined.contains("/codebus-goal ") {
        GoalMode::Ingest
    } else {
        GoalMode::Unknown
    }
}

/// Structurally valid, citation-free quiz body for the
/// `validation: ok` path.
const MOCK_QUIZ_CLEAN: &str = "## Q1. What does the validator check?\\n\\n- A) nothing\\n- B) schema and wikilink existence\\n- C) the network\\n- D) the model\\n\\n## Answer: B\\n\\n## Explanation: The deterministic validator checks question schema and citation existence.";

/// Malformed body — the single question has no `## Answer:` line — for
/// the residual-failure best-effort `validation: failed` path.
const MOCK_QUIZ_BAD: &str = "## Q1. What is missing from this question?\\n\\n- A) the stem\\n- B) the choices\\n- C) the answer line\\n- D) nothing\\n\\n## Explanation: This block intentionally omits the Answer line.";

/// One well-formed quiz question body (no frontmatter, no fence) — the
/// post-D4 shape the agent is instructed to emit. Integration tests
/// assert the caller flow / persistence / exit code, not LLM question
/// quality, so a single question suffices.
const MOCK_QUIZ_BODY: &str = "## Q1. What does the quiz integration mock validate?\\n\\n- A) the language model\\n- B) the caller two-shot flow and persistence\\n- C) the network stack\\n- D) nothing\\n\\n## Answer: B\\n\\n## Explanation: The mock pins the caller plan/generate flow and frontmatter injection, see [[auth-middleware]].";

#[derive(PartialEq)]
enum QuizMode {
    Plan,
    Generate,
    Verify,
    Unknown,
}

/// Classify the spawn by scanning argv for the `/codebus-quiz` slash
/// command's mode prefix.
fn quiz_mode(args: &[String]) -> QuizMode {
    let joined = args.join(" ");
    if joined.contains("/codebus-quiz plan:") {
        QuizMode::Plan
    } else if joined.contains("/codebus-quiz verify:") {
        QuizMode::Verify
    } else if joined.contains("/codebus-quiz generate:") {
        QuizMode::Generate
    } else {
        QuizMode::Unknown
    }
}

fn emit_quiz_init(sid: &str) {
    println!(
        r#"{{"type":"system","subtype":"init","session_id":"{sid}","tools":["Read","Glob","Grep"]}}"#
    );
}

fn emit_assistant_text(text: &str) {
    println!(
        r#"{{"type":"assistant","message":{{"content":[{{"type":"text","text":"{text}"}}]}}}}"#
    );
}

fn emit_quiz_result() {
    println!(r#"{{"type":"result","usage":{{"input_tokens":7,"output_tokens":4}}}}"#);
}

fn session_id() -> String {
    env::var("CODEBUS_MOCK_SESSION_ID").unwrap_or_else(|_| "mock-session-0001".to_string())
}

/// Print one line to stdout and flush immediately. Used by trickle
/// behaviors so the parent (which reads via `BufReader::lines()` on a
/// piped child stdout) sees each line as it's emitted, not at process
/// exit. Default `println!` is fine for one-shot dumps but unreliable
/// when the test cares about between-line timing.
fn emit_flushed(line: &str) {
    use std::io::Write;
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    let _ = writeln!(lock, "{line}");
    let _ = lock.flush();
}

fn emit_stream_json_success() {
    println!(r#"{{"type":"system","subtype":"init"}}"#);
    println!(
        r#"{{"type":"assistant","message":{{"content":[{{"type":"text","text":"思考中..."}}]}}}}"#
    );
    println!(
        r#"{{"type":"assistant","message":{{"content":[{{"type":"tool_use","name":"Read","input":{{"file_path":"/x"}}}}]}}}}"#
    );
    println!(
        r#"{{"type":"user","message":{{"content":[{{"type":"tool_result","content":"file contents","is_error":false}}]}}}}"#
    );
    println!(
        r#"{{"type":"result","usage":{{"input_tokens":100,"output_tokens":50,"cache_read_input_tokens":10,"cache_creation_input_tokens":5}}}}"#
    );
}

fn emit_stream_json_partial() {
    println!(r#"{{"type":"system","subtype":"init"}}"#);
    println!(
        r#"{{"type":"assistant","message":{{"content":[{{"type":"text","text":"about to fail"}}]}}}}"#
    );
    // No result event → no Usage → RunLog tokens stay zero.
}

fn write_test_page(rel_path: &str, name: &str) -> std::io::Result<()> {
    let path = PathBuf::from(rel_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let body = format!("---\nname: {name}\n---\n\nbody from mock-claude\n");
    fs::write(&path, body)
}
