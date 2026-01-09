//! Safety primitives for AGIT.
//!
//! This module provides atomic write operations and file locking
//! to ensure data integrity during concurrent operations.

mod atomic;
mod lock;

pub use atomic::*;
pub use lock::*;
