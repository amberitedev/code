//! Owner recovery copies and retry transport, kept separate from upstream sharing.
//! A committed journal always refers to immutable local bytes. Account sessions
//! never go to storage nodes; those receive only short-lived upload capabilities.
use crate::state::{ContentSetSyncStatus, State};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::{
    LazyLock,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::Mutex;

static STARTED: AtomicBool = AtomicBool::new(false);
static TRANSFER: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

pub(crate) struct SnapshotFile {
    pub name: String,
    pub kind: String,
    pub path: std::path::PathBuf,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
struct SavedFile {
    name: String,
    kind: String,
    sha256: String,
    size: u64,
}

#[derive(Serialize, Deserialize)]
struct Journal {
    id: String,
    account: String,
    origin: String,
    instance: String,
    shared_instance: String,
    request: serde_json::Value,
    files: Vec<SavedFile>,
    response: Option<Version>,
}

#[derive(Serialize, Deserialize)]
struct Version {
    version: i32,
    external_files: Vec<Upload>,
}

#[derive(Serialize, Deserialize)]
struct Upload {
    file_name: String,
    file_type: String,
    url: String,
}

#[derive(Deserialize)]
struct UploadState {
    status: String,
    upload_url: Option<String>,
    sha256: Option<String>,
    size: Option<u64>,
}

fn error(message: &str) -> crate::Error {
    crate::ErrorKind::OtherError(message.to_string()).into()
}

async fn initialize(pool: &sqlx::SqlitePool) -> crate::Result<()> {
    sqlx::query("CREATE TABLE IF NOT EXISTS self_hosted_uploads (id TEXT PRIMARY KEY, account TEXT NOT NULL, origin TEXT NOT NULL, instance TEXT NOT NULL, manifest TEXT NOT NULL, uploaded INTEGER NOT NULL DEFAULT 0, reconciled INTEGER NOT NULL DEFAULT 0, checked_at INTEGER NOT NULL DEFAULT 0, created INTEGER NOT NULL DEFAULT (unixepoch()))")
        .execute(pool).await?;
    Ok(())
}

fn same_snapshot(
    journal: &Journal,
    shared_instance: &str,
    request: &serde_json::Value,
    files: &[SavedFile],
) -> bool {
    journal.shared_instance == shared_instance
        && journal.request == *request
        && journal.files == files
}

async fn recovery_rows(
    pool: &sqlx::SqlitePool,
    account: &str,
    origin: &str,
) -> crate::Result<Vec<(String, String, bool)>> {
    // Retry pending work first. Historical copies are retained, but checking
    // every version on every tick would grow without bound as users publish.
    Ok(sqlx::query_as("SELECT id,manifest,reconciled FROM self_hosted_uploads WHERE account=? AND origin=? AND (uploaded=0 OR reconciled=0 OR checked_at < unixepoch()-300) ORDER BY uploaded,reconciled,checked_at,rowid LIMIT 20")
        .bind(account).bind(origin).fetch_all(pool).await?)
}

async fn earlier_metadata(
    pool: &sqlx::SqlitePool,
    journal: &Journal,
) -> crate::Result<Vec<Journal>> {
    let manifests: Vec<String> = sqlx::query_scalar("SELECT manifest FROM self_hosted_uploads WHERE account=? AND origin=? AND uploaded=0 AND rowid < (SELECT rowid FROM self_hosted_uploads WHERE id=?) ORDER BY rowid")
        .bind(&journal.account).bind(&journal.origin).bind(&journal.id).fetch_all(pool).await?;
    let mut earlier = Vec::new();
    for manifest in manifests {
        let candidate: Journal = serde_json::from_str(&manifest)?;
        if candidate.shared_instance == journal.shared_instance
            && candidate.response.is_none()
        {
            earlier.push(candidate);
        }
    }
    Ok(earlier)
}

async fn create_metadata(
    journal: &mut Journal,
    client: &reqwest::Client,
    token: &str,
    pool: &sqlx::SqlitePool,
) -> crate::Result<bool> {
    let response = client
        .post(format!(
            "{}/v1/instances/{}/versions",
            journal.origin, journal.shared_instance
        ))
        .bearer_auth(token)
        .header("Idempotency-Key", &journal.id)
        .json(&journal.request)
        .send()
        .await?;
    if response.status().is_server_error() {
        return Ok(false);
    }
    let response: Version = response.error_for_status()?.json().await?;
    if response.external_files.len() != journal.files.len() {
        return Err(error(
            "Sharing service returned an incomplete upload list",
        ));
    }
    journal.response = Some(response);
    sqlx::query("UPDATE self_hosted_uploads SET manifest=? WHERE id=?")
        .bind(serde_json::to_string(journal)?)
        .bind(&journal.id)
        .execute(pool)
        .await?;
    Ok(true)
}

fn client() -> crate::Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(120))
        .build()?)
}

fn upload_client() -> crate::Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(std::time::Duration::from_secs(15))
        .read_timeout(std::time::Duration::from_secs(30))
        .build()?)
}

/// Persist the complete upload before creating the remote version. The request
/// ID allows a restart to recover even if the server reply was never received.
pub(crate) async fn publish(
    instance: &str,
    shared_instance: &str,
    request: serde_json::Value,
    files: Vec<SnapshotFile>,
    state: &State,
) -> crate::Result<i32> {
    initialize(&state.pool).await?;
    let session = crate::instance::shared_clients_session()
        .ok_or_else(|| error("Sharing service is not configured"))?;
    let account = session
        .user_id
        .ok_or_else(|| error("Sign in before sharing"))?;
    let id = uuid::Uuid::new_v4().to_string();
    let directory = state
        .directories
        .settings_dir
        .join("sharing-uploads")
        .join(&id);
    crate::util::io::create_dir_all(&directory).await?;
    let mut saved = Vec::with_capacity(files.len());
    let mut written = std::collections::HashSet::new();
    for (index, file) in files.into_iter().enumerate() {
        // Hash the same bytes we persist so edits to live files cannot invalidate the journal.
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let temporary = directory.join(format!("{index}.partial"));
        let mut source = tokio::fs::File::open(&file.path)
            .await
            .map_err(crate::util::io::IOError::from)?;
        let mut target = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .await
            .map_err(crate::util::io::IOError::from)?;
        let mut digest = Sha256::new();
        let mut size = 0;
        let mut buffer = [0; 64 * 1024];
        loop {
            let count = source
                .read(&mut buffer)
                .await
                .map_err(crate::util::io::IOError::from)?;
            if count == 0 {
                break;
            }
            target
                .write_all(&buffer[..count])
                .await
                .map_err(crate::util::io::IOError::from)?;
            digest.update(&buffer[..count]);
            size += count as u64;
        }
        target
            .sync_all()
            .await
            .map_err(crate::util::io::IOError::from)?;
        drop(target);
        let sha256 = format!("{:x}", digest.finalize());
        if written.insert(sha256.clone()) {
            tokio::fs::rename(&temporary, directory.join(&sha256))
                .await
                .map_err(crate::util::io::IOError::from)?;
        } else {
            tokio::fs::remove_file(&temporary)
                .await
                .map_err(crate::util::io::IOError::from)?;
        }
        saved.push(SavedFile {
            name: file.name,
            kind: file.kind,
            sha256,
            size,
        });
    }
    saved.sort();
    let pending: Vec<String> = sqlx::query_scalar("SELECT manifest FROM self_hosted_uploads WHERE account=? AND origin=? AND instance=? AND uploaded=0 ORDER BY rowid")
        .bind(&account).bind(&session.base_url).bind(instance).fetch_all(&state.pool).await?;
    let mut existing = None;
    for manifest in pending {
        let journal: Journal = serde_json::from_str(&manifest)?;
        if same_snapshot(&journal, shared_instance, &request, &saved) {
            existing = Some(journal.id);
            break;
        }
    }
    let id = if let Some(existing_id) = existing {
        tokio::fs::remove_dir_all(&directory)
            .await
            .map_err(crate::util::io::IOError::from)?;
        existing_id
    } else {
        let journal = Journal {
            id: id.clone(),
            account: account.clone(),
            origin: session.base_url.clone(),
            instance: instance.into(),
            shared_instance: shared_instance.into(),
            request,
            files: saved,
            response: None,
        };
        sqlx::query("INSERT INTO self_hosted_uploads (id,account,origin,instance,manifest) VALUES (?,?,?,?,?)")
            .bind(&id).bind(&account).bind(&session.base_url).bind(instance).bind(serde_json::to_string(&journal)?).execute(&state.pool).await?;
        id
    };
    loop {
        if let Some(version) = transfer(&id, state).await? {
            return Ok(version);
        }
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
}

async fn transfer(id: &str, state: &State) -> crate::Result<Option<i32>> {
    let _guard = TRANSFER.lock().await;
    let manifest: String = sqlx::query_scalar(
        "SELECT manifest FROM self_hosted_uploads WHERE id=?",
    )
    .bind(id)
    .fetch_one(&state.pool)
    .await?;
    let mut journal: Journal = serde_json::from_str(&manifest)?;
    let session = crate::instance::shared_clients_session()
        .ok_or_else(|| error("Sharing service is not configured"))?;
    if session.user_id.as_deref() != Some(&journal.account)
        || session.base_url != journal.origin
    {
        return Err(error(
            "Upload is saved for its owning account. Switch back to resume.",
        ));
    }
    let token = session
        .access_token
        .ok_or_else(|| error("Sign in to resume this upload"))?;
    sqlx::query(
        "UPDATE self_hosted_uploads SET checked_at=unixepoch() WHERE id=?",
    )
    .bind(id)
    .execute(&state.pool)
    .await?;
    let client = client()?;
    let upload_client = upload_client()?;
    if journal.response.is_none() {
        // Assign version numbers in snapshot order even if the backend was
        // offline when an earlier snapshot was saved. This does not wait for
        // storage, so newer metadata remains visible while uploads are queued.
        for mut earlier in earlier_metadata(&state.pool, &journal).await? {
            if !create_metadata(&mut earlier, &client, &token, &state.pool)
                .await?
            {
                return Ok(None);
            }
        }
        if !create_metadata(&mut journal, &client, &token, &state.pool).await? {
            return Ok(None);
        }
    }
    let version = journal
        .response
        .as_ref()
        .ok_or_else(|| error("Missing shared version"))?;
    let origin = reqwest::Url::parse(&journal.origin)?;
    for upload in &version.external_files {
        let file = journal
            .files
            .iter()
            .find(|file| {
                file.name == upload.file_name && file.kind == upload.file_type
            })
            .ok_or_else(|| {
                error("Sharing service requested an unknown file")
            })?;
        let url = reqwest::Url::parse(&upload.url)?;
        if url.origin() != origin.origin()
            || !url.path().starts_with("/v1/uploads/")
            || url.query().is_some()
        {
            return Err(error(
                "Sharing service returned an unsafe upload endpoint",
            ));
        }
        let prepared = client
            .post(format!("{}/prepare", upload.url))
            .bearer_auth(&token)
            .json(&serde_json::json!({"sha256":file.sha256,"size":file.size}))
            .send()
            .await?;
        if prepared.status().is_server_error() {
            return Ok(None);
        }
        let prepared: UploadState = prepared.error_for_status()?.json().await?;
        if prepared.status == "pending" {
            return Ok(None);
        }
        if prepared.sha256.as_deref() != Some(&file.sha256)
            || prepared.size != Some(file.size)
        {
            return Err(error("Upload receipt does not match the saved file"));
        }
        if prepared.status == "available" {
            continue;
        }
        if prepared.status != "upload" {
            return Err(error("Unknown upload state"));
        }
        let target = reqwest::Url::parse(
            prepared
                .upload_url
                .as_deref()
                .ok_or_else(|| error("Missing storage upload URL"))?,
        )?;
        if !matches!(target.scheme(), "http" | "https")
            || !target.username().is_empty()
            || target.password().is_some()
        {
            return Err(error("Unsafe storage URL"));
        }
        let path = state
            .directories
            .settings_dir
            .join("sharing-uploads")
            .join(id)
            .join(&file.sha256);
        if tokio::fs::metadata(&path)
            .await
            .map_err(crate::util::io::IOError::from)?
            .len()
            != file.size
            || crate::self_hosted::integrity::shared_file_changed(
                &path,
                Some(&file.sha256),
            )
            .await?
        {
            return Err(error("Saved upload failed its integrity check"));
        }
        let source = tokio::fs::File::open(&path)
            .await
            .map_err(crate::util::io::IOError::from)?;
        // No account Authorization header is attached to this capability URL.
        let result = upload_client
            .put(target)
            .header(reqwest::header::CONTENT_LENGTH, file.size)
            .body(reqwest::Body::wrap_stream(
                tokio_util::io::ReaderStream::new(source),
            ))
            .send()
            .await
            .map_err(reqwest::Error::without_url)?;
        if !result.status().is_success() {
            return Ok(None);
        }
        let receipt: UploadState = client
            .get(format!("{}/status", upload.url))
            .bearer_auth(&token)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        if receipt.status != "available" {
            return Ok(None);
        }
        if receipt.sha256.as_deref() != Some(&file.sha256)
            || receipt.size != Some(file.size)
        {
            return Err(error(
                "Verified storage receipt does not match the upload",
            ));
        }
    }
    sqlx::query("UPDATE self_hosted_uploads SET uploaded=1 WHERE id=?")
        .bind(id)
        .execute(&state.pool)
        .await?;
    Ok(Some(version.version))
}

pub(crate) fn start_worker() {
    if !super::accounts::enabled() || STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    tokio::spawn(async {
        loop {
            if let Ok(state) = State::get().await {
                if let Err(err) = resume(&state).await {
                    tracing::warn!(error=%err, "Could not resume saved sharing uploads");
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(15)).await;
        }
    });
}

async fn resume(state: &State) -> crate::Result<()> {
    initialize(&state.pool).await?;
    let Some(session) = crate::instance::shared_clients_session() else {
        return Ok(());
    };
    let Some(account) = session.user_id.as_ref() else {
        return Ok(());
    };
    let rows = recovery_rows(&state.pool, account, &session.base_url).await?;
    for (id, manifest, reconciled) in rows {
        match transfer(&id, state).await {
            Ok(Some(version)) => {
                if reconciled {
                    continue;
                }
                let journal: Journal = serde_json::from_str(&manifest)?;
                let instance = &journal.instance;
                let _guard = state.lock_shared_instance(instance).await;
                let Some(current) = crate::instance::shared_clients_session()
                else {
                    continue;
                };
                if current.user_id != session.user_id
                    || current.base_url != session.base_url
                {
                    continue;
                }
                let attachment =
                    crate::state::get_instance(instance, &state.pool)
                        .await?
                        .and_then(|metadata| metadata.shared_instance);
                if let Some(attachment) = attachment.filter(|attachment| {
                    attachment.id == journal.shared_instance
                        && attachment.linked_user_id.as_deref()
                            == Some(account.as_str())
                        && !attachment
                            .applied_version
                            .is_some_and(|applied| applied >= version)
                }) {
                    // The immutable upload may have completed after the live
                    // instance was edited. Existing publish preview computes
                    // whether that content is up to date when it is opened.
                    crate::state::set_shared_instance_sync_status(
                        instance,
                        ContentSetSyncStatus::Stale,
                        attachment.applied_version,
                        Some(
                            attachment
                                .latest_version
                                .unwrap_or(version)
                                .max(version),
                        ),
                        &state.pool,
                    )
                    .await?;
                    crate::event::emit::emit_instance(
                        instance,
                        crate::event::InstancePayloadType::Edited,
                    )
                    .await?;
                }
                // Persist this only after the local write. Repeating that
                // write after a crash is safe; skipping it is not.
                sqlx::query(
                    "UPDATE self_hosted_uploads SET reconciled=1 WHERE id=?",
                )
                .bind(&id)
                .execute(&state.pool)
                .await?;
            }
            Ok(None) => {}
            Err(err) => {
                tracing::warn!(error=%err, "Saved sharing upload will be retried")
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_snapshot_reuse_requires_identical_destination_and_content() {
        let mut journal = Journal {
            id: "upload".into(),
            account: "owner".into(),
            origin: "http://localhost:8787".into(),
            instance: "local".into(),
            shared_instance: "remote".into(),
            request: serde_json::json!({"game_version": "1.21"}),
            files: vec![SavedFile {
                name: "example.jar".into(),
                kind: "mod".into(),
                sha256: "old-bytes".into(),
                size: 4,
            }],
            response: None,
        };
        let saved: Vec<SavedFile> = serde_json::from_value(
            serde_json::to_value(&journal.files).unwrap(),
        )
        .unwrap();
        let request = journal.request.clone();
        assert!(same_snapshot(&journal, "remote", &request, &saved));
        assert!(!same_snapshot(&journal, "new-remote", &request, &saved));
        assert!(!same_snapshot(
            &journal,
            "remote",
            &serde_json::json!({"game_version": "1.22"}),
            &saved
        ));
        // Identical filenames and sizes are insufficient after an edit.
        journal.files[0].sha256 = "new-bytes".into();
        assert!(!same_snapshot(&journal, "remote", &request, &saved));
    }

    #[tokio::test]
    async fn metadata_creation_follows_insertion_order_within_remote_instance()
    {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        initialize(&pool).await.unwrap();
        let mut current = None;
        for (id, account, origin, remote) in [
            ("z-first", "owner", "origin", "remote"),
            ("a-second", "owner", "origin", "remote"),
            ("other-account", "other", "origin", "remote"),
            ("other-origin", "owner", "elsewhere", "remote"),
            ("other-instance", "owner", "origin", "elsewhere"),
            ("current", "owner", "origin", "remote"),
            ("later", "owner", "origin", "remote"),
        ] {
            let journal = Journal {
                id: id.into(),
                account: account.into(),
                origin: origin.into(),
                instance: "local".into(),
                shared_instance: remote.into(),
                request: serde_json::json!({}),
                files: vec![],
                response: None,
            };
            sqlx::query("INSERT INTO self_hosted_uploads(id,account,origin,instance,manifest,created) VALUES (?, ?, ?, 'local', ?, 1)")
                .bind(id).bind(account).bind(origin).bind(serde_json::to_string(&journal).unwrap()).execute(&pool).await.unwrap();
            if id == "current" {
                current = Some(journal);
            }
        }
        let earlier = earlier_metadata(&pool, &current.unwrap()).await.unwrap();
        assert_eq!(
            earlier
                .iter()
                .map(|row| row.id.as_str())
                .collect::<Vec<_>>(),
            ["z-first", "a-second"]
        );
    }

    #[tokio::test]
    async fn recovery_prioritizes_unfinished_work_and_retains_crash_gap() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        initialize(&pool).await.unwrap();
        for index in 0..25 {
            sqlx::query("INSERT INTO self_hosted_uploads(id,account,origin,instance,manifest,uploaded,reconciled) VALUES (?, 'owner', 'origin', 'local', '{}', 1, 1)")
                .bind(format!("history-{index:02}")).execute(&pool).await.unwrap();
        }
        for (id, account, uploaded, reconciled) in [
            ("pending", "owner", false, false),
            ("crash-after-upload", "owner", true, false),
            ("recently-checked", "owner", true, true),
            ("other-account", "someone-else", false, false),
        ] {
            sqlx::query("INSERT INTO self_hosted_uploads(id,account,origin,instance,manifest,uploaded,reconciled,checked_at) VALUES (?, ?, 'origin', 'local', '{}', ?, ?, unixepoch())")
                .bind(id).bind(account).bind(uploaded).bind(reconciled).execute(&pool).await.unwrap();
        }
        // Reinitializing models a restart; uploaded does not imply reconciled.
        initialize(&pool).await.unwrap();
        let rows = recovery_rows(&pool, "owner", "origin").await.unwrap();
        assert_eq!(rows.len(), 20);
        assert_eq!(rows[0].0, "pending");
        assert_eq!(rows[1].0, "crash-after-upload");
        assert!(!rows[1].2);
        assert!(
            !rows
                .iter()
                .any(|row| row.0 == "recently-checked"
                    || row.0 == "other-account")
        );
        assert!(
            recovery_rows(&pool, "owner", "other-origin")
                .await
                .unwrap()
                .is_empty()
        );
    }
}
