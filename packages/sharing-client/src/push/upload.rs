use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use contracts::Hash;
use contracts::store::{ListedObject, VersionList};
use contracts::tickets::StoreHost;

use super::{Object, PushError, PushEvent};
use crate::pool::StoreClient;

/// Uploads rounds of whatever the Core still lacks until it holds the whole version.
const UPLOAD_ROUNDS: usize = 3;
const UPLOAD_PROGRESS_INTERVAL: Duration = Duration::from_millis(200);

/// Stores a version's objects on `core`, sending only those it lacks, and returns the hash of each
/// object it sent. Every object of a push is on disk, so one the Core lost since an earlier
/// version is sent again.
pub(super) async fn store_version(
    store: &StoreClient,
    core: &StoreHost,
    ticket: &str,
    root: Hash,
    objects: &[Object],
    on_event: &mut (dyn FnMut(PushEvent<'_>) + Send),
) -> Result<HashSet<Hash>, PushError> {
    let paths = objects
        .iter()
        .map(|object| (Hash(object.object), object.path.clone()))
        .collect::<HashMap<_, _>>();
    let mut list = objects
        .iter()
        .map(|object| ListedObject {
            hash: Hash(object.object),
            size: object.size,
        })
        .collect::<Vec<_>>();
    list.sort_by_key(|object| object.hash);
    list.dedup_by_key(|object| object.hash);
    let hashes = list.iter().map(|object| object.hash).collect::<Vec<_>>();
    let list = VersionList { objects: list };

    let have = store.have(core, ticket, root, hashes).await?;
    if have.complete {
        return Ok(HashSet::new());
    }
    let mut missing = have.missing;
    let requested = missing.iter().copied().collect::<HashSet<_>>();
    let total = list
        .objects
        .iter()
        .filter(|object| requested.contains(&object.hash))
        .map(|object| object.size)
        .sum();
    let uploaded = Arc::new(AtomicU64::new(0));
    on_event(PushEvent::Progress(0, total));
    for _ in 0..UPLOAD_ROUNDS {
        let sending = missing
            .iter()
            .filter_map(|hash| Some((*hash, paths.get(hash)?.clone())))
            .collect::<Vec<_>>();
        let upload = store.upload(
            core,
            ticket,
            root,
            &list,
            sending,
            Arc::clone(&uploaded),
        );
        tokio::pin!(upload);
        let mut ticker = tokio::time::interval(UPLOAD_PROGRESS_INTERVAL);
        let outcome = loop {
            tokio::select! {
                outcome = &mut upload => break outcome,
                _ = ticker.tick() => on_event(PushEvent::Progress(
                    uploaded.load(Ordering::Relaxed).min(total),
                    total,
                )),
            }
        }?;
        if outcome.complete {
            on_event(PushEvent::Progress(total, total));
            return Ok(requested);
        }
        missing = outcome.missing;
    }
    Err(PushError::NotAccepted)
}
