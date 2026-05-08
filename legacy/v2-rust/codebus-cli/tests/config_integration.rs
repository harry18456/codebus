//! Integration test for `~/.codebus/config.yaml` end-to-end.
//!
//! Spawns the built `codebus` binary with `HOME` (and `USERPROFILE` on
//! Windows, since `dirs` crate uses that) pointed at a tmp dir containing
//! a hand-rolled `~/.codebus/config.yaml`, runs `--check` against an
//! empty vault, and asserts the emoji state in stdout reflects the
//! config-pinned setting.
//!
//! Implements task 6.6: "寫 `emoji: off` 到 tmp config.yaml、設 `HOME`
//! env 指過去、跑 codebus、stdout 不含 emoji".

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn nanos() -> u32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .subsec_nanos()
}

fn fresh_tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "codebus-cfg-int-{name}-{}-{}",
        std::process::id(),
        nanos()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn make_empty_vault(root: &std::path::Path) {
    let wiki = root.join(".codebus").join("wiki");
    fs::create_dir_all(&wiki).unwrap();
    fs::write(wiki.join("index.md"), "# index\n").unwrap();
    fs::write(wiki.join("log.md"), "# log\n").unwrap();
    for f in ["concepts", "entities", "modules", "processes", "synthesis"] {
        fs::create_dir_all(wiki.join(f)).unwrap();
    }
}

fn run_check_with_home(home: &std::path::Path, vault: &std::path::Path) -> String {
    let bin = env!("CARGO_BIN_EXE_codebus");
    let output = Command::new(bin)
        .arg("--repo")
        .arg(vault)
        .arg("--check")
        // `dirs::home_dir()` honors HOME on unix but ignores HOME/USERPROFILE
        // on Windows (it goes through `SHGetKnownFolderPath`). `CODEBUS_HOME`
        // is the cross-platform override the loader checks before `dirs`.
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("CODEBUS_HOME", home)
        .env_remove("NO_EMOJI")
        .output()
        .expect("codebus.exe failed to spawn");
    assert!(
        output.status.success(),
        "codebus exited with {}: stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stderr),
    );
    String::from_utf8(output.stdout).expect("stdout is utf-8")
}

#[test]
fn config_emoji_off_suppresses_emoji_in_check_output() {
    let home = fresh_tmp("emoji-off-home");
    let vault = fresh_tmp("emoji-off-vault");
    make_empty_vault(&vault);

    fs::create_dir_all(home.join(".codebus")).unwrap();
    fs::write(home.join(".codebus/config.yaml"), "emoji: off\n").unwrap();

    let out = run_check_with_home(&home, &vault);
    // Empty vault prints "ok 0 pages + 2 nav files scanned, no issues"
    // when emoji is OFF (ASCII glyph). When ON, the leading glyph is "✅".
    assert!(
        out.starts_with("ok "),
        "expected ASCII 'ok' prefix when config.yaml `emoji: off`, got: {out:?}"
    );
    assert!(
        !out.contains('✅'),
        "expected no emoji glyph in output, got: {out:?}"
    );

    let _ = fs::remove_dir_all(&home);
    let _ = fs::remove_dir_all(&vault);
}

#[test]
fn full_config_with_all_five_plugin_sections_runs_without_error() {
    // R8 task 8.4 — write a config.yaml that exercises every recognized
    // top-level key + every plugin section discriminator and confirm the
    // binary still runs `--check` cleanly (which exercises load_config()
    // at startup; if any section parse panicked, the binary would exit
    // non-zero and the assertion in run_check_with_home would fire).
    let home = fresh_tmp("fullcfg-home");
    let vault = fresh_tmp("fullcfg-vault");
    make_empty_vault(&vault);

    fs::create_dir_all(home.join(".codebus")).unwrap();
    let body = r#"
emoji: off
llm:
  provider: claude_cli
  binary_path: claude
  timeout_secs: 1800
pii:
  scanner: regex_basic
  on_hit: warn
  patterns_extra:
    - 'INTERNAL-\d{6}'
lint:
  disabled_rules: []
render:
  format: terminal
log:
  sink: jsonl
  retention_days: 30
"#;
    fs::write(home.join(".codebus/config.yaml"), body).unwrap();

    let out = run_check_with_home(&home, &vault);
    // Empty vault → "ok 0 pages + 2 nav files scanned, no issues" when
    // config-pinned `emoji: off` is honored AND every section parsed
    // without aborting.
    assert!(
        out.starts_with("ok "),
        "expected ASCII 'ok' prefix, got: {out:?}"
    );

    let _ = fs::remove_dir_all(&home);
    let _ = fs::remove_dir_all(&vault);
}

#[test]
fn config_emoji_on_emits_emoji_in_check_output() {
    let home = fresh_tmp("emoji-on-home");
    let vault = fresh_tmp("emoji-on-vault");
    make_empty_vault(&vault);

    fs::create_dir_all(home.join(".codebus")).unwrap();
    fs::write(home.join(".codebus/config.yaml"), "emoji: on\n").unwrap();

    let out = run_check_with_home(&home, &vault);
    assert!(
        out.contains('✅'),
        "expected emoji glyph when config.yaml `emoji: on`, got: {out:?}"
    );

    let _ = fs::remove_dir_all(&home);
    let _ = fs::remove_dir_all(&vault);
}
