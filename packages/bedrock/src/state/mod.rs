//! app-lib's content schema, compiled from app-lib's own files so both sides share one
//! definition. The module paths copy app-lib's, so the borrowed files resolve unchanged.

#[path = "../../../app-lib/src/state/instance_types.rs"]
pub mod instance_types;
pub use self::instance_types::*;

pub mod instances;
pub use self::instances::*;
