//! Safety primitives for AGIT.
//!
//! This module provides atomic write operations, file locking,
//! and path validation to ensure data integrity and security.

mod atomic;
mod lock;
mod paths;

pub use atomic::*;
pub use lock::*;
pub use paths::*;
