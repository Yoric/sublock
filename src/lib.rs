//! A crate designed to allow batch-locking/batch-unlocking of groups of locks.
//!
//! This crate was initially designed to permit refactoring of code using `RefCell` into `Sync` code.

// Locks for single-treaded use.
pub mod cell;

// Locks for multi-threaded use.
pub mod sync;