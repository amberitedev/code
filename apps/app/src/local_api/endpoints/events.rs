use contracts::{Empty, Upgrade};

use crate::local_api::events::LocalEvent;

contracts::endpoints! {
    group events;

    /// Sends every engine and App event; reads nothing.
    EventSocket: Get "/events", None, (), (), Empty => Upgrade<LocalEvent, ()>;
}
