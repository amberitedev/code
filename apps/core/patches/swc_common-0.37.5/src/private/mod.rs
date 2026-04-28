//! This module is private module and can be changed without notice.

// Patched: serde::__private was removed in serde >= 1.0.220.
// This re-export is unused in our dependency tree, so provide an empty shim.
pub mod serde {}
