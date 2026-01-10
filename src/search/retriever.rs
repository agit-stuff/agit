//! Search retriever for querying the Tantivy index.

use std::path::Path;

use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::OwnedValue;
use tantivy::TantivyDocument;

use crate::error::Result;

use super::open_or_create_index;

/// A single search result.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Unique identifier (timestamp)
    pub id: String,
    /// The matched content
    pub body: String,
    /// Category of the entry (intent, reasoning, error, note)
    pub category: String,
    /// Unix timestamp
    pub timestamp: u64,
    /// Relevance score
    pub score: f32,
}

/// Search the index for relevant context.
///
/// Performs a full-text search on the `body` field and returns
/// the top `limit` results sorted by relevance score.
pub fn search(agit_dir: &Path, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
    let index = open_or_create_index(agit_dir)?;
    let schema = index.schema();
    // Force reload to see freshly committed documents
    let reader = index.reader()?;
    reader.reload()?;
    let searcher = reader.searcher();

    let body_field = schema.get_field("body")?;
    let id_field = schema.get_field("id")?;
    let category_field = schema.get_field("category")?;
    let timestamp_field = schema.get_field("timestamp")?;

    let query_parser = QueryParser::for_index(&index, vec![body_field]);
    let parsed_query = query_parser.parse_query(query)?;

    let top_docs = searcher.search(&parsed_query, &TopDocs::with_limit(limit))?;

    let mut results = Vec::new();
    for (score, doc_address) in top_docs {
        let doc: TantivyDocument = searcher.doc(doc_address)?;

        let id = extract_text(&doc, id_field);
        let body = extract_text(&doc, body_field);
        let category = extract_text(&doc, category_field);
        let timestamp = extract_u64(&doc, timestamp_field);

        results.push(SearchResult {
            id,
            body,
            category,
            timestamp,
            score,
        });
    }

    Ok(results)
}

/// Extract text value from a document field.
fn extract_text(doc: &TantivyDocument, field: tantivy::schema::Field) -> String {
    doc.get_first(field)
        .and_then(|v| match v {
            OwnedValue::Str(s) => Some(s.to_string()),
            _ => None,
        })
        .unwrap_or_default()
}

/// Extract u64 value from a document field.
fn extract_u64(doc: &TantivyDocument, field: tantivy::schema::Field) -> u64 {
    doc.get_first(field)
        .and_then(|v| match v {
            OwnedValue::U64(n) => Some(*n),
            _ => None,
        })
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Category, IndexEntry, Role};
    use crate::search::indexer::index_entries;
    use chrono::Utc;
    use tempfile::TempDir;

    fn create_test_entry(content: &str, category: Category) -> IndexEntry {
        IndexEntry {
            role: Role::Ai,
            category,
            content: content.to_string(),
            timestamp: Utc::now(),
            file_path: None,
            line_number: None,
        }
    }

    #[test]
    fn test_search_empty_index() {
        let temp_dir = TempDir::new().unwrap();
        let agit_dir = temp_dir.path().join(".agit");
        std::fs::create_dir_all(&agit_dir).unwrap();

        let results = search(&agit_dir, "authentication", 5).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_with_results() {
        let temp_dir = TempDir::new().unwrap();
        let agit_dir = temp_dir.path().join(".agit");
        std::fs::create_dir_all(&agit_dir).unwrap();

        // Index some entries
        let entries = vec![
            create_test_entry("Planning to implement authentication", Category::Intent),
            create_test_entry("Decided to use JWT tokens for auth", Category::Reasoning),
            create_test_entry("Fixed a bug in the database layer", Category::Note),
        ];
        index_entries(&agit_dir, &entries).unwrap();

        // Search for authentication-related entries
        let results = search(&agit_dir, "authentication", 5).unwrap();
        assert!(!results.is_empty());
        assert!(results[0].body.contains("authentication"));
    }

    #[test]
    fn test_search_respects_limit() {
        let temp_dir = TempDir::new().unwrap();
        let agit_dir = temp_dir.path().join(".agit");
        std::fs::create_dir_all(&agit_dir).unwrap();

        // Index multiple entries with "test" in them
        let entries: Vec<IndexEntry> = (0..10)
            .map(|i| create_test_entry(&format!("Test entry number {}", i), Category::Note))
            .collect();
        index_entries(&agit_dir, &entries).unwrap();

        // Search with limit of 3
        let results = search(&agit_dir, "test", 3).unwrap();
        assert_eq!(results.len(), 3);
    }
}
