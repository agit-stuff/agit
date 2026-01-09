//! Storage layer for AGIT.
//!
//! This module provides trait-based abstractions for storage operations,
//! making the core logic testable and the storage implementation swappable.

mod cas;
mod index;
mod refs;
mod traits;

pub use cas::*;
pub use index::*;
pub use refs::*;
pub use traits::*;
