//! What keeps a leaked ticket or a scripted install from draining a volunteer's uplink: a daily
//! egress cap, a cap on fetches at once, and a per-address rate of refused requests.

use std::{
    collections::HashMap,
    net::IpAddr,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// Object fetches streaming at once.
pub const CONCURRENT_FETCHES: usize = 32;
/// Refused requests one address may make a minute before it is turned away unheard. Requests
/// with a valid ticket never count, so an install fetching thousands of files is never slowed.
pub const REFUSALS_PER_MINUTE: u32 = 120;

/// Bytes served on one UTC day, as `egress.json` keeps them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Egress {
    /// `YYYY-MM-DD`, UTC.
    day: String,
    bytes: u64,
}

pub struct Limits {
    fetches: Arc<Semaphore>,
    addresses: Mutex<HashMap<IpAddr, (Instant, u32)>>,
    egress: Mutex<Egress>,
    egress_file: PathBuf,
}

/// Today in UTC, as `egress.json` names days.
fn today() -> String {
    chrono::Utc::now().format("%Y-%m-%d").to_string()
}

impl Limits {
    /// Limits with today's egress so far from `egress_file`, which need not exist.
    pub fn load(egress_file: PathBuf) -> Self {
        let egress = std::fs::read(&egress_file)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self {
            fetches: Arc::new(Semaphore::new(CONCURRENT_FETCHES)),
            addresses: Mutex::default(),
            egress: Mutex::new(egress),
            egress_file,
        }
    }

    /// Whether `address` has used up this minute's [`REFUSALS_PER_MINUTE`].
    pub fn limited(&self, address: IpAddr) -> bool {
        let mut addresses = self.addresses.lock().expect("not poisoned");
        addresses
            .retain(|_, (start, _)| start.elapsed() < Duration::from_secs(60));
        addresses
            .get(&address)
            .is_some_and(|(_, count)| *count >= REFUSALS_PER_MINUTE)
    }

    /// Count a refused request from `address`.
    pub fn refused(&self, address: IpAddr) {
        let mut addresses = self.addresses.lock().expect("not poisoned");
        let (_, count) =
            addresses.entry(address).or_insert((Instant::now(), 0));
        *count += 1;
    }

    /// One of the fetch slots, held until the response is sent. `None` when all are taken.
    pub fn fetch_slot(&self) -> Option<OwnedSemaphorePermit> {
        Arc::clone(&self.fetches).try_acquire_owned().ok()
    }

    /// Bytes served today.
    pub fn egress_today(&self) -> u64 {
        let today = today();
        let egress = self.egress.lock().expect("not poisoned");
        if egress.day == today { egress.bytes } else { 0 }
    }

    /// Count `bytes` served now.
    pub fn spend(&self, bytes: u64) {
        let today = today();
        let mut egress = self.egress.lock().expect("not poisoned");
        if egress.day != today {
            *egress = Egress {
                day: today,
                bytes: 0,
            };
        }
        egress.bytes += bytes;
    }

    /// Write today's egress to `egress.json`, so a restart does not reset the cap.
    pub async fn save(&self) {
        let egress = self.egress.lock().expect("not poisoned").clone();
        let json = serde_json::to_vec(&egress).expect("egress serializes");
        if let Err(error) =
            super::objects::write_atomic(&self.egress_file, &json).await
        {
            tracing::warn!(%error, "could not save today's egress");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_address_gets_its_minute_s_refusals_and_no_more() {
        let limits = Limits::load(PathBuf::from("missing.json"));
        let one = IpAddr::from([203, 0, 113, 1]);
        for _ in 0..REFUSALS_PER_MINUTE {
            assert!(!limits.limited(one));
            limits.refused(one);
        }
        assert!(limits.limited(one));
        assert!(!limits.limited(IpAddr::from([203, 0, 113, 2])));
    }

    #[test]
    fn fetch_slots_run_out() {
        let limits = Limits::load(PathBuf::from("missing.json"));
        let held: Vec<_> = (0..CONCURRENT_FETCHES)
            .map(|_| limits.fetch_slot().unwrap())
            .collect();
        assert!(limits.fetch_slot().is_none());
        drop(held);
        assert!(limits.fetch_slot().is_some());
    }

    #[tokio::test]
    async fn egress_survives_a_restart_and_resets_the_next_day() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("egress.json");
        let limits = Limits::load(path.clone());
        limits.spend(10);
        limits.spend(5);
        limits.save().await;
        assert_eq!(Limits::load(path.clone()).egress_today(), 15);
        std::fs::write(&path, r#"{"day":"2000-01-01","bytes":99}"#).unwrap();
        let yesterday = Limits::load(path);
        assert_eq!(yesterday.egress_today(), 0);
        yesterday.spend(1);
        assert_eq!(yesterday.egress_today(), 1);
    }
}
