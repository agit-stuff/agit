//! Object envelope types for storage.
//!
//! All objects in the content-addressable store are wrapped in an envelope
//! that includes type information and schema version for forward compatibility.

use serde::{Deserialize, Serialize};

use super::NeuralCommit;

/// Current schema version for all objects.
pub const SCHEMA_VERSION: u32 = 1;

/// The type of an AGIT object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectType {
    /// A neural commit linking git to context.
    NeuralCommit,
    /// A generic blob (roadmap, trace, etc.).
    Blob,
}

/// An envelope wrapper for all stored objects.
///
/// This provides type information and versioning for forward compatibility.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectEnvelope<T> {
    /// The type of object contained.
    pub object_type: ObjectType,
    /// Schema version for migration support.
    pub schema_version: u32,
    /// Unix timestamp when the object was created.
    pub created_at: i64,
    /// The actual object data.
    pub data: T,
}

impl<T> ObjectEnvelope<T> {
    /// Create a new envelope for an object.
    pub fn new(object_type: ObjectType, data: T) -> Self {
        Self {
            object_type,
            schema_version: SCHEMA_VERSION,
            created_at: chrono::Utc::now().timestamp(),
            data,
        }
    }
}

/// A wrapped neural commit ready for storage.
pub type WrappedNeuralCommit = ObjectEnvelope<NeuralCommit>;

impl WrappedNeuralCommit {
    /// Create a new wrapped neural commit.
    pub fn wrap(commit: NeuralCommit) -> Self {
        ObjectEnvelope::new(ObjectType::NeuralCommit, commit)
    }
}

/// Content stored as a blob (roadmap, trace data, etc.).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlobContent {
    /// The content type (e.g., "roadmap", "trace", "state").
    pub content_type: String,
    /// The actual content.
    pub content: String,
}

impl BlobContent {
    /// Create a new blob.
    pub fn new(content_type: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            content_type: content_type.into(),
            content: content.into(),
        }
    }

    /// Create a roadmap blob.
    pub fn roadmap(content: impl Into<String>) -> Self {
        Self::new("roadmap", content)
    }

    /// Create a trace blob.
    pub fn trace(content: impl Into<String>) -> Self {
        Self::new("trace", content)
    }
}

/// A wrapped blob ready for storage.
pub type WrappedBlob = ObjectEnvelope<BlobContent>;

impl WrappedBlob {
    /// Create a new wrapped blob.
    pub fn wrap(blob: BlobContent) -> Self {
        ObjectEnvelope::new(ObjectType::Blob, blob)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_object_envelope() {
        let blob = BlobContent::roadmap("Build the best AI tool");
        let wrapped = WrappedBlob::wrap(blob);

        assert_eq!(wrapped.object_type, ObjectType::Blob);
        assert_eq!(wrapped.schema_version, SCHEMA_VERSION);
        assert_eq!(wrapped.data.content_type, "roadmap");
    }

    #[test]
    fn test_blob_content_types() {
        let roadmap = BlobContent::roadmap("Goals here");
        assert_eq!(roadmap.content_type, "roadmap");

        let trace = BlobContent::trace("Trace data");
        assert_eq!(trace.content_type, "trace");
    }

    #[test]
    fn test_serialization() {
        let blob = BlobContent::roadmap("Test");
        let wrapped = WrappedBlob::wrap(blob);
        let json = serde_json::to_string(&wrapped).unwrap();

        assert!(json.contains("\"object_type\":\"blob\""));
        assert!(json.contains("\"schema_version\":1"));
    }
}
