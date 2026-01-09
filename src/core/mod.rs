//! Core business logic for AGIT.
//!
//! This module contains the main algorithms and pipelines,
//! separated from I/O concerns for better testability.

mod branch_sync;
mod commit_pipeline;
mod synthesizer;

pub use branch_sync::*;
pub use commit_pipeline::*;
pub use synthesizer::*;
