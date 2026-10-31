//! Updating Copal itself: releases on a channel, signed manifests, versions installed side by
//! side, and the launcher that tries each new one.
//!
//! Applying an update downloads the new version into `versions/<version>/`, checks it against
//! the signed manifest, points `versions/state.json` at it, and exits with
//! [`launcher::RESTART`]. The launcher then runs it on trial (see `launcher`). No running file
//! is overwritten, and servers keep running throughout: their shims do not depend on Core.
//! Routes are in `api::update`.

pub mod launcher;
pub mod release;
pub mod versions;

use std::{
    io,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use semver::Version;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use tokio::{io::AsyncWriteExt, sync::Notify};
use tracing::{info, warn};

use crate::{
    api::update::UpdateStatus, daemon::paths::Paths, servers::Servers,
};
use release::{Channel, Manifest, ManifestError, Release};

/// Where releases are listed when `update_url` is not set.
pub const RELEASES: &str =
    "https://api.github.com/repos/amberitedev/code/releases?per_page=100";
/// How long after start the first check waits, so a Core that just updated settles first.
const FIRST_CHECK: Duration = Duration::from_secs(5 * 60);
const CHECK_EVERY: Duration = Duration::from_secs(24 * 60 * 60);
/// How long auto-update waits for servers to finish starting or stopping.
const SETTLE_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const METADATA_TIMEOUT: Duration = Duration::from_secs(30);
/// The most Core reads of each answer before it has checked a signature, so a hostile server
/// cannot fill its memory.
const MAX_LIST_PAGE: usize = 1024 * 1024;
const MAX_MANIFEST: usize = 64 * 1024;
const MAX_SIGNATURE: usize = 4 * 1024;
/// Pages of the releases list read at most, 100 releases each.
const MAX_PAGES: usize = 5;
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// The version of this binary.
pub fn current() -> Version {
    Version::parse(env!("CARGO_PKG_VERSION"))
        .expect("the crate version is semver")
}

/// Why an update did not happen.
#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    #[error(
        "{0} is older than this Core ({1}). Downgrading needs copal update apply --allow-downgrade"
    )]
    Downgrade(Version, Version),
    #[error("only the copal CLI on this machine can downgrade Core")]
    DowngradeRemote,
    #[error("Copal {0} is already running")]
    UpToDate(Version),
    #[error(
        "Copal {0} was tried here and Core went back: it {1}. To try it again: copal update apply --retry"
    )]
    FailedBefore(Version, &'static str),
    #[error("there is no Copal release on the {0:?} channel")]
    NoRelease(Channel),
    #[error("Core is still checking its last update. Try again in a minute")]
    OnTrial,
    #[error("there is no previous version to go back to")]
    NothingToRollBackTo,
    #[error("the release has no {0}")]
    MissingAsset(String),
    #[error("the download does not match the signed manifest")]
    Mismatch,
    #[error("the release server sent more than {0} bytes for one answer")]
    TooLarge(usize),
    #[error(transparent)]
    Manifest(#[from] ManifestError),
    #[error("could not reach the release server: {0}")]
    Network(#[from] reqwest::Error),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

/// Whether `next` may replace the running `current`: never itself, and an older version only
/// with `allow_downgrade`, so a validly signed old manifest cannot push Core back to a buggy
/// build.
fn may_replace(
    next: &Version,
    current: &Version,
    allow_downgrade: bool,
) -> Result<(), UpdateError> {
    if next == current {
        Err(UpdateError::UpToDate(current.clone()))
    } else if next < current && !allow_downgrade {
        Err(UpdateError::Downgrade(next.clone(), current.clone()))
    } else {
        Ok(())
    }
}

/// Whether `version` is the release that last failed its trial here.
fn failed_before(version: &Version, failed: Option<&str>) -> bool {
    failed.is_some_and(|failed| {
        Version::parse(failed).ok().as_ref() == Some(version)
    })
}

/// Read an answer as it streams in, refusing it once it passes `cap` bytes.
async fn read_capped(
    mut response: reqwest::Response,
    cap: usize,
) -> Result<Vec<u8>, UpdateError> {
    if response
        .content_length()
        .is_some_and(|length| length > cap as u64)
    {
        return Err(UpdateError::TooLarge(cap));
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if body.len() + chunk.len() > cap {
            return Err(UpdateError::TooLarge(cap));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

/// The last check's answer.
#[derive(Debug, Clone, Default)]
struct Checked {
    at: Option<i64>,
    latest: Option<Version>,
    error: Option<String>,
}

/// A release ready to install.
struct Candidate {
    version: Version,
    file: release::File,
    url: String,
}

/// The updater. One per Core.
pub struct Updater {
    paths: Paths,
    db: SqlitePool,
    http: reqwest::Client,
    /// The releases list: GitHub's, or anything that serves its shape.
    source: String,
    public_key: String,
    /// Notified when Core should exit so the launcher runs the active version.
    pub restart: Arc<Notify>,
    checked: Mutex<Checked>,
    /// Held while a version is installed or removed.
    busy: tokio::sync::Mutex<()>,
    answered: AtomicBool,
}

impl Updater {
    pub fn new(
        paths: Paths,
        db: SqlitePool,
        http: reqwest::Client,
        source: String,
    ) -> Self {
        // Development builds can trust a local test key, to try updates against releases
        // built and signed on this machine. Release builds trust only the release key.
        #[cfg(debug_assertions)]
        let public_key = std::env::var("COPAL_UPDATE_PUBLIC_KEY")
            .unwrap_or_else(|_| release::PUBLIC_KEY.into());
        #[cfg(not(debug_assertions))]
        let public_key = release::PUBLIC_KEY.to_owned();
        Self {
            paths,
            db,
            http,
            source,
            public_key,
            restart: Arc::new(Notify::new()),
            checked: Mutex::default(),
            busy: tokio::sync::Mutex::new(()),
            answered: AtomicBool::new(false),
        }
    }

    /// Core answered a request. The first time, write this version's health marker, which ends
    /// its trial (see `launcher`).
    pub fn answered(&self) {
        if self.answered.swap(true, Ordering::Relaxed) {
            return;
        }
        let version = current().to_string();
        if self.paths.version(&version).is_dir()
            && let Err(error) =
                std::fs::write(versions::marker(&self.paths, &version), "")
        {
            warn!(%error, "could not write the health marker");
        }
    }

    /// The channel and whether updates install by themselves.
    pub async fn settings(&self) -> sqlx::Result<(Channel, bool)> {
        let row: Option<(Channel, bool)> = sqlx::query_as(
            "SELECT update_channel, auto_update FROM core WHERE id = 1",
        )
        .fetch_optional(&self.db)
        .await?;
        Ok(row.unwrap_or((Channel::Stable, true)))
    }

    pub async fn set_settings(
        &self,
        channel: Option<Channel>,
        auto_update: Option<bool>,
    ) -> sqlx::Result<()> {
        sqlx::query(
            "UPDATE core SET update_channel = coalesce(?, update_channel), \
             auto_update = coalesce(?, auto_update) WHERE id = 1",
        )
        .bind(channel)
        .bind(auto_update)
        .execute(&self.db)
        .await?;
        if channel.is_some() {
            *self.checked.lock().expect("not poisoned") = Checked::default();
        }
        Ok(())
    }

    pub async fn status(&self) -> Result<UpdateStatus, UpdateError> {
        let (channel, auto_update) = self.settings().await?;
        let checked = self.checked.lock().expect("not poisoned").clone();
        let state = versions::load(&self.paths)?;
        let version = current();
        let failed = state.as_ref().and_then(|state| state.failed.clone());
        Ok(UpdateStatus {
            version: version.to_string(),
            target: release::TARGET.map(Into::into),
            channel,
            auto_update,
            update_available: checked.latest.as_ref().is_some_and(|latest| {
                latest > &version && !failed_before(latest, failed.as_deref())
            }),
            latest: checked.latest.map(|latest| latest.to_string()),
            checked_at: checked.at,
            check_error: checked.error,
            previous: state.as_ref().and_then(|state| state.previous.clone()),
            failed,
            failed_reason: state.and_then(|state| state.failed_reason),
        })
    }

    /// Look up the newest release on the channel.
    pub async fn check(&self) -> Result<UpdateStatus, UpdateError> {
        let (channel, _) = self.settings().await?;
        let found = self.candidate(channel).await;
        {
            let mut checked = self.checked.lock().expect("not poisoned");
            checked.at = Some(chrono::Utc::now().timestamp());
            match &found {
                Ok(candidate) => {
                    checked.latest = Some(candidate.version.clone());
                    checked.error = None;
                }
                Err(error) => checked.error = Some(error.to_string()),
            }
        }
        found?;
        self.status().await
    }

    /// Install the newest release on the channel and restart into it. Returns its version.
    /// `allow_downgrade` lets an older release replace this one; only the CLI may pass it.
    /// `retry` installs the release even when it already failed its trial here.
    pub async fn apply(
        &self,
        allow_downgrade: bool,
        retry: bool,
    ) -> Result<Version, UpdateError> {
        let _busy = self.busy.lock().await;
        let state = versions::load(&self.paths)?;
        if state.as_ref().is_some_and(|state| state.trial.is_some()) {
            return Err(UpdateError::OnTrial);
        }
        let (channel, _) = self.settings().await?;
        let candidate = self.candidate(channel).await?;
        let current = current();
        may_replace(&candidate.version, &current, allow_downgrade)?;
        if !retry
            && let Some(state) = &state
            && failed_before(&candidate.version, state.failed.as_deref())
        {
            return Err(UpdateError::FailedBefore(
                candidate.version,
                state.failed_reason.map_or(
                    "it did not prove healthy",
                    versions::FailedTrial::describe,
                ),
            ));
        }
        self.install(&candidate).await?;
        let paths = self.paths.clone();
        let running = current.to_string();
        tokio::task::spawn_blocking(move || {
            versions::keep_running(&paths, &running)
        })
        .await
        .expect("the copy does not panic")?;
        let next = candidate.version.to_string();
        versions::save(
            &self.paths,
            Some(&versions::State::update(
                &current.to_string(),
                &next,
                state.as_ref(),
            )),
        )?;
        info!(from = %current, to = %next, "updated; restarting into the new version");
        self.restart_soon();
        Ok(candidate.version)
    }

    /// Go back to the previous version, with the database from before the update, and
    /// restart into it. Returns its version.
    pub async fn roll_back(&self) -> Result<String, UpdateError> {
        let _busy = self.busy.lock().await;
        let state = versions::load(&self.paths)?
            .ok_or(UpdateError::NothingToRollBackTo)?;
        if state.trial.is_some() {
            return Err(UpdateError::OnTrial);
        }
        let back = state
            .roll_back()
            .filter(|back| {
                versions::binary(&self.paths, &back.active).exists()
                    && self.paths.database_before_update().is_dir()
            })
            .ok_or(UpdateError::NothingToRollBackTo)?;
        versions::save(&self.paths, Some(&back))?;
        info!(to = %back.active, "rolling back");
        self.restart_soon();
        Ok(back.active)
    }

    /// Check once a day, and install what the check finds when auto-update is on.
    pub async fn auto(self: Arc<Self>, servers: Arc<Servers>) {
        tokio::time::sleep(FIRST_CHECK).await;
        self.clean().await;
        loop {
            match self.check().await {
                Ok(status) => {
                    // `update_available` is false for a release that failed its trial here.
                    if status.auto_update && status.update_available {
                        servers.settle(SETTLE_TIMEOUT).await;
                        if let Err(error) = self.apply(false, false).await {
                            warn!(%error, "could not update");
                        }
                    }
                }
                Err(error) => warn!(%error, "could not check for updates"),
            }
            tokio::time::sleep(CHECK_EVERY).await;
        }
    }

    /// Remove versions no one goes back to.
    async fn clean(&self) {
        let _busy = self.busy.lock().await;
        let running = current().to_string();
        let state = versions::load(&self.paths).ok().flatten();
        let mut keep = vec![running.as_str()];
        if let Some(state) = &state {
            keep.push(&state.active);
            keep.extend(state.previous.as_deref());
        }
        versions::clean(&self.paths, &keep);
    }

    /// Exit soon, once the answer to the request that asked for it is out.
    fn restart_soon(&self) {
        let restart = Arc::clone(&self.restart);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(500)).await;
            restart.notify_one();
        });
    }

    /// The newest release on `channel`, with its manifest verified.
    async fn candidate(
        &self,
        channel: Channel,
    ) -> Result<Candidate, UpdateError> {
        let releases = self.releases(channel).await?;
        let (version, release) = release::resolve(&releases, channel)
            .ok_or(UpdateError::NoRelease(channel))?;
        let asset = |name: &str| {
            release
                .asset(name)
                .ok_or_else(|| UpdateError::MissingAsset(name.into()))
        };
        let manifest =
            self.fetch(asset(release::MANIFEST)?, MAX_MANIFEST).await?;
        let signature = self
            .fetch(asset(release::SIGNATURE)?, MAX_SIGNATURE)
            .await?;
        let manifest = Manifest::verify(
            &manifest,
            &String::from_utf8_lossy(&signature),
            &self.public_key,
        )?;
        let file = manifest.file(&version, release::TARGET)?.clone();
        Ok(Candidate {
            url: asset(&file.name)?.to_owned(),
            version,
            file,
        })
    }

    /// The releases list, newest first, page by page until one has a release on `channel`.
    /// The Modrinth App's releases share the list, so the newest Copal release can be past the
    /// first page.
    async fn releases(
        &self,
        channel: Channel,
    ) -> Result<Vec<Release>, UpdateError> {
        let mut releases = Vec::new();
        let mut next = Some(self.source.clone());
        for _ in 0..MAX_PAGES {
            let Some(url) = next.take() else { break };
            let response = self
                .http
                .get(&url)
                .header("accept", "application/vnd.github+json")
                .timeout(METADATA_TIMEOUT)
                .send()
                .await?
                .error_for_status()?;
            next = response
                .headers()
                .get("link")
                .and_then(|link| link.to_str().ok())
                .and_then(release::next_page);
            let page = read_capped(response, MAX_LIST_PAGE).await?;
            releases.extend(
                serde_json::from_slice::<Vec<Release>>(&page).map_err(
                    |error| {
                        UpdateError::Manifest(ManifestError::Invalid(
                            error.to_string(),
                        ))
                    },
                )?,
            );
            if release::resolve(&releases, channel).is_some() {
                break;
            }
        }
        Ok(releases)
    }

    async fn fetch(
        &self,
        url: &str,
        cap: usize,
    ) -> Result<Vec<u8>, UpdateError> {
        let response = self
            .http
            .get(url)
            .timeout(METADATA_TIMEOUT)
            .send()
            .await?
            .error_for_status()?;
        read_capped(response, cap).await
    }

    /// Download a version into `versions/<version>/` and check it against the manifest. An
    /// installed copy that matches is kept.
    async fn install(&self, candidate: &Candidate) -> Result<(), UpdateError> {
        let version = candidate.version.to_string();
        let binary = versions::binary(&self.paths, &version);
        if std::fs::read(&binary).is_ok_and(|bytes| {
            hex::encode(Sha256::digest(bytes)) == candidate.file.sha256
        }) {
            return Ok(());
        }
        let staging = self.paths.versions().join(format!("{version}.partial"));
        match std::fs::remove_dir_all(&staging) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => {
                return Err(error.into());
            }
            _ => {}
        }
        std::fs::create_dir_all(&staging)?;
        let staged =
            staging.join(binary.file_name().expect("a binary has a name"));
        let downloaded = self.download(candidate, &staged).await;
        if let Err(error) = downloaded {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(error);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                &staged,
                std::fs::Permissions::from_mode(0o750),
            )?;
        }
        let folder = self.paths.version(&version);
        match std::fs::remove_dir_all(&folder) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => {
                return Err(error.into());
            }
            _ => {}
        }
        std::fs::rename(&staging, &folder)?;
        Ok(())
    }

    async fn download(
        &self,
        candidate: &Candidate,
        to: &std::path::Path,
    ) -> Result<(), UpdateError> {
        let mut response = self
            .http
            .get(&candidate.url)
            .timeout(DOWNLOAD_TIMEOUT)
            .send()
            .await?
            .error_for_status()?;
        let mut file = tokio::fs::File::create(to).await?;
        let mut hash = Sha256::new();
        let mut size = 0;
        while let Some(chunk) = response.chunk().await? {
            size += chunk.len() as u64;
            if size > candidate.file.size {
                return Err(UpdateError::Mismatch);
            }
            hash.update(&chunk);
            file.write_all(&chunk).await?;
        }
        file.sync_all().await?;
        if size != candidate.file.size
            || hex::encode(hash.finalize()) != candidate.file.sha256
        {
            return Err(UpdateError::Mismatch);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(text: &str) -> Version {
        Version::parse(text).unwrap()
    }

    /// Answer one request with `body`, saying its length only when `say_length`.
    async fn serve_once(body: Vec<u8>, say_length: bool) -> String {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener =
            tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0; 1024];
            let _ = stream.read(&mut request).await;
            let length = if say_length {
                format!("Content-Length: {}\r\n", body.len())
            } else {
                String::new()
            };
            let head =
                format!("HTTP/1.1 200 OK\r\n{length}Connection: close\r\n\r\n");
            let _ = stream.write_all(head.as_bytes()).await;
            let _ = stream.write_all(&body).await;
        });
        format!("http://{address}/")
    }

    #[tokio::test]
    async fn answers_past_their_cap_are_refused_before_they_are_read_whole() {
        let http = crate::network::http().build().unwrap();
        for say_length in [true, false] {
            let url =
                serve_once(vec![b'x'; MAX_SIGNATURE + 1], say_length).await;
            let response = http.get(url).send().await.unwrap();
            assert!(matches!(
                read_capped(response, MAX_SIGNATURE).await,
                Err(UpdateError::TooLarge(MAX_SIGNATURE))
            ));
        }
        let url = serve_once(b"small".to_vec(), false).await;
        let response = http.get(url).send().await.unwrap();
        assert_eq!(
            read_capped(response, MAX_SIGNATURE).await.unwrap(),
            b"small"
        );
    }

    #[test]
    fn a_release_that_failed_here_is_known_by_its_version() {
        assert!(failed_before(&version("0.3.0"), Some("0.3.0")));
        assert!(!failed_before(&version("0.4.0"), Some("0.3.0")));
        assert!(!failed_before(&version("0.3.0"), None));
    }

    #[test]
    fn a_downgrade_is_refused_unless_allowed() {
        let current = version("0.3.0");
        assert!(may_replace(&version("0.4.0"), &current, false).is_ok());
        assert!(matches!(
            may_replace(&version("0.2.0"), &current, false),
            Err(UpdateError::Downgrade(..))
        ));
        // A pre-release of the running version is older than it.
        assert!(matches!(
            may_replace(&version("0.3.0-beta.1"), &current, false),
            Err(UpdateError::Downgrade(..))
        ));
        assert!(may_replace(&version("0.2.0"), &current, true).is_ok());
        assert!(matches!(
            may_replace(&current, &current, true),
            Err(UpdateError::UpToDate(_))
        ));
    }
}
