//! Integration test for multi-vault `codebus mcp` (registry mode) plus the
//! pinned-mode P1 guard. Spawns the real stdio server with `CODEBUS_HOME`
//! pointed at a temp registry (`app-state.json`) and drives it with bare
//! newline-delimited JSON-RPC (no rmcp client dependency).
//!
//! Covers the `mcp-server` capability's multi-vault behavior:
//! - registry-mode startup (no `--vault`) and `vault_list` shape;
//! - `Vault selection across startup modes` — omit `vault` on wiki_list /
//!   wiki_search aggregates across ALL present vaults (tagging each result with
//!   its source vault), while wiki_read requires an explicit vault when more
//!   than one is present;
//! - `Read-only security boundary` — out-of-registry vault rejected, the
//!   registry is never written, `raw/code/` unreachable, aggregation stays
//!   inside the registry;
//! - `wiki_search` global cap across vaults;
//! - registry re-read per call (a vault added mid-session becomes visible);
//! - pinned mode rejects a mismatched `vault` (fail-loud).

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};
use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_codebus");

/// Create a vault dir under `parent` with the given wiki pages (`(slug, body)`).
fn make_vault(parent: &Path, name: &str, pages: &[(&str, &str)]) -> PathBuf {
    let vault = parent.join(name);
    let wiki = vault.join(".codebus").join("wiki");
    std::fs::create_dir_all(&wiki).unwrap();
    for (slug, body) in pages {
        std::fs::write(wiki.join(format!("{slug}.md")), body).unwrap();
    }
    vault
}

fn app_state_file(home: &Path) -> PathBuf {
    home.join(".codebus").join("app-state.json")
}

/// Write the app-state registry at `<home>/.codebus/app-state.json` with the
/// given `(vault_path, display_name)` entries.
fn write_registry(home: &Path, vaults: &[(&Path, &str)]) {
    let entries: Vec<Value> = vaults
        .iter()
        .map(|(p, name)| {
            json!({
                "path": p.display().to_string(),
                "display_name": name,
                "last_opened": "2026-06-27T00:00:00Z",
            })
        })
        .collect();
    let state = json!({ "schema_version": 1, "vault_list": entries });
    let path = app_state_file(home);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, serde_json::to_string_pretty(&state).unwrap()).unwrap();
}

struct McpClient {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl McpClient {
    fn spawn(cmd: &mut Command) -> Self {
        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn `codebus mcp`");
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        McpClient { child, stdin, stdout }
    }

    fn spawn_registry(home: &Path) -> Self {
        let mut cmd = Command::new(BIN);
        cmd.arg("mcp").env("CODEBUS_HOME", home);
        Self::spawn(&mut cmd)
    }

    fn spawn_pinned(vault: &Path) -> Self {
        let mut cmd = Command::new(BIN);
        cmd.args(["mcp", "--vault"]).arg(vault);
        Self::spawn(&mut cmd)
    }

    fn send(&mut self, msg: &Value) {
        let line = serde_json::to_string(msg).unwrap();
        self.stdin.write_all(line.as_bytes()).unwrap();
        self.stdin.write_all(b"\n").unwrap();
        self.stdin.flush().unwrap();
    }

    fn recv(&mut self) -> Value {
        let mut line = String::new();
        let n = self.stdout.read_line(&mut line).expect("read response line");
        assert!(n > 0, "server closed stdout before responding");
        serde_json::from_str(line.trim()).unwrap_or_else(|e| panic!("bad json {line:?}: {e}"))
    }

    fn request(&mut self, id: i64, method: &str, params: Value) -> Value {
        self.send(&json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}));
        self.recv()
    }

    fn initialize(&mut self) {
        self.request(
            1,
            "initialize",
            json!({"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"it","version":"0"}}),
        );
        self.send(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
    }

    fn call_raw(&mut self, id: i64, name: &str, arguments: Value) -> Value {
        self.request(id, "tools/call", json!({"name": name, "arguments": arguments}))
    }

    fn call_ok(&mut self, id: i64, name: &str, arguments: Value) -> Value {
        let resp = self.call_raw(id, name, arguments);
        assert!(resp.get("error").is_none(), "tool {name} protocol error: {resp}");
        assert_ne!(resp["result"]["isError"], json!(true), "tool {name} isError: {resp}");
        let text = resp["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_else(|| panic!("tool {name} has no text content: {resp}"));
        serde_json::from_str(text).unwrap_or_else(|e| panic!("tool {name} text not json: {text}: {e}"))
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn is_error(resp: &Value) -> bool {
    resp.get("error").is_some() || resp["result"]["isError"] == json!(true)
}

#[test]
fn registry_mode_aggregates_addresses_and_guards() {
    let home = TempDir::new().unwrap();
    let vaults = TempDir::new().unwrap();
    let a = make_vault(
        vaults.path(),
        "alpha",
        &[
            ("auth", "---\ntitle: Alpha Auth\n---\nThe authentication flow for alpha.\n"),
            ("index", "---\ntitle: Alpha Index\n---\nAlpha home.\n"),
        ],
    );
    let b = make_vault(
        vaults.path(),
        "beta",
        &[
            ("deploy", "---\ntitle: Beta Deploy\n---\nBeta authentication during deploy.\n"),
            ("guide", "---\ntitle: Beta Guide\n---\nBeta guide.\n"),
        ],
    );
    // A secret OUTSIDE alpha's wiki subtree (mimics the redacted code mirror).
    std::fs::create_dir_all(a.join(".codebus").join("raw").join("code")).unwrap();
    std::fs::write(
        a.join(".codebus").join("raw").join("code").join("secret.md"),
        "SECRET_TOKEN_DO_NOT_LEAK\n",
    )
    .unwrap();

    write_registry(home.path(), &[(a.as_path(), "alpha"), (b.as_path(), "beta")]);

    let mut c = McpClient::spawn_registry(home.path());
    c.initialize();

    // tools/list → four tools.
    let list = c.request(2, "tools/list", json!({}));
    let names: Vec<&str> = list["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(names.len(), 4, "four tools in registry mode: {names:?}");
    for t in ["vault_list", "wiki_list", "wiki_read", "wiki_search"] {
        assert!(names.contains(&t), "missing {t}: {names:?}");
    }

    // vault_list → both vaults with vault id + name.
    let vl = c.call_ok(3, "vault_list", json!({}));
    let entries = vl.as_array().unwrap();
    assert_eq!(entries.len(), 2, "two registered present vaults: {vl}");
    let ids: Vec<&str> = entries.iter().map(|e| e["vault"].as_str().unwrap()).collect();
    let a_id = ids.iter().find(|s| s.contains("alpha")).unwrap().to_string();
    let b_id = ids.iter().find(|s| s.contains("beta")).unwrap().to_string();
    assert!(entries.iter().any(|e| e["name"] == json!("alpha")));
    assert!(entries.iter().any(|e| e["name"] == json!("beta")));

    // wiki_list omit → aggregate across both, each tagged with its source vault.
    let listed = c.call_ok(4, "wiki_list", json!({}));
    let pages = listed.as_array().unwrap();
    assert_eq!(pages.len(), 4, "4 pages aggregated across alpha+beta: {listed}");
    for p in pages {
        let v = p["vault"].as_str().expect("each entry tagged with source vault");
        assert!(v == a_id || v == b_id, "unexpected source vault {v}");
        assert!(p["name"].is_string(), "each entry tagged with source name: {p}");
    }
    assert!(!pages.iter().any(|p| p["slug"] == json!("secret")), "raw/code leaked into wiki_list");

    // wiki_search omit → hits from BOTH vaults, each tagged.
    let search = c.call_ok(5, "wiki_search", json!({"query": "authentication"}));
    let hits = search["results"].as_array().unwrap();
    assert_eq!(hits.len(), 2, "one hit per vault for 'authentication': {search}");
    let hit_vaults: Vec<&str> = hits.iter().map(|h| h["vault"].as_str().unwrap()).collect();
    assert!(hit_vaults.contains(&a_id.as_str()) && hit_vaults.contains(&b_id.as_str()));

    // wiki_read omit + multi present → error (needs an explicit vault).
    let amb = c.call_raw(6, "wiki_read", json!({"slug": "auth"}));
    assert!(is_error(&amb), "wiki_read with no vault + multi present must error: {amb}");

    // wiki_read with the source vault → reads that vault's page.
    let read = c.call_ok(7, "wiki_read", json!({"vault": a_id, "slug": "auth"}));
    assert_eq!(read["title"], json!("Alpha Auth"));
    assert!(read["content"].as_str().unwrap().contains("alpha"));

    // wiki_search scoped to one vault → only that vault's hits.
    let scoped = c.call_ok(8, "wiki_search", json!({"vault": b_id, "query": "authentication"}));
    let scoped_hits = scoped["results"].as_array().unwrap();
    assert_eq!(scoped_hits.len(), 1, "scoped to beta only: {scoped}");
    assert_eq!(scoped_hits[0]["vault"].as_str().unwrap(), b_id);

    // Whitelist: an out-of-registry path is rejected (not read).
    let outside = TempDir::new().unwrap();
    let bad = c.call_raw(9, "wiki_read", json!({"vault": outside.path().display().to_string(), "slug": "auth"}));
    assert!(is_error(&bad), "out-of-registry vault must be rejected: {bad}");
    let bad_search = c.call_raw(10, "wiki_search", json!({"vault": outside.path().display().to_string(), "query": "x"}));
    assert!(is_error(&bad_search), "out-of-registry vault must be rejected for search: {bad_search}");

    // raw/code unreachable: not searchable and not readable even via its vault.
    let secret_search = c.call_ok(11, "wiki_search", json!({"query": "SECRET_TOKEN"}));
    assert!(secret_search["results"].as_array().unwrap().is_empty(), "raw/code content searchable: {secret_search}");
    let secret_read = c.call_raw(12, "wiki_read", json!({"vault": a_id, "slug": "secret"}));
    assert!(is_error(&secret_read), "raw/code page must be unreachable: {secret_read}");

    // The server never wrote the registry (read-only boundary).
    let on_disk = std::fs::read_to_string(app_state_file(home.path())).unwrap();
    assert!(on_disk.contains("alpha") && on_disk.contains("beta"));
    assert!(!on_disk.contains("schema_version\": 2"), "registry must be unchanged");
}

#[test]
fn registry_mode_single_vault_defaults_on_omission() {
    let home = TempDir::new().unwrap();
    let vaults = TempDir::new().unwrap();
    let only = make_vault(vaults.path(), "solo", &[("readme", "---\ntitle: Solo\n---\nThe only vault.\n")]);
    write_registry(home.path(), &[(only.as_path(), "solo")]);

    let mut c = McpClient::spawn_registry(home.path());
    c.initialize();

    // Single present vault → omit resolves to it for read too.
    let read = c.call_ok(2, "wiki_read", json!({"slug": "readme"}));
    assert_eq!(read["title"], json!("Solo"));
    let listed = c.call_ok(3, "wiki_list", json!({}));
    assert_eq!(listed.as_array().unwrap().len(), 1);
}

#[test]
fn registry_mode_empty_registry_errors_and_creates_no_file() {
    let home = TempDir::new().unwrap();
    // No app-state.json written at all.
    let mut c = McpClient::spawn_registry(home.path());
    c.initialize();

    let vl = c.call_ok(2, "vault_list", json!({}));
    assert!(vl.as_array().unwrap().is_empty(), "empty registry → empty vault_list: {vl}");

    let listed = c.call_raw(3, "wiki_list", json!({}));
    assert!(is_error(&listed), "wiki_list with no registered vault must error: {listed}");

    // Read-only: the server must NOT have created app-state.json.
    assert!(
        !app_state_file(home.path()).exists(),
        "registry-mode server must not create app-state.json"
    );
}

#[test]
fn registry_mode_sees_newly_added_vault_without_restart() {
    let home = TempDir::new().unwrap();
    let vaults = TempDir::new().unwrap();
    let a = make_vault(vaults.path(), "first", &[("p", "---\ntitle: First\n---\nfirst.\n")]);
    write_registry(home.path(), &[(a.as_path(), "first")]);

    let mut c = McpClient::spawn_registry(home.path());
    c.initialize();

    let before = c.call_ok(2, "vault_list", json!({}));
    assert_eq!(before.as_array().unwrap().len(), 1);

    // Add a second vault to the registry while the server runs.
    let b = make_vault(vaults.path(), "second", &[("q", "---\ntitle: Second\n---\nsecond.\n")]);
    write_registry(home.path(), &[(a.as_path(), "first"), (b.as_path(), "second")]);

    // Per-call reread → the new vault is visible without a restart.
    let after = c.call_ok(3, "vault_list", json!({}));
    assert_eq!(after.as_array().unwrap().len(), 2, "newly added vault must appear: {after}");
}

#[test]
fn registry_mode_search_global_cap_across_vaults() {
    let home = TempDir::new().unwrap();
    let vaults = TempDir::new().unwrap();
    // Two vaults, 15 needle pages each → 30 total, over the global cap of 20.
    let mut a_pages: Vec<(String, String)> = Vec::new();
    let mut b_pages: Vec<(String, String)> = Vec::new();
    for i in 0..15 {
        a_pages.push((format!("a{i:02}"), "---\ntitle: A\n---\nthe needle is here\n".to_string()));
        b_pages.push((format!("b{i:02}"), "---\ntitle: B\n---\nthe needle is here\n".to_string()));
    }
    let a_refs: Vec<(&str, &str)> = a_pages.iter().map(|(s, b)| (s.as_str(), b.as_str())).collect();
    let b_refs: Vec<(&str, &str)> = b_pages.iter().map(|(s, b)| (s.as_str(), b.as_str())).collect();
    let a = make_vault(vaults.path(), "av", &a_refs);
    let b = make_vault(vaults.path(), "bv", &b_refs);
    write_registry(home.path(), &[(a.as_path(), "av"), (b.as_path(), "bv")]);

    let mut c = McpClient::spawn_registry(home.path());
    c.initialize();

    let search = c.call_ok(2, "wiki_search", json!({"query": "needle"}));
    let hits = search["results"].as_array().unwrap();
    assert_eq!(hits.len(), 20, "global cap is 20 across vaults: {}", hits.len());
    assert_eq!(search["truncated"], json!(true), "more than 20 matched → truncated");
}

#[test]
fn pinned_mode_rejects_mismatched_vault() {
    let tmp = TempDir::new().unwrap();
    let pinned = make_vault(tmp.path(), "pinned", &[("home", "---\ntitle: Pinned\n---\npinned body.\n")]);
    let other = TempDir::new().unwrap();

    let mut c = McpClient::spawn_pinned(&pinned);
    c.initialize();

    // Omitted vault → the pinned vault (v1 behavior, untagged result shape).
    let read = c.call_ok(2, "wiki_read", json!({"slug": "home"}));
    assert_eq!(read["title"], json!("Pinned"));

    // A mismatched vault → fail-loud (P1), not silently ignored.
    let mismatch = c.call_raw(3, "wiki_read", json!({"vault": other.path().display().to_string(), "slug": "home"}));
    assert!(is_error(&mismatch), "pinned mode must reject a mismatched vault: {mismatch}");
}
