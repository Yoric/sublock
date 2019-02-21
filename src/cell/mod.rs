//! Variants of `RefCell` that support sublocks, opened for reading if the main `RefCell` is opened
//! for reading, opened for writing if the main `RefCell` is opened for writing.

/// A variant of `RefCell` with subcells that can be opened at no cost by providing a proof that the
/// main cell is opened.
pub mod proofcell;