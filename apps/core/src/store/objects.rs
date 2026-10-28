//! The store's folder:
//!
//! - `objects/<2 hex>/<2 hex>/<C>`: one encrypted object, named by its hash. Written to a
//!   `.part` file beside it and renamed only once the hash checks out.
//! - `versions/<R>.json`: a stored version's list, `{ objects: [{ hash, size }] }`.
//! - `versions/<R>.pending.<unix seconds>.json`: the list of an upload still under way, which
//!   keeps its objects from cleanup until it completes or [`GRACE_S`] passes.
//! - `versions/<R>.dropped.<unix seconds>.json`: a dropped list, kept [`GRACE_S`] so fetch
//!   tickets issued before the drop still work.
//! - `keys.json`, `egress.json`: see `access::keys` and `limits`.
//!
//! A path is only ever built from a [`Hash`], never from request text.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt, io,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};

/// How long a dropped or abandoned list keeps its objects: the life of a fetch ticket.
pub const GRACE_S: i64 = 6 * 3600;

/// A SHA-256 as 64 lowercase hex characters: an object's name (C) or a version's (R). Sorting
/// the text sorts the bytes.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(try_from = "String", into = "String")]
pub struct Hash(String);

impl Hash {
    pub fn parse(text: &str) -> Option<Self> {
        (text.len() == 64
            && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')))
        .then(|| Self(text.to_owned()))
    }

    pub fn of_digest(digest: &[u8]) -> Self {
        Self(hex::encode(digest))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Hash {
    type Error = &'static str;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        Self::parse(&text).ok_or("a hash is 64 lowercase hex characters")
    }
}

impl From<Hash> for String {
    fn from(hash: Hash) -> Self {
        hash.0
    }
}

impl fmt::Display for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub hash: Hash,
    pub size: u64,
}

/// A version as the store sees it: its objects, sorted by hash.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionList {
    pub objects: Vec<Entry>,
}

impl VersionList {
    /// R: the SHA-256 of the objects' hashes as raw bytes, in order.
    pub fn root(&self) -> Hash {
        let mut hasher = Sha256::new();
        for entry in &self.objects {
            hasher.update(
                hex::decode(entry.hash.as_str()).expect("a valid hash"),
            );
        }
        Hash::of_digest(&hasher.finalize())
    }

    /// Require the list to be sorted, without repeats, and to name `root`.
    pub fn check(&self, root: &Hash) -> Result<(), &'static str> {
        if !self
            .objects
            .windows(2)
            .all(|pair| pair[0].hash < pair[1].hash)
        {
            return Err(
                "The version's objects are not sorted by hash, or repeat.",
            );
        }
        // The root names hashes only, so the sizes are the sender's word: a total that
        // overflows would wrap past every cap.
        if self
            .objects
            .iter()
            .try_fold(0u64, |sum, entry| sum.checked_add(entry.size))
            .is_none()
        {
            return Err("The version's objects are too large.");
        }
        if self.root() != *root {
            return Err("The version's objects do not hash to its root.");
        }
        Ok(())
    }

    pub fn bytes(&self) -> u64 {
        self.objects.iter().map(|entry| entry.size).sum()
    }

    /// The listed size of `hash`, when the list has it.
    pub fn size_of(&self, hash: &Hash) -> Option<u64> {
        self.objects
            .binary_search_by(|entry| entry.hash.cmp(hash))
            .ok()
            .map(|index| self.objects[index].size)
    }
}

/// Paths inside the store's folder.
#[derive(Debug, Clone)]
pub struct Layout {
    root: PathBuf,
}

impl Layout {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn create(&self) -> io::Result<()> {
        std::fs::create_dir_all(self.objects())?;
        std::fs::create_dir_all(self.versions())
    }

    fn objects(&self) -> PathBuf {
        self.root.join("objects")
    }

    fn versions(&self) -> PathBuf {
        self.root.join("versions")
    }

    pub fn keys(&self) -> PathBuf {
        self.root.join("keys.json")
    }

    pub fn egress(&self) -> PathBuf {
        self.root.join("egress.json")
    }

    pub fn object(&self, hash: &Hash) -> PathBuf {
        let name = hash.as_str();
        self.objects().join(&name[..2]).join(&name[2..4]).join(name)
    }

    /// A temporary file for `hash`, unique so two uploads of one object never share it.
    fn part(&self, hash: &Hash) -> PathBuf {
        self.object(hash)
            .with_extension(format!("{}.part", uuid::Uuid::new_v4().simple()))
    }

    pub fn list(&self, root: &Hash, state: ListState) -> PathBuf {
        let name = match state {
            ListState::Stored => format!("{root}.json"),
            ListState::Pending(at) => format!("{root}.pending.{at}.json"),
            ListState::Dropped(at) => format!("{root}.dropped.{at}.json"),
        };
        self.versions().join(name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListState {
    Stored,
    /// Since this unix time.
    Pending(i64),
    Dropped(i64),
}

/// The root and state a list file's name gives.
fn parse_list_name(name: &str) -> Option<(Hash, ListState)> {
    let stem = name.strip_suffix(".json")?;
    let mut parts = stem.split('.');
    let root = Hash::parse(parts.next()?)?;
    let state = match (parts.next(), parts.next(), parts.next()) {
        (None, ..) => ListState::Stored,
        (Some("pending"), Some(at), None) => {
            ListState::Pending(at.parse().ok()?)
        }
        (Some("dropped"), Some(at), None) => {
            ListState::Dropped(at.parse().ok()?)
        }
        _ => return None,
    };
    Some((root, state))
}

/// Every list the store keeps, held in memory and mirrored in `versions/`.
#[derive(Debug, Default)]
pub struct Index {
    pub stored: BTreeMap<Hash, Arc<VersionList>>,
    pub pending: HashMap<Hash, (i64, Arc<VersionList>)>,
    pub dropped: Vec<(Hash, i64, Arc<VersionList>)>,
}

impl Index {
    /// Read `versions/`. A file that does not parse is left alone and logged.
    pub fn load(layout: &Layout) -> io::Result<Self> {
        let mut index = Self::default();
        for file in std::fs::read_dir(layout.versions())? {
            let file = file?;
            let name = file.file_name();
            let Some((root, state)) = name.to_str().and_then(parse_list_name)
            else {
                continue;
            };
            let list = std::fs::read(file.path()).ok().and_then(|bytes| {
                serde_json::from_slice::<VersionList>(&bytes).ok()
            });
            let Some(list) = list.filter(|list| list.check(&root).is_ok())
            else {
                tracing::warn!(file = ?name, "a version list is unreadable; ignored");
                continue;
            };
            let list = Arc::new(list);
            match state {
                ListState::Stored => {
                    index.stored.insert(root, list);
                }
                ListState::Pending(at) => {
                    index.pending.insert(root, (at, list));
                }
                ListState::Dropped(at) => index.dropped.push((root, at, list)),
            }
        }
        Ok(index)
    }

    fn lists(&self) -> impl Iterator<Item = &VersionList> {
        self.stored
            .values()
            .chain(self.pending.values().map(|(_, list)| list))
            .chain(self.dropped.iter().map(|(_, _, list)| list))
            .map(Arc::as_ref)
    }

    /// Bytes of the distinct objects every list names, `extra` included: the disk the store has
    /// promised.
    pub fn used_with(&self, extra: Option<&VersionList>) -> u64 {
        let mut seen = HashSet::new();
        self.lists()
            .chain(extra)
            .flat_map(|list| &list.objects)
            .filter(|entry| seen.insert(&entry.hash))
            .map(|entry| entry.size)
            .sum()
    }

    pub fn used(&self) -> u64 {
        self.used_with(None)
    }

    /// The list a fetch may read: stored, or dropped within the grace period.
    pub fn servable(&self, root: &Hash) -> Option<Arc<VersionList>> {
        self.stored.get(root).cloned().or_else(|| {
            self.dropped
                .iter()
                .find(|(dropped, ..)| dropped == root)
                .map(|(_, _, list)| Arc::clone(list))
        })
    }

    fn referenced(&self) -> HashSet<Hash> {
        self.lists()
            .flat_map(|list| &list.objects)
            .map(|entry| entry.hash.clone())
            .collect()
    }
}

/// Write `bytes` to `path` beside it and rename, so readers see the old file or the new one.
pub async fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let part = path.with_extension("json.part");
    tokio::fs::write(&part, bytes).await?;
    tokio::fs::rename(&part, path).await
}

/// Why an object was not written.
#[derive(Debug)]
pub enum WriteError {
    Io(io::Error),
    /// The bytes were not `size` long or did not hash to the name.
    Mismatch(&'static str),
}

impl From<io::Error> for WriteError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Stream `reader` into the object `hash`, which must be exactly `size` bytes and hash to its
/// name. Nothing is left behind when it does not.
pub async fn write_object(
    layout: &Layout,
    hash: &Hash,
    size: u64,
    reader: impl AsyncRead + Unpin,
) -> Result<(), WriteError> {
    let target = layout.object(hash);
    tokio::fs::create_dir_all(target.parent().expect("objects have a folder"))
        .await?;
    let part = layout.part(hash);
    let result = match copy_hashed(reader, &part, size).await {
        Err(error) => Err(error.into()),
        Ok((written, _)) if written != size => Err(WriteError::Mismatch(
            "An object is not the size its version lists.",
        )),
        Ok((_, digest)) if Hash::of_digest(&digest) != *hash => Err(
            WriteError::Mismatch("An object's bytes do not match its name."),
        ),
        Ok(_) => tokio::fs::rename(&part, &target).await.map_err(Into::into),
    };
    if result.is_err() {
        let _ = tokio::fs::remove_file(&part).await;
    }
    result
}

/// Copy at most `size + 1` bytes into a new file at `path`, hashing them. Returns how many and
/// their hash.
async fn copy_hashed(
    reader: impl AsyncRead + Unpin,
    path: &Path,
    size: u64,
) -> io::Result<(u64, [u8; 32])> {
    let mut reader = reader.take(size + 1);
    let mut file = tokio::fs::File::create(path).await?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 64 * 1024];
    let mut written = 0u64;
    loop {
        let read = reader.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        file.write_all(&buffer[..read]).await?;
        written += read as u64;
    }
    file.flush().await?;
    file.sync_all().await?;
    Ok((written, hasher.finalize().into()))
}

/// The objects of `list` not on disk.
pub async fn missing(
    layout: &Layout,
    list: &VersionList,
) -> io::Result<Vec<Hash>> {
    let mut missing = Vec::new();
    for entry in &list.objects {
        if !tokio::fs::try_exists(layout.object(&entry.hash)).await? {
            missing.push(entry.hash.clone());
        }
    }
    Ok(missing)
}

/// Delete lists past their grace period, then every object no list names. Objects and parts
/// younger than an hour are kept, so an upload racing this never loses what it just wrote.
/// Blocking: run it on a blocking thread.
pub fn collect_garbage(
    layout: &Layout,
    index: &std::sync::Mutex<Index>,
    now: i64,
) -> io::Result<()> {
    let referenced = {
        let mut index = index.lock().expect("not poisoned");
        let mut expired = Vec::new();
        index.dropped.retain(|(root, at, _)| {
            let keep = now - at < GRACE_S;
            if !keep {
                expired.push(layout.list(root, ListState::Dropped(*at)));
            }
            keep
        });
        index.pending.retain(|root, (at, _)| {
            let keep = now - *at < GRACE_S;
            if !keep {
                expired.push(layout.list(root, ListState::Pending(*at)));
            }
            keep
        });
        for path in expired {
            remove(&path)?;
        }
        index.referenced()
    };
    let young = SystemTime::now() - Duration::from_secs(3600);
    for file in walkdir::WalkDir::new(layout.objects())
        .min_depth(3)
        .max_depth(3)
    {
        let file = file.map_err(io::Error::other)?;
        if !file.file_type().is_file() {
            continue;
        }
        let name = file.file_name().to_str().unwrap_or_default();
        if Hash::parse(name).is_some_and(|hash| referenced.contains(&hash)) {
            continue;
        }
        if file.metadata().map_err(io::Error::other)?.modified()? > young {
            continue;
        }
        remove(file.path())?;
    }
    Ok(())
}

fn remove(path: &Path) -> io::Result<()> {
    match std::fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

/// A list of `objects`, sorted, for tests.
#[cfg(test)]
pub fn list_of(objects: &[&[u8]]) -> VersionList {
    let mut objects: Vec<_> = objects
        .iter()
        .map(|bytes| Entry {
            hash: Hash::of_digest(&Sha256::digest(bytes)),
            size: bytes.len() as u64,
        })
        .collect();
    objects.sort_by(|a, b| a.hash.cmp(&b.hash));
    VersionList { objects }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_lowercase_hex_of_the_right_length_is_a_hash() {
        assert!(Hash::parse(&"a".repeat(64)).is_some());
        for bad in [
            "A".repeat(64),
            "a".repeat(63),
            "../".repeat(21) + "a",
            "g".repeat(64),
        ] {
            assert!(Hash::parse(&bad).is_none(), "{bad}");
        }
        assert!(serde_json::from_str::<Hash>("\"../../etc/passwd\"").is_err());
    }

    #[test]
    fn the_root_is_the_hash_of_the_sorted_raw_hashes() {
        assert_eq!(
            VersionList::default().root().as_str(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        let list = list_of(&[b"one", b"two"]);
        let mut raw = Vec::new();
        for entry in &list.objects {
            raw.extend(hex::decode(entry.hash.as_str()).unwrap());
        }
        assert_eq!(list.root(), Hash::of_digest(&Sha256::digest(&raw)));
        assert!(list.check(&list.root()).is_ok());
        let mut unsorted = list.clone();
        unsorted.objects.reverse();
        assert!(unsorted.check(&unsorted.root()).is_err());
        let mut repeated = list.clone();
        repeated.objects.push(repeated.objects[1].clone());
        assert!(repeated.check(&repeated.root()).is_err());
        assert!(list.check(&Hash::parse(&"0".repeat(64)).unwrap()).is_err());
        // Sizes that wrap past u64 would slip under every cap.
        let mut huge = list.clone();
        huge.objects[0].size = 1 << 63;
        huge.objects[1].size = (1 << 63) + 5;
        assert!(huge.check(&huge.root()).is_err());
    }

    #[test]
    fn list_names_round_trip() {
        let layout = Layout::new("store");
        let root = list_of(&[b"x"]).root();
        for state in [
            ListState::Stored,
            ListState::Pending(5),
            ListState::Dropped(7),
        ] {
            let path = layout.list(&root, state);
            let name = path.file_name().unwrap().to_str().unwrap();
            assert_eq!(parse_list_name(name), Some((root.clone(), state)));
        }
        assert_eq!(parse_list_name("x.json"), None);
        assert_eq!(parse_list_name(&format!("{root}.json.part")), None);
    }

    #[test]
    fn used_counts_each_object_once() {
        let mut index = Index::default();
        let a = list_of(&[b"one", b"two"]);
        let b = list_of(&[b"two", b"three"]);
        index.stored.insert(a.root(), Arc::new(a));
        assert_eq!(index.used(), 6);
        assert_eq!(index.used_with(Some(&b)), 11);
    }

    #[tokio::test]
    async fn an_object_is_written_only_when_its_bytes_match() {
        let dir = tempfile::tempdir().unwrap();
        let layout = Layout::new(dir.path());
        layout.create().unwrap();
        let list = list_of(&[b"hello"]);
        let hash = &list.objects[0].hash;
        assert!(matches!(
            write_object(&layout, hash, 5, &b"jello"[..]).await,
            Err(WriteError::Mismatch(_))
        ));
        assert!(matches!(
            write_object(&layout, hash, 5, &b"hello!"[..]).await,
            Err(WriteError::Mismatch(_))
        ));
        assert!(matches!(
            write_object(&layout, hash, 5, &b"hell"[..]).await,
            Err(WriteError::Mismatch(_))
        ));
        assert!(missing(&layout, &list).await.unwrap().len() == 1);
        let folder = layout.object(hash).parent().unwrap().to_owned();
        assert_eq!(
            std::fs::read_dir(&folder).unwrap().count(),
            0,
            "no parts left"
        );
        write_object(&layout, hash, 5, &b"hello"[..]).await.unwrap();
        assert!(missing(&layout, &list).await.unwrap().is_empty());
    }
}
