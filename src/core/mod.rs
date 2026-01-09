//! Core business logic for AGIT.
//!
//! This module contains the main algorithms and pipelines,
//! separated from I/O concerns for better testability.

mod commit_pipeline;
mod synthesizer;

pub use commit_pipeline::*;
pub use synthesizer::*;
