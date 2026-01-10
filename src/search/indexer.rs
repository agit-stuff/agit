//! Search indexer for adding entries to the Tantivy index.

use std::path::Path;

use tantivy::doc;

use crate::domain::IndexEntry;
use crate::error::Result;

use super::open_or_create_index;

/// Search indexer that wraps a Tantivy IndexWriter.
pub struct SearchIndexer {
    writer: tantivy::IndexWriter,
    schema: tantivy::schema::Schema,
}

impl SearchIndexer {
    /// Create a new search indexer for the given agit directory.
    ///
    /// Allocates 50MB of heap space for the index writer.
    pub fn new(agit_dir: &Path) -> Result<Self> {
        let index = open_or_create_index(agit_dir)?;
        let schema = index.schema();
        let writer = index.writer(50_000_000)?; // 50MB heap
        Ok(Self { writer, schema })
    }

    /// Add a log entry to the search index.
    pub fn add_entry(&mut self, entry: &IndexEntry) -> Result<()> {
        let id_field = self.schema.get_field("id")?;
        let body_field = self.schema.get_field("body")?;
        let category_field = self.schema.get_field("category")?;
        let timestamp_field = self.schema.get_field("timestamp")?;

        // Use timestamp as unique ID (unix seconds + nanoseconds for uniqueness)
        let timestamp_secs = entry.timestamp.timestamp();
        let timestamp_nanos = entry.timestamp.timestamp_subsec_nanos();
        let id = format!("{}.{}", timestamp_secs, timestamp_nanos);
        let timestamp_u64 = timestamp_secs as u64;

        self.writer.add_document(doc!(
            id_field => id,
            body_field => entry.content.clone(),
            category_field => entry.category.to_string(),
            timestamp_field => timestamp_u64,
        ))?;
        Ok(())
    }

    /// Commit all pending changes to the index.
    pub fn commit(&mut self) -> Result<()> {
        self.writer.commit()?;
        Ok(())
    }
}

/// Index a batch of entries (called from commit pipeline).
///
/// This is the main entry point for indexing entries during commits.
/// Returns early if there are no entries to index.
pub fn index_entries(agit_dir: &Path, entries: &[IndexEntry]) -> Result<()> {
    if entries.is_empty() {
        return Ok(());
    }
    let mut indexer = SearchIndexer::new(agit_dir)?;
    for entry in entries {
        indexer.add_entry(entry)?;
    }
    indexer.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Category, Role};
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
    fn test_index_entries_empty() {
        let temp_dir = TempDir::new().unwrap();
        let agit_dir = temp_dir.path().join(".agit");
        std::fs::create_dir_all(&agit_dir).unwrap();

        // Should succeed with empty entries
        let result = index_entries(&agit_dir, &[]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_index_entries() {
        let temp_dir = TempDir::new().unwrap();
        let agit_dir = temp_dir.path().join(".agit");
        std::fs::create_dir_all(&agit_dir).unwrap();

        let entries = vec![
            create_test_entry("Planning to implement authentication", Category::Intent),
            create_test_entry("Decided to use JWT tokens", Category::Reasoning),
        ];

        let result = index_entries(&agit_dir, &entries);
        assert!(result.is_ok());

        // Verify index was created
        assert!(agit_dir.join("search_index").exists());
    }
}
