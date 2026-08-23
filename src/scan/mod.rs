//! Reads and returns suggestions. **Nothing in here writes to disk** (invariant 1).

// ponytail: `collect` is the caller and lands later in M2. Delete this when it does.
#![allow(dead_code)]

pub mod refs;
