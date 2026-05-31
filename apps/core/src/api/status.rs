//! The lifecycle of a single delivery.

use serde::{Deserialize, Serialize};

/// Where a message is in its journey to one recipient. Terminal states let Core
/// stop tracking the row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeliveryStatus {
    Pending,
    Delivering,
    Received,
    Processed,
    Expired,
    Failed,
}

impl DeliveryStatus {
    pub fn wire(self) -> &'static str {
        match self {
            DeliveryStatus::Pending => "pending",
            DeliveryStatus::Delivering => "delivering",
            DeliveryStatus::Received => "received",
            DeliveryStatus::Processed => "processed",
            DeliveryStatus::Expired => "expired",
            DeliveryStatus::Failed => "failed",
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            DeliveryStatus::Processed | DeliveryStatus::Expired | DeliveryStatus::Failed
        )
    }
}
