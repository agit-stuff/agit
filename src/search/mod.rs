//! Full-text search module using Tantivy.
//!
//! This module provides the "Retriever" layer that enables fuzzy-searching
//! past reasoning logs instead of reading the entire history.
//!
//! The search index is stored in `.agit/search_index/` and is local-only.
//! Use `agit search rebuild` to regenerate the index from existing neural commits.
//!
//! After `agit pull`, the index is automatically updated with newly fetched commits.

pub mod incremental;
pub mod index_state;
pub mod indexer;
pub mod retriever;

use std::path::Path;

use tantivy::schema::{NumericOptions, Schema, STORED, STRING, TEXT};
use tantivy::Index;

use crate::error::Result;

/// Build the Tantivy schema for search documents.
///
/// Fields:
/// - `id`: Unique identifier (timestamp as string), stored only
/// - `body`: The content to search, full-text indexed
/// - `category`: Category for filtering (intent, reasoning, error, note)
/// - `timestamp`: Unix timestamp for sorting/filtering
pub fn build_schema() -> Schema {
    let mut schema_builder = Schema::builder();
    schema_builder.add_text_field("id", STORED);
    schema_builder.add_text_field("body", TEXT | STORED);
    schema_builder.add_text_field("category", STRING | STORED);
    schema_builder.add_u64_field("timestamp", NumericOptions::default().set_stored());
    schema_builder.build()
}

/// Open an existing index or create a new one.
///
/// The index is stored in `.agit/search_index/`.
pub fn open_or_create_index(agit_dir: &Path) -> Result<Index> {
    let index_path = agit_dir.join("search_index");
    if index_path.exists() {
        Ok(Index::open_in_dir(&index_path)?)
    } else {
        std::fs::create_dir_all(&index_path)?;
        Ok(Index::create_in_dir(&index_path, build_schema())?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_build_schema() {
        let schema = build_schema();
        assert!(schema.get_field("id").is_ok());
        assert!(schema.get_field("body").is_ok());
        assert!(schema.get_field("category").is_ok());
        assert!(schema.get_field("timestamp").is_ok());
    }

    #[test]
    fn test_open_or_create_index() {
        let temp_dir = TempDir::new().unwrap();
        let agit_dir = temp_dir.path().join(".agit");
        std::fs::create_dir_all(&agit_dir).unwrap();

        // First call should create the index
        let index = open_or_create_index(&agit_dir).unwrap();
        assert!(agit_dir.join("search_index").exists());

        // Second call should open the existing index
        let _index2 = open_or_create_index(&agit_dir).unwrap();
        assert_eq!(index.schema(), build_schema());
    }
}
