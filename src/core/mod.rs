//! Core business logic for AGIT.
//!
//! This module contains the main algorithms and pipelines,
//! separated from I/O concerns for better testability.

pub mod branch_sync;
pub mod commit_pipeline;
pub mod synthesizer;

pub use branch_sync::*;
pub use commit_pipeline::*;
pub use synthesizer::*;
