//! Domain models for AGIT.
//!
//! This module contains the core data structures that represent
//! the neural graph and its components.

mod index_entry;
mod neural_commit;
mod objects;

pub use index_entry::*;
pub use neural_commit::*;
pub use objects::*;
