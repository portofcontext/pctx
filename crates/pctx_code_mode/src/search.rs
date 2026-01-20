//! BM25 search functionality for CodeMode functions
//!
//! Provides full-text search over available functions using BM25 ranking.

use bm25::{Document, Language, SearchEngine, SearchEngineBuilder};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use pctx_codegen::{ToolSet, case};

/// Wrapper around BM25 SearchEngine that maps document indices to Tool IDs
pub struct ToolSearchIndex {
    engine: SearchEngine<usize>,
    /// Maps document index to (tool_id, namespace)
    tool_ids: Vec<Uuid>,
}

impl std::fmt::Debug for ToolSearchIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolSearchIndex")
            .field("tool_count", &self.tool_ids.len())
            .finish_non_exhaustive()
    }
}

impl ToolSearchIndex {
    /// Build a search index from tool sets
    pub fn from_tool_sets(tool_sets: &[ToolSet]) -> Self {
        let mut tool_ids = Vec::new();
        let mut documents = Vec::new();

        for tool_set in tool_sets {
            if tool_set.tools.is_empty() {
                continue;
            }

            for tool in &tool_set.tools {
                // Build corpus entry matching Python format:
                // "{namespace_spaced}.{fn_name_spaced}: {description}"
                let namespace_spaced = to_spaced_lowercase(&tool_set.namespace);
                let fn_name_spaced = to_spaced_lowercase(&tool.fn_name);
                let description = tool.description.as_deref().unwrap_or("");

                let contents = format!("{namespace_spaced}.{fn_name_spaced}: {description}");

                documents.push(Document {
                    id: tool_ids.len(),
                    contents,
                });
                tool_ids.push(tool.id);
            }
        }

        let engine = SearchEngineBuilder::with_documents(Language::English, documents).build();

        Self { engine, tool_ids }
    }

    /// Search for functions matching the query
    ///
    /// Returns Tool IDs with their relevance scores, ordered by score descending
    pub fn search(&self, query: &str, k: usize) -> Vec<SearchResult> {
        if self.tool_ids.is_empty() || query.trim().is_empty() {
            return Vec::new();
        }

        let actual_k = k.min(self.tool_ids.len());
        let results = self.engine.search(query, actual_k);

        results
            .into_iter()
            .filter(|r| r.score > 0.0)
            .filter_map(|r| {
                let idx = r.document.id;
                self.tool_ids.get(idx).map(|id| SearchResult {
                    tool_id: *id,
                    score: r.score,
                })
            })
            .collect()
    }

    /// Returns the number of indexed tools
    pub fn len(&self) -> usize {
        self.tool_ids.len()
    }

    /// Returns true if the index is empty
    pub fn is_empty(&self) -> bool {
        self.tool_ids.is_empty()
    }
}

/// A search result containing a tool ID and its relevance score
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct SearchResult {
    /// The unique identifier of the matching tool
    #[schema(value_type = String)]
    #[schemars(with = "String")]
    pub tool_id: Uuid,
    /// BM25 relevance score (higher = more relevant)
    pub score: f32,
}

// -------------- Search Functions Input/Output --------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct SearchFunctionsInput {
    /// Search query to find relevant functions
    pub query: String,
    /// Maximum number of results to return (default: 10)
    #[serde(default = "default_k")]
    pub k: usize,
}

fn default_k() -> usize {
    10
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct SearchFunctionsOutput {
    /// Functions matching the search query, ordered by relevance
    pub functions: Vec<SearchResult>,
}

/// Convert PascalCase/camelCase to spaced lowercase
/// Example: "MyNamespace" -> "my namespace"
fn to_spaced_lowercase(s: &str) -> String {
    let snake = case::Case::Snake.sanitize(s);
    snake.replace("_", " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_spaced_lowercase() {
        assert_eq!(to_spaced_lowercase("MyNamespace"), "my namespace");
        assert_eq!(to_spaced_lowercase("getData"), "get data");
        assert_eq!(to_spaced_lowercase("APIClient"), "api client");
        assert_eq!(to_spaced_lowercase("snake_case"), "snake case");
        assert_eq!(to_spaced_lowercase("XMLParser"), "xml parser");
    }

    #[test]
    fn test_empty_index() {
        let index = ToolSearchIndex::from_tool_sets(&[]);
        assert!(index.is_empty());
        assert_eq!(index.len(), 0);
        assert!(index.search("test", 10).is_empty());
    }

    #[test]
    fn test_empty_query() {
        let index = ToolSearchIndex::from_tool_sets(&[]);
        assert!(index.search("", 10).is_empty());
        assert!(index.search("   ", 10).is_empty());
    }
}
