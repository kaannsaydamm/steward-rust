//! A curated, hardcoded catalog of official MCP reference servers (see
//! <https://github.com/modelcontextprotocol/servers>). This is the "browse and add" half of the
//! MCP adapters panel: `mcp_registry`/`mcp_lifecycle` manage adapters the user already registered,
//! this module lists ones they haven't yet, so the UI/CLI can offer a one-click starting point.

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogEntry {
    pub catalog_id: &'static str,
    pub name: &'static str,
    pub publisher: &'static str,
    pub description: &'static str,
    pub command: &'static str,
    pub args: &'static [&'static str],
}

pub fn entries() -> Vec<CatalogEntry> {
    vec![
        CatalogEntry {
            catalog_id: "filesystem",
            name: "Filesystem",
            publisher: "modelcontextprotocol",
            description: "Read/write files under directories you allow. Append the allowed \
                directory path as an extra argument when registering.",
            command: "npx",
            args: &["-y", "@modelcontextprotocol/server-filesystem"],
        },
        CatalogEntry {
            catalog_id: "memory",
            name: "Memory",
            publisher: "modelcontextprotocol",
            description: "Persistent knowledge-graph memory across sessions.",
            command: "npx",
            args: &["-y", "@modelcontextprotocol/server-memory"],
        },
        CatalogEntry {
            catalog_id: "sequential-thinking",
            name: "Sequential Thinking",
            publisher: "modelcontextprotocol",
            description: "Structured step-by-step reasoning tool for breaking down complex \
                problems.",
            command: "npx",
            args: &["-y", "@modelcontextprotocol/server-sequential-thinking"],
        },
        CatalogEntry {
            catalog_id: "everything",
            name: "Everything (reference/test)",
            publisher: "modelcontextprotocol",
            description: "Reference server exercising the full MCP feature set. Useful for \
                testing an MCP client integration.",
            command: "npx",
            args: &["-y", "@modelcontextprotocol/server-everything"],
        },
        CatalogEntry {
            catalog_id: "brave-search",
            name: "Brave Search",
            publisher: "modelcontextprotocol",
            description: "Web and local search via the Brave Search API. Requires a \
                BRAVE_API_KEY environment variable.",
            command: "npx",
            args: &["-y", "@modelcontextprotocol/server-brave-search"],
        },
        CatalogEntry {
            catalog_id: "fetch",
            name: "Fetch",
            publisher: "modelcontextprotocol",
            description: "Fetches a URL and converts its content to markdown for the model to \
                read. Requires uv/uvx installed.",
            command: "uvx",
            args: &["mcp-server-fetch"],
        },
        CatalogEntry {
            catalog_id: "git",
            name: "Git",
            publisher: "modelcontextprotocol",
            description: "Read, search, and manipulate local Git repositories. Requires uv/uvx \
                installed.",
            command: "uvx",
            args: &["mcp-server-git"],
        },
    ]
}

#[cfg(test)]
#[path = "mcp_catalog_tests.rs"]
mod tests;
