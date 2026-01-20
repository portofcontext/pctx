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

/// Convert PascalCase/camelCase to spaced lowercase
/// Example: "MyNamespace" -> "my namespace"
fn to_spaced_lowercase(s: &str) -> String {
    let snake = case::Case::Snake.sanitize(s);
    snake.replace("_", " ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use pctx_codegen::{Tool, ToolSet};

    fn create_test_tool(name: &str, description: Option<&str>) -> Tool {
        Tool::new_mcp(
            name,
            description.map(String::from),
            Default::default(),
            None,
        )
        .unwrap()
    }

    fn create_test_index(tools: Vec<Tool>) -> ToolSearchIndex {
        create_test_index_with_namespace("Tools", tools)
    }

    fn create_test_index_with_namespace(namespace: &str, tools: Vec<Tool>) -> ToolSearchIndex {
        let toolset = ToolSet::new(namespace, "", tools);
        ToolSearchIndex::from_tool_sets(&[toolset])
    }

    fn create_test_index_multi_namespace(namespaces: Vec<(&str, Vec<Tool>)>) -> ToolSearchIndex {
        let toolsets: Vec<ToolSet> = namespaces
            .into_iter()
            .map(|(n, t)| ToolSet::new(n, "", t))
            .collect();

        ToolSearchIndex::from_tool_sets(&toolsets)
    }

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
    fn test_empty_query_returns_nothing() {
        let index = create_test_index(vec![
            create_test_tool("tool_one", Some("First tool")),
            create_test_tool("tool_two", Some("Second tool")),
        ]);

        // Empty query should return nothing (BM25 requires terms to match)
        assert!(index.search("", 10).is_empty());
        assert!(index.search("   ", 10).is_empty());
    }

    #[test]
    fn test_search_case_insensitivity() {
        let tool = create_test_tool("calculate_total", Some("Calculate the total sum of items"));
        let expected_id = tool.id;
        let index = create_test_index(vec![tool]);

        // All should find the same tool
        let lower = index.search("calculate", 3);
        let upper = index.search("CALCULATE", 3);
        let mixed = index.search("CaLcUlAtE", 3);

        assert_eq!(lower.len(), 1);
        assert_eq!(upper.len(), 1);
        assert_eq!(mixed.len(), 1);

        // All should return the expected tool_id
        assert_eq!(lower[0].tool_id, expected_id);
        assert_eq!(upper[0].tool_id, expected_id);
        assert_eq!(mixed[0].tool_id, expected_id);
    }

    #[test]
    fn test_search_by_description() {
        let tools = vec![
            create_test_tool(
                "process_data",
                Some("Transform and validate incoming user records"),
            ),
            create_test_tool(
                "send_notification",
                Some("Alert the administrator about system events"),
            ),
        ];
        let process_data_id = tools[0].id;
        let send_notification_id = tools[1].id;
        let index = create_test_index(tools);

        // Search by description keywords
        let results = index.search("user records", 3);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].tool_id, process_data_id);

        // Search by different description keyword
        let results = index.search("administrator alert", 3);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].tool_id, send_notification_id);
    }

    #[test]
    fn test_search_no_matches() {
        let index = create_test_index(vec![create_test_tool("add", Some("Add two numbers"))]);

        // Search for something completely unrelated
        let results = index.search("xyzzy quantum blockchain", 3);
        // BM25 requires term matches, so this should return empty
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_similar_names() {
        let tools = vec![
            create_test_tool("get_user", Some("Retrieve a single user by ID")),
            create_test_tool("get_users", Some("Retrieve multiple users")),
            create_test_tool(
                "get_user_profile",
                Some("Retrieve detailed user profile information"),
            ),
        ];
        let get_user_id = tools[0].id;
        let get_users_id = tools[1].id;
        let get_user_profile_id = tools[2].id;
        let index = create_test_index(tools);

        // Search for single user - should return get_user first
        let results = index.search("single user by id", 3);
        assert!(!results.is_empty());
        assert_eq!(results[0].tool_id, get_user_id);

        // Search for multiple users - should return get_users first
        let results = index.search("retrieve multiple users", 3);
        assert!(!results.is_empty());
        assert_eq!(results[0].tool_id, get_users_id);

        // Search for profile - should return get_user_profile first
        let results = index.search("user profile", 3);
        assert!(!results.is_empty());
        assert_eq!(results[0].tool_id, get_user_profile_id);
    }

    #[test]
    fn test_search_with_namespace() {
        let db_tool = create_test_tool("query", Some("Execute a database query"));
        let api_tool = create_test_tool("query", Some("Query an API endpoint"));
        let cache_tool = create_test_tool("get", Some("Get a value from cache"));
        let db_id = db_tool.id;
        let api_id = api_tool.id;
        let cache_id = cache_tool.id;
        let index = create_test_index_multi_namespace(vec![
            ("Database", vec![db_tool]),
            ("Api", vec![api_tool]),
            ("Cache", vec![cache_tool]),
        ]);

        // Search for database query
        let results = index.search("database query", 3);
        assert!(!results.is_empty());
        assert_eq!(results[0].tool_id, db_id);

        // Search for API
        let results = index.search("api endpoint", 3);
        assert!(!results.is_empty());
        assert_eq!(results[0].tool_id, api_id);

        // Search for cache
        let results = index.search("cache value", 3);
        assert!(!results.is_empty());
        assert_eq!(results[0].tool_id, cache_id);
    }

    #[test]
    fn test_search_k_parameter_limits_results() {
        let tools = vec![
            create_test_tool("func_a", Some("Function A does things")),
            create_test_tool("func_b", Some("Function B does things")),
            create_test_tool("func_c", Some("Function C does things")),
            create_test_tool("func_d", Some("Function D does things")),
        ];
        let all_ids: Vec<Uuid> = tools.iter().map(|t| t.id).collect();
        let index = create_test_index(tools);

        // k=1 should return exactly 1 result
        let results = index.search("function", 1);
        assert_eq!(results.len(), 1);
        assert!(all_ids.contains(&results[0].tool_id));

        // k=2 should return exactly 2 results
        let results = index.search("function", 2);
        assert_eq!(results.len(), 2);

        // k=3 should return exactly 3 results
        let results = index.search("function", 3);
        assert_eq!(results.len(), 3);

        // k=4 should return all 4
        let results = index.search("function", 4);
        assert_eq!(results.len(), 4);

        // k greater than available should return all 4
        let results = index.search("function", 100);
        assert_eq!(results.len(), 4);
        // Verify all returned IDs are from our tools
        for result in &results {
            assert!(all_ids.contains(&result.tool_id));
        }
    }

    #[test]
    fn test_search_special_characters() {
        let index = create_test_index(vec![create_test_tool(
            "normal_function",
            Some("A normal function"),
        )]);

        // Search with special characters - should not panic
        let results = index.search("test (with) [brackets]", 3);
        assert!(results.is_empty() || !results.is_empty()); // Just checking it doesn't panic

        // Search with quotes
        let results = index.search("test \"quoted\"", 3);
        assert!(results.is_empty() || !results.is_empty());

        // Search with unicode
        let results = index.search("tëst üñíçödé", 3);
        assert!(results.is_empty() || !results.is_empty());
    }

    #[test]
    fn test_search_camel_case_conversion() {
        let tools = vec![
            create_test_tool(
                "get_all_items_from_database",
                Some("Get all items from a database table"),
            ),
            create_test_tool(
                "send_email_notification_to_user",
                Some("Send an email notification to a user"),
            ),
        ];
        let get_all_items_id = tools[0].id;
        let send_email_id = tools[1].id;
        let index = create_test_index(tools);

        // Search using individual words from snake_case name
        let results = index.search("get all items", 3);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].tool_id, get_all_items_id);

        // Search using description words
        let results = index.search("email notification user", 3);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].tool_id, send_email_id);
    }

    #[test]
    fn test_search_underscore_namespace() {
        let tool = create_test_tool("foo_bar", None);
        let foo_bar_id = tool.id;
        let index = create_test_index_with_namespace("namespaced_with_underscore", vec![tool]);

        // Search for namespace words (underscores converted to spaces)
        let results = index.search("namespaced", 3);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].tool_id, foo_bar_id);

        // Search for function name words
        let results = index.search("bar", 3);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].tool_id, foo_bar_id);
    }

    #[test]
    fn test_search_add_numbers() {
        let tools = vec![
            create_test_tool("add_numbers", Some("Add two numbers together")),
            create_test_tool("greet", Some("Greet someone with a custom greeting")),
        ];
        let add_numbers_id = tools[0].id;
        let index = create_test_index(tools);

        let results = index.search("Add numbers together", 3);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].tool_id, add_numbers_id);
    }

    #[test]
    fn test_search_greet_user() {
        let tools = vec![
            create_test_tool("add_numbers", Some("Add two numbers together")),
            create_test_tool("greet", Some("Greet someone with a custom greeting")),
        ];
        let greet_id = tools[1].id;
        let index = create_test_index(tools);

        let results = index.search("greet user", 3);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].tool_id, greet_id);
    }

    #[test]
    fn test_search_k_greater_than_available() {
        let tools = vec![
            create_test_tool("add_numbers", Some("Add two numbers together")),
            create_test_tool("greet", Some("Greet someone with a custom greeting")),
        ];
        let add_numbers_id = tools[0].id;
        let greet_id = tools[1].id;
        let index = create_test_index(tools);

        // k=5 but only 2 tools, should return both matching ones
        let results = index.search("Greet number", 5);
        // "Greet" matches greet, "number" matches add_numbers
        assert_eq!(results.len(), 2);
        let result_ids: Vec<Uuid> = results.iter().map(|r| r.tool_id).collect();
        assert!(result_ids.contains(&add_numbers_id));
        assert!(result_ids.contains(&greet_id));
    }

    #[test]
    fn test_search_results_ordered_by_score() {
        let tools = vec![
            create_test_tool("unrelated_function", Some("Does something else entirely")),
            create_test_tool("search_items", Some("Search for items in the database")),
            create_test_tool("search_users", Some("Search for users in the system")),
        ];
        let search_items_id = tools[1].id;
        let search_users_id = tools[2].id;
        let index = create_test_index(tools);

        let results = index.search("search", 3);
        // Should only match the two search_* functions
        assert_eq!(results.len(), 2);
        // Results should be sorted by score descending
        for i in 1..results.len() {
            assert!(
                results[i - 1].score >= results[i].score,
                "Results should be ordered by score descending"
            );
        }
        // Both search functions should be in results
        let result_ids: Vec<Uuid> = results.iter().map(|r| r.tool_id).collect();
        assert!(result_ids.contains(&search_items_id));
        assert!(result_ids.contains(&search_users_id));
    }

    #[test]
    fn test_search_filters_zero_score_results() {
        let tools = vec![
            create_test_tool("add", Some("Add numbers")),
            create_test_tool("multiply", Some("Multiply numbers")),
        ];
        let add_id = tools[0].id;
        let index = create_test_index(tools);

        // Search for "add" should only match the add tool
        let results = index.search("add", 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].tool_id, add_id);
        assert!(
            results[0].score > 0.0,
            "Results should have positive scores"
        );
    }

    #[test]
    fn test_index_len() {
        let index = create_test_index(vec![
            create_test_tool("tool_a", None),
            create_test_tool("tool_b", None),
            create_test_tool("tool_c", None),
        ]);
        assert_eq!(index.len(), 3);
    }

    #[test]
    fn test_index_is_empty() {
        let empty_index = ToolSearchIndex::from_tool_sets(&[]);
        assert!(empty_index.is_empty());

        let non_empty = create_test_index(vec![create_test_tool("tool", None)]);
        assert!(!non_empty.is_empty());
    }

    #[test]
    fn test_search_toolset_with_empty_tools() {
        let toolset = ToolSet {
            name: "Empty".to_string(),
            namespace: "Empty".to_string(),
            description: "Empty toolset".to_string(),
            tools: vec![],
        };
        let index = ToolSearchIndex::from_tool_sets(&[toolset]);
        assert!(index.is_empty());
        assert!(index.search("anything", 10).is_empty());
    }

    #[test]
    fn test_search_mixed_toolsets() {
        let my_tool = create_test_tool("my_tool", Some("A useful tool"));
        let my_tool_id = my_tool.id;
        let empty_toolset = ToolSet::new("Empty", "Empty toolset", vec![]);
        let filled_toolset = ToolSet::new("Filled", "Filled toolset", vec![my_tool]);

        let index = ToolSearchIndex::from_tool_sets(&[empty_toolset, filled_toolset]);
        assert_eq!(index.len(), 1);

        let results = index.search("useful tool", 3);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].tool_id, my_tool_id);
    }
}
