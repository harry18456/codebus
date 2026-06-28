//! rmcp 1.8.0 stdio server wrapping the pure wiki tools in [`super::tools`] and
//! the vault resolution in [`super::registry`].
//!
//! Uses the explicit `tool_router` template (struct field + `#[tool_router]` /
//! `#[tool]` / `#[tool_handler]`) verified against the pinned `=1.8.0` release;
//! `ServerInfo` is `#[non_exhaustive]` so it is built via mut-default, not a
//! struct literal. All blocking filesystem work — including the per-call
//! registry reread — runs on `spawn_blocking`; all diagnostics go to stderr so
//! stdout stays a clean JSON-RPC channel.
//!
//! Four query-only tools: `vault_list` (discovery) plus `wiki_list` /
//! `wiki_read` / `wiki_search`. The three wiki tools take an OPTIONAL `vault`
//! selector resolved by [`super::registry`]; in registry mode, omitting it on
//! `wiki_list` / `wiki_search` aggregates across all present vaults and tags
//! each result with its source vault. The pure query logic in `tools.rs` is
//! unchanged — this layer only adds vault resolution, aggregation, and tagging.

use rmcp::{
    ErrorData, ServerHandler, ServiceExt,
    handler::server::router::tool::ToolRouter,
    handler::server::wrapper::Parameters,
    model::{ServerCapabilities, ServerInfo},
    schemars, tool, tool_handler, tool_router,
    transport::stdio,
};
use serde::{Deserialize, Serialize};

use super::registry::{self, ResolveError, ServeMode};
use super::tools::{self, DEFAULT_READ_LIMIT};

/// Multi-vault wiki MCP server. `mode` is fixed at construction (pinned to one
/// vault, or registry-backed); the registry is re-read on each call.
#[derive(Clone)]
pub struct WikiServer {
    mode: ServeMode,
    // `#[tool_handler]` generates code that reads this field, but rustc's
    // dead-code pass can't see through the macro (verified at runtime: the
    // tools capability is advertised and calls route). Silence the false
    // positive rather than leave a warning the quality gate would flag.
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct WikiListArgs {
    /// Optional vault selector — the `vault` path from `vault_list`. Omit it to
    /// list pages across ALL registered vaults at once.
    #[serde(default)]
    vault: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct WikiReadArgs {
    /// The source vault of the page — the `vault` carried on the `wiki_list` /
    /// `wiki_search` result. Required when more than one vault is registered.
    #[serde(default)]
    vault: Option<String>,
    /// Page slug (filename without `.md`), as returned by `wiki_list`.
    slug: String,
    /// Character offset to start from. Defaults to 0.
    #[serde(default)]
    offset: Option<usize>,
    /// Max characters to return. Defaults to 12000, capped at 20000. Use
    /// `next_offset` from the previous call to page through a long document.
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct WikiSearchArgs {
    /// Optional vault selector — the `vault` path from `vault_list`. Omit it to
    /// search across ALL registered vaults at once.
    #[serde(default)]
    vault: Option<String>,
    /// A single keyword to match (case-insensitive substring), e.g.
    /// `authentication`. Pass a keyword, NOT a full sentence or question —
    /// this is literal substring matching, not semantic retrieval.
    query: String,
}

#[derive(Serialize)]
struct VaultListEntry {
    vault: String,
    name: String,
}

#[derive(Serialize)]
struct WikiListEntry {
    /// Source vault id — present only in registry (multi-vault) mode, so the
    /// caller can pass it back to `wiki_read`.
    #[serde(skip_serializing_if = "Option::is_none")]
    vault: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    slug: String,
    title: String,
}

#[derive(Serialize)]
struct WikiReadResult {
    slug: String,
    title: String,
    content: String,
    offset: usize,
    next_offset: Option<usize>,
    has_more: bool,
    total_chars: usize,
}

#[derive(Serialize)]
struct SearchHitOut {
    #[serde(skip_serializing_if = "Option::is_none")]
    vault: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    slug: String,
    title: String,
    snippet: String,
}

#[derive(Serialize)]
struct SearchOutcomeOut {
    results: Vec<SearchHitOut>,
    truncated: bool,
}

/// Error from a tool handler's blocking body, mapped to MCP `ErrorData` after
/// the `spawn_blocking` join. Keeps the error-vs-empty distinction: a vault
/// resolution failure or unknown slug is `invalid_params`, a real fs failure
/// is `internal_error` — never silently coerced into an empty success.
enum HandlerErr {
    Resolve(ResolveError),
    Io(std::io::Error),
    NotFound(String),
}

impl HandlerErr {
    fn into_error_data(self) -> ErrorData {
        match self {
            HandlerErr::Resolve(r) => ErrorData::invalid_params(r.message(), None),
            HandlerErr::Io(io) => ErrorData::internal_error(format!("read error: {io}"), None),
            HandlerErr::NotFound(slug) => {
                ErrorData::invalid_params(format!("no such page: {slug}"), None)
            }
        }
    }
}

fn join_err(e: tokio::task::JoinError) -> ErrorData {
    ErrorData::internal_error(format!("join error: {e}"), None)
}

fn serialize_err(e: serde_json::Error) -> ErrorData {
    ErrorData::internal_error(format!("serialize: {e}"), None)
}

#[tool_router]
impl WikiServer {
    pub fn new(mode: ServeMode) -> Self {
        Self {
            mode,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        description = "List the codebus vaults this server can query. Returns each vault's `vault` (its absolute path — pass this as the `vault` argument to the other tools) and `name` (display label). In multi-vault mode you can also just omit `vault` on wiki_list / wiki_search to explore across every vault at once."
    )]
    async fn vault_list(&self) -> Result<String, ErrorData> {
        let mode = self.mode.clone();
        let entries = tokio::task::spawn_blocking(move || registry::list_entries(&mode))
            .await
            .map_err(join_err)?;
        let out: Vec<VaultListEntry> = entries
            .into_iter()
            .map(|(vault, name)| VaultListEntry { vault, name })
            .collect();
        serde_json::to_string(&out).map_err(serialize_err)
    }

    #[tool(
        description = "List wiki pages. Optional `vault` selects one vault (the `vault` path from vault_list); omit it to list pages across ALL registered vaults at once. Each entry carries slug + title, plus its source `vault` in multi-vault mode. Call this (or wiki_search) first to discover pages, then wiki_read a slug."
    )]
    async fn wiki_list(
        &self,
        Parameters(args): Parameters<WikiListArgs>,
    ) -> Result<String, ErrorData> {
        let mode = self.mode.clone();
        let entries = tokio::task::spawn_blocking(move || -> Result<Vec<WikiListEntry>, HandlerErr> {
            let vaults =
                registry::resolve_for_query(&mode, args.vault.as_deref()).map_err(HandlerErr::Resolve)?;
            let tag = registry::tags_source(&mode);
            let mut out = Vec::new();
            for rv in vaults {
                if !rv.wiki_root.is_dir() {
                    continue; // a present vault without a wiki contributes nothing
                }
                let pages =
                    codebus_core::wiki::read::list_pages(&rv.wiki_root).map_err(HandlerErr::Io)?;
                for p in pages {
                    out.push(WikiListEntry {
                        vault: tag.then(|| rv.vault.clone()),
                        name: tag.then(|| rv.name.clone()),
                        slug: p.slug,
                        title: p.title,
                    });
                }
            }
            Ok(out)
        })
        .await
        .map_err(join_err)?
        .map_err(HandlerErr::into_error_data)?;
        serde_json::to_string(&entries).map_err(serialize_err)
    }

    #[tool(
        description = "Read one wiki page's body (frontmatter stripped), paginated by character. Args: vault (the page's source vault from wiki_list/wiki_search — required when more than one vault is registered), slug (required), offset (default 0), limit (default 12000, max 20000). When has_more is true, call again with offset = next_offset to continue."
    )]
    async fn wiki_read(
        &self,
        Parameters(args): Parameters<WikiReadArgs>,
    ) -> Result<String, ErrorData> {
        let mode = self.mode.clone();
        let slug = args.slug.clone();
        let vault_arg = args.vault.clone();
        let offset = args.offset.unwrap_or(0);
        let limit = args.limit.unwrap_or(DEFAULT_READ_LIMIT);

        let (title, body) =
            tokio::task::spawn_blocking(move || -> Result<(String, String), HandlerErr> {
                let rv = registry::resolve_for_read(&mode, vault_arg.as_deref())
                    .map_err(HandlerErr::Resolve)?;
                let path = tools::resolve_page_path(&rv.wiki_root, &slug)
                    .ok_or_else(|| HandlerErr::NotFound(slug.clone()))?;
                let raw = std::fs::read_to_string(&path).map_err(HandlerErr::Io)?;
                let title = codebus_core::wiki::read::frontmatter_title(&raw, &slug);
                let body = codebus_core::wiki::read::strip_frontmatter(&raw).to_string();
                Ok((title, body))
            })
            .await
            .map_err(join_err)?
            .map_err(HandlerErr::into_error_data)?;

        let slice = tools::paginate(&body, offset, limit);
        let result = WikiReadResult {
            slug: args.slug,
            title,
            content: slice.content,
            offset: slice.offset,
            next_offset: slice.next_offset,
            has_more: slice.has_more,
            total_chars: slice.total_chars,
        };
        serde_json::to_string(&result).map_err(serialize_err)
    }

    #[tool(
        description = "Search wiki pages for a keyword (case-insensitive substring over title and body). Optional `vault` limits the search to one vault; omit it to search across ALL registered vaults at once. Returns matching pages with slug, title, snippet, plus the source `vault` in multi-vault mode. Pass a single keyword like `authentication`, NOT a full sentence — this is literal substring matching, not semantic search."
    )]
    async fn wiki_search(
        &self,
        Parameters(args): Parameters<WikiSearchArgs>,
    ) -> Result<String, ErrorData> {
        if args.query.trim().is_empty() {
            return Err(ErrorData::invalid_params(
                "query must be a non-empty keyword",
                None,
            ));
        }
        let mode = self.mode.clone();
        let query = args.query.clone();
        let vault_arg = args.vault.clone();

        let outcome = tokio::task::spawn_blocking(move || -> Result<SearchOutcomeOut, HandlerErr> {
            let vaults = registry::resolve_for_query(&mode, vault_arg.as_deref())
                .map_err(HandlerErr::Resolve)?;
            let tag = registry::tags_source(&mode);
            let mut hits: Vec<SearchHitOut> = Vec::new();
            let mut any_truncated = false;
            for rv in vaults {
                if !rv.wiki_root.is_dir() {
                    continue;
                }
                let res = tools::search_pages(&rv.wiki_root, &query).map_err(HandlerErr::Io)?;
                any_truncated |= res.truncated;
                for hit in res.results {
                    hits.push(SearchHitOut {
                        vault: tag.then(|| rv.vault.clone()),
                        name: tag.then(|| rv.name.clone()),
                        slug: hit.slug,
                        title: hit.title,
                        snippet: hit.snippet,
                    });
                }
            }
            // Global cap across all searched vaults: more matched than returned
            // (either a single vault already truncated, or the merged total
            // exceeds the cap) → truncated.
            let truncated = any_truncated || hits.len() > tools::SEARCH_RESULT_CAP;
            hits.truncate(tools::SEARCH_RESULT_CAP);
            Ok(SearchOutcomeOut {
                results: hits,
                truncated,
            })
        })
        .await
        .map_err(join_err)?
        .map_err(HandlerErr::into_error_data)?;

        serde_json::to_string(&outcome).map_err(serialize_err)
    }
}

#[tool_handler]
impl ServerHandler for WikiServer {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default();
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info.instructions = Some(match self.mode {
            ServeMode::Pinned { .. } => {
                "codebus wiki for one pinned vault. Use wiki_list to discover pages, wiki_read to \
                 read a page (paginated by character), wiki_search to find pages by keyword. \
                 Read-only."
                    .to_string()
            }
            ServeMode::Registry => {
                "codebus wiki over MCP (multi-vault). Call vault_list to see the registered \
                 vaults, or omit `vault` on wiki_list / wiki_search to explore across all of them \
                 at once; pass a result's `vault` into wiki_read to read that page. Read-only."
                    .to_string()
            }
        });
        info
    }
}

/// Start the stdio MCP server in `mode`, blocking until the client
/// disconnects. Diagnostics go to stderr; stdout is the JSON-RPC channel.
pub async fn serve(mode: ServeMode) -> anyhow::Result<()> {
    let server = WikiServer::new(mode);
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
