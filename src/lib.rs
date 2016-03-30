//! Variants of `RwLock` that support sublocks, opened for reading if the main `RwLock` is opened
//! for reading, opened for writing if the main `RwLock` is opened for writing.
//!
//! This crate has been designed to permit refactoring of code using `RefCell` into `Sync` code.

/// A variant of `RwLock` based on dynamic checks (comparable to `RefCell`).
pub mod atomlock;

/// A variant of `RwLock` based on proofs of opening. Faster and safer than `atomlock`, but
/// a bit more verbose.
pub mod prooflock;

