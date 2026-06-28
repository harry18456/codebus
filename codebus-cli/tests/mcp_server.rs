//! Integration test for `codebus mcp` — spawn the stdio MCP server against a
//! real temp vault and drive it with bare newline-delimited JSON-RPC (no rmcp
//! client dependency, mirroring the Phase 0 spike's `mcp_verify` path).
//!
//! Pinned mode (`--vault`): covers initialize advertises tools-only; tools/list
//! returns the four query tools (vault_list + wiki_list / wiki_read /
//! wiki_search), where the wiki tools expose an optional `vault` selector and
//! no raw path arg; tools/call for wiki_list / wiki_read (with pagination) /
//! wiki_search against real data; unknown slug and blank query surface as
//! errors; and `raw/code/` is unreachable. Multi-vault registry behavior is
//! covered by `mcp_multi_vault.rs`.

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};
use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_codebus");

fn write_page(dir: &Path, rel: &str, content: &str) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, content).unwrap();
}

/// Minimal stdio MCP client: one request in, one response line out.
struct McpClient {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl McpClient {
    fn spawn(vault: &Path) -> Self {
        let mut child = Command::new(BIN)
            .args(["mcp", "--vault"])
            .arg(vault)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            // stderr carries server diagnostics only; keep it off the test log.
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn `codebus mcp`");
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        McpClient {
            child,
            stdin,
            stdout,
        }
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

    fn notify(&mut self, method: &str) {
        self.send(&json!({"jsonrpc":"2.0","method":method}));
    }

    /// Call a tool and return the raw JSON-RPC response (caller inspects for
    /// success vs error).
    fn call_raw(&mut self, id: i64, name: &str, arguments: Value) -> Value {
        self.request(id, "tools/call", json!({"name": name, "arguments": arguments}))
    }

    /// Call a tool expected to succeed, returning the parsed JSON payload from
    /// the first text content block.
    fn call_ok(&mut self, id: i64, name: &str, arguments: Value) -> Value {
        let resp = self.call_raw(id, name, arguments);
        assert!(
            resp.get("error").is_none(),
            "tool {name} returned protocol error: {resp}"
        );
        let result = &resp["result"];
        assert_ne!(
            result["isError"],
            json!(true),
            "tool {name} returned isError: {resp}"
        );
        let text = result["content"][0]["text"]
            .as_str()
            .unwrap_or_else(|| panic!("tool {name} response has no text content: {resp}"));
        serde_json::from_str(text).unwrap_or_else(|e| panic!("tool {name} text not json: {text}: {e}"))
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A response represents an error if it carries a JSON-RPC `error` OR a
/// tool result flagged `isError`.
fn is_error_response(resp: &Value) -> bool {
    resp.get("error").is_some() || resp["result"]["isError"] == json!(true)
}

#[test]
fn mcp_server_serves_wiki_over_stdio() {
    let tmp = TempDir::new().unwrap();
    let vault = tmp.path();
    let wiki = vault.join(".codebus").join("wiki");

    // Real pages: a CJK title, a searchable keyword, and a page large enough
    // to exercise pagination.
    write_page(
        &wiki,
        "index.md",
        "---\ntitle: 索引首頁\n---\nWelcome to the authentication guide.\n",
    );
    write_page(
        &wiki,
        "big.md",
        &format!("---\ntitle: Big Page\n---\n{}", "x".repeat(30_000)),
    );
    write_page(&wiki, "plain.md", "no frontmatter here\n");
    // A secret OUTSIDE the wiki subtree (mimics the redacted code mirror). The
    // server must never reach it.
    write_page(
        &vault.join(".codebus").join("raw").join("code"),
        "secret.md",
        "SECRET_TOKEN_DO_NOT_LEAK\n",
    );

    let mut client = McpClient::spawn(vault);

    // --- initialize: tools-only, no resources/prompts ---
    let init = client.request(
        1,
        "initialize",
        json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "codebus-it", "version": "0"}
        }),
    );
    let caps = &init["result"]["capabilities"];
    assert!(!caps["tools"].is_null(), "tools capability must be advertised: {init}");
    assert!(caps["resources"].is_null(), "resources must NOT be advertised: {init}");
    assert!(caps["prompts"].is_null(), "prompts must NOT be advertised: {init}");
    client.notify("notifications/initialized");

    // --- tools/list: the four query tools; wiki tools expose an optional
    // vault selector, vault_list takes none, and no tool takes a raw path ---
    let list = client.request(2, "tools/list", json!({}));
    let tools = list["result"]["tools"].as_array().expect("tools array");
    let names: Vec<&str> = tools
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(names.len(), 4, "exactly four tools: {names:?}");
    for expected in ["vault_list", "wiki_list", "wiki_read", "wiki_search"] {
        assert!(names.contains(&expected), "missing tool {expected}: {names:?}");
    }
    for tool in tools {
        let name = tool["name"].as_str().unwrap();
        let schema = serde_json::to_string(&tool["inputSchema"]).unwrap();
        assert!(
            !schema.contains("\"path\""),
            "tool {name} must not accept a raw filesystem path arg: {schema}"
        );
        if name == "vault_list" {
            assert!(
                !schema.contains("vault"),
                "vault_list takes no argument: {schema}"
            );
        } else {
            assert!(
                schema.contains("vault"),
                "wiki tool {name} must expose the optional vault selector: {schema}"
            );
        }
    }

    // --- mcp-usage-guidance: tool descriptions convey the cross-project
    // wiki-library use case while keeping the keyword mechanic ---
    let desc = |n: &str| -> String {
        tools
            .iter()
            .find(|t| t["name"].as_str() == Some(n))
            .unwrap()["description"]
            .as_str()
            .unwrap()
            .to_lowercase()
    };
    let vl = desc("vault_list");
    assert!(
        vl.contains("cross-project") && vl.contains("library"),
        "vault_list must frame the cross-project library: {vl}"
    );
    let ws = desc("wiki_search");
    assert!(
        ws.contains("keyword"),
        "wiki_search must keep the keyword instruction: {ws}"
    );
    assert!(
        ws.contains("across") && ws.contains("indexed"),
        "wiki_search must convey searching across indexed wikis: {ws}"
    );

    // --- wiki_list: lists the 3 wiki pages (incl. no-frontmatter), not the secret ---
    let listed = client.call_ok(3, "wiki_list", json!({}));
    let slugs: Vec<&str> = listed
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["slug"].as_str().unwrap())
        .collect();
    assert_eq!(slugs.len(), 3, "expected 3 wiki pages, got {slugs:?}");
    assert!(slugs.contains(&"index"));
    assert!(slugs.contains(&"plain"));
    assert!(!slugs.contains(&"secret"), "secret leaked into wiki_list: {slugs:?}");
    // CJK title survives the round trip; no-frontmatter page falls back to slug.
    let index_title = listed
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["slug"] == json!("index"))
        .unwrap()["title"]
        .as_str()
        .unwrap();
    assert_eq!(index_title, "索引首頁");

    // --- wiki_read: pagination boundaries on the 30k page ---
    let page0 = client.call_ok(4, "wiki_read", json!({"slug": "big", "offset": 0, "limit": 12000}));
    assert_eq!(page0["total_chars"], json!(30_000));
    assert_eq!(page0["has_more"], json!(true));
    assert_eq!(page0["next_offset"], json!(12_000));
    assert_eq!(page0["content"].as_str().unwrap().chars().count(), 12_000);

    let page_last = client.call_ok(5, "wiki_read", json!({"slug": "big", "offset": 24000, "limit": 12000}));
    assert_eq!(page_last["has_more"], json!(false));
    assert_eq!(page_last["next_offset"], Value::Null);
    assert_eq!(page_last["content"].as_str().unwrap().chars().count(), 6_000);

    // limit over the cap is clamped to 20000.
    let clamped = client.call_ok(6, "wiki_read", json!({"slug": "big", "offset": 0, "limit": 99999}));
    assert_eq!(clamped["content"].as_str().unwrap().chars().count(), 20_000);
    assert_eq!(clamped["next_offset"], json!(20_000));

    // --- wiki_read: frontmatter stripped ---
    let idx = client.call_ok(7, "wiki_read", json!({"slug": "index"}));
    assert!(
        idx["content"].as_str().unwrap().starts_with("Welcome"),
        "frontmatter must be stripped: {:?}",
        idx["content"]
    );

    // --- wiki_search: keyword hit with snippet; no-match empty; blank rejected ---
    let search = client.call_ok(8, "wiki_search", json!({"query": "authentication"}));
    let hits = search["results"].as_array().unwrap();
    assert_eq!(hits.len(), 1, "one page matches 'authentication': {hits:?}");
    assert_eq!(hits[0]["slug"], json!("index"));
    assert_eq!(search["truncated"], json!(false));

    let empty = client.call_ok(9, "wiki_search", json!({"query": "zzznomatch"}));
    assert!(empty["results"].as_array().unwrap().is_empty());

    let blank = client.call_raw(10, "wiki_search", json!({"query": "   "}));
    assert!(is_error_response(&blank), "blank query must be an error: {blank}");

    // --- errors: unknown slug, and the secret is unreachable ---
    let unknown = client.call_raw(11, "wiki_read", json!({"slug": "does-not-exist"}));
    assert!(is_error_response(&unknown), "unknown slug must error: {unknown}");

    let secret = client.call_raw(12, "wiki_read", json!({"slug": "secret"}));
    assert!(
        is_error_response(&secret),
        "raw/code/secret must be unreachable via wiki_read: {secret}"
    );
    let secret_search = client.call_ok(13, "wiki_search", json!({"query": "SECRET_TOKEN"}));
    assert!(
        secret_search["results"].as_array().unwrap().is_empty(),
        "raw/code content must not be searchable: {secret_search}"
    );
}

/// An empty (but existing) wiki dir yields an empty list, returned as success
/// — not an error (spec `mcp-server` § wiki_list "Empty vault").
#[test]
fn wiki_list_empty_vault_returns_empty_array() {
    let tmp = TempDir::new().unwrap();
    let wiki = tmp.path().join(".codebus").join("wiki");
    std::fs::create_dir_all(&wiki).unwrap();

    let mut client = McpClient::spawn(tmp.path());
    client.request(
        1,
        "initialize",
        json!({"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"t","version":"0"}}),
    );
    client.notify("notifications/initialized");
    let listed = client.call_ok(2, "wiki_list", json!({}));
    assert!(
        listed.as_array().unwrap().is_empty(),
        "empty wiki dir must list as empty: {listed}"
    );
}
