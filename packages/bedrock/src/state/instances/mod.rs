pub mod model;
pub use self::model::*;

// The instance rows the content store clone reads, from its glue.
pub(crate) use crate::upstream::store::instances::*;
