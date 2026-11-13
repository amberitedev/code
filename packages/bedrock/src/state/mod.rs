//! The engine's content schema, compiled from the engine's own files so both sides share one
//! definition. The module paths copy the engine's, so the borrowed files resolve unchanged.

#[path = "../../../engine/src/state/instance_types.rs"]
pub mod instance_types;
pub use self::instance_types::*;

pub mod instances;
pub use self::instances::*;
