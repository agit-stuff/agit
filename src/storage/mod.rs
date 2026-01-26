//! Storage layer for AGIT.
//!
//! This module provides trait-based abstractions for storage operations,
//! making the core logic testable and the storage implementation swappable.
//!
//! ## Storage Implementations
//!
//! - **File-based (V1)**: Objects in `.agit/objects/`, refs in `.agit/refs/heads/`
//! - **Git-native (V2)**: Objects in Git ODB, refs in `refs/agit/heads/`
//!
//! ## Merge-Friendly Memory Storage
//!
//! The `memory` module provides immutable object storage for memory nodes,
//! designed to eliminate merge conflicts by storing each memory as a separate file.

mod cas;
mod git_objects;
mod git_refs;
mod index;
mod memory;
mod refs;
mod traits;

pub use cas::*;
pub use git_objects::*;
pub use git_refs::*;
pub use index::*;
pub use memory::*;
pub use refs::*;
pub use traits::*;
