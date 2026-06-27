//! rmcp 1.8.0 stdio server wrapping the pure wiki tools in [`super::tools`].
//!
//! Uses the explicit `tool_router` template (struct field + `#[tool_router]` /
//! `#[tool]` / `#[tool_handler]`) verified against the pinned `=1.8.0` release;
//! `ServerInfo` is `#[non_exhaustive]` so it is built via mut-default, not a
//! struct literal. All blocking filesystem work runs on `spawn_blocking`; all
//! diagnostics go to stderr so stdout stays a clean JSON-RPC channel.

use std::path::PathBuf;

use rmcp::{
    ErrorData, ServerHandler, ServiceExt,
    handler::server::router::tool::ToolRouter,
    handler::server::wrapper::Parameters,
    model::{ServerCapabilities, ServerInfo},
    schemars, tool, tool_handler, tool_router,
    transport::stdio,
};
use serde::{Deserialize, Serialize};

use super::tools::{self, DEFAULT_READ_LIMIT};

/// Single-vault wiki MCP server. `wiki_root` is pinned at construction; the
/// tools never accept a path, so the server can only ever read under it.
#[derive(Clone)]
pub struct WikiServer {
    wiki_root: PathBuf,
    // `#[tool_handler]` generates code that reads this field, but rustc's
    // dead-code pass can't see through the macro (verified at runtime: the
    // tools capability is advertised and calls route). Silence the false
    // positive rather than leave a warning the quality gate would flag.
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct WikiReadArgs {
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
    /// A single keyword to match (case-insensitive substring), e.g.
    /// `authentication`. Pass a keyword, NOT a full sentence or question —
    /// this is literal substring matching, not semantic retrieval.
    query: String,
}

#[derive(Serialize)]
struct WikiListEntry {
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

enum ReadError {
    NotFound,
    Io(std::io::Error),
}

#[tool_router]
impl WikiServer {
    pub fn new(wiki_root: PathBuf) -> Self {
        Self {
            wiki_root,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        description = "List every wiki page in the vault. Returns each page's slug and title. Call this first to discover what pages exist, then wiki_read a slug for its content."
    )]
    async fn wiki_list(&self) -> Result<String, ErrorData> {
        let root = self.wiki_root.clone();
        let pages = tokio::task::spawn_blocking(move || codebus_core::wiki::read::list_pages(&root))
            .await
            .map_err(|e| ErrorData::internal_error(format!("join error: {e}"), None))?
            .map_err(|e| ErrorData::internal_error(format!("list pages: {e}"), None))?;
        let entries: Vec<WikiListEntry> = pages
            .into_iter()
            .map(|p| WikiListEntry {
                slug: p.slug,
                title: p.title,
            })
            .collect();
        serde_json::to_string(&entries)
            .map_err(|e| ErrorData::internal_error(format!("serialize: {e}"), None))
    }

    #[tool(
        description = "Read one wiki page's body (frontmatter stripped), paginated by character. Args: slug (required), offset (default 0), limit (default 12000, max 20000). When has_more is true, call again with offset = next_offset to continue."
    )]
    async fn wiki_read(
        &self,
        Parameters(args): Parameters<WikiReadArgs>,
    ) -> Result<String, ErrorData> {
        let root = self.wiki_root.clone();
        let slug = args.slug.clone();
        let offset = args.offset.unwrap_or(0);
        let limit = args.limit.unwrap_or(DEFAULT_READ_LIMIT);

        let (title, body) = tokio::task::spawn_blocking(move || {
            let path = tools::resolve_page_path(&root, &slug).ok_or(ReadError::NotFound)?;
            let raw = std::fs::read_to_string(&path).map_err(ReadError::Io)?;
            let title = codebus_core::wiki::read::frontmatter_title(&raw, &slug);
            let body = codebus_core::wiki::read::strip_frontmatter(&raw).to_string();
            Ok::<_, ReadError>((title, body))
        })
        .await
        .map_err(|e| ErrorData::internal_error(format!("join error: {e}"), None))?
        .map_err(|e| match e {
            ReadError::NotFound => {
                ErrorData::invalid_params(format!("no such page: {}", args.slug), None)
            }
            ReadError::Io(io) => ErrorData::internal_error(format!("read page: {io}"), None),
        })?;

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
        serde_json::to_string(&result)
            .map_err(|e| ErrorData::internal_error(format!("serialize: {e}"), None))
    }

    #[tool(
        description = "Search wiki pages for a keyword (case-insensitive substring over title and body). Returns matching pages with slug, title, and a context snippet. Pass a single keyword like `authentication`, NOT a full sentence — this is literal substring matching, not semantic search."
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
        let root = self.wiki_root.clone();
        let query = args.query.clone();
        let outcome = tokio::task::spawn_blocking(move || tools::search_pages(&root, &query))
            .await
            .map_err(|e| ErrorData::internal_error(format!("join error: {e}"), None))?
            .map_err(|e| ErrorData::internal_error(format!("search: {e}"), None))?;
        serde_json::to_string(&outcome)
            .map_err(|e| ErrorData::internal_error(format!("serialize: {e}"), None))
    }
}

#[tool_handler]
impl ServerHandler for WikiServer {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default();
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info.instructions = Some(
            "codebus wiki for one vault. Use wiki_list to discover pages, wiki_read to read a \
             page (paginated by character), wiki_search to find pages by keyword. Read-only."
                .to_string(),
        );
        info
    }
}

/// Start the stdio MCP server bound to `wiki_root`, blocking until the client
/// disconnects. Diagnostics go to stderr; stdout is the JSON-RPC channel.
pub async fn serve(wiki_root: PathBuf) -> anyhow::Result<()> {
    let server = WikiServer::new(wiki_root);
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
