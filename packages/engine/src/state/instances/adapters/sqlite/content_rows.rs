#![allow(dead_code)]

use crate::ErrorKind;
use crate::state::instances::adapters::filesystem::server_addon_type;
use crate::state::instances::{
    ContentEntry, ContentRequirement, ContentSet, ContentSetRemoteRef,
    ContentSetRemoteRefType, ContentSetStatus, ContentSetSyncProvider,
    ContentSetSyncState, ContentSetSyncStatus, ContentSourceKind,
    ContentUpdateCheck, InstanceFile,
};
use crate::state::{ModLoader, ProjectType, ReleaseChannel};
use anyhow::bail;
use chrono::{DateTime, TimeZone, Utc};
use sqlx::{Executor, Sqlite, SqliteConnection, SqlitePool, Transaction};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct ContentSetRow {
    pub id: String,
    pub instance_id: String,
    pub name: String,
    pub source_kind: String,
    pub status: String,
    pub game_version: String,
    pub protocol_version: Option<i64>,
    pub loader: String,
    pub loader_version: Option<String>,
    pub created: i64,
    pub modified: i64,
}

impl TryFrom<ContentSetRow> for ContentSet {
    type Error = crate::Error;

    fn try_from(row: ContentSetRow) -> crate::Result<Self> {
        Ok(Self {
            id: row.id,
            instance_id: row.instance_id,
            name: row.name,
            source_kind: ContentSourceKind::from_str(&row.source_kind)?,
            status: ContentSetStatus::from_str(&row.status)?,
            game_version: row.game_version,
            protocol_version: row.protocol_version.map(|value| value as u32),
            loader: ModLoader::from_string(&row.loader),
            loader_version: row.loader_version,
            created: timestamp(row.created),
            modified: timestamp(row.modified),
        })
    }
}

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct ContentSetRemoteRefRow {
    pub content_set_id: String,
    pub ref_type: String,
    pub ref_id: String,
}

impl TryFrom<ContentSetRemoteRefRow> for ContentSetRemoteRef {
    type Error = crate::Error;

    fn try_from(row: ContentSetRemoteRefRow) -> crate::Result<Self> {
        Ok(Self {
            content_set_id: row.content_set_id,
            ref_type: ContentSetRemoteRefType::from_str(&row.ref_type)?,
            ref_id: row.ref_id,
        })
    }
}

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct ContentSetSyncStateRow {
    pub content_set_id: String,
    pub provider: String,
    pub applied_update_id: Option<String>,
    pub latest_available_update_id: Option<String>,
    pub checked_at: Option<i64>,
    pub status: String,
    pub applied_backtrack_version: Option<String>,
}

impl TryFrom<ContentSetSyncStateRow> for ContentSetSyncState {
    type Error = crate::Error;

    fn try_from(row: ContentSetSyncStateRow) -> crate::Result<Self> {
        Ok(Self {
            content_set_id: row.content_set_id,
            provider: ContentSetSyncProvider::from_str(&row.provider)?,
            applied_update_id: row.applied_update_id,
            applied_backtrack_version: row.applied_backtrack_version,
            latest_available_update_id: row.latest_available_update_id,
            checked_at: row.checked_at.and_then(optional_timestamp),
            status: ContentSetSyncStatus::from_str(&row.status)?,
        })
    }
}

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct InstanceFileRow {
    pub id: String,
    pub instance_id: String,
    pub relative_path: String,
    pub file_name: String,
    pub enabled: i64,
    pub sha1: String,
    pub size: i64,
    pub missing: i64,
    pub added_at: i64,
    pub modified_at: i64,
}

impl TryFrom<InstanceFileRow> for InstanceFile {
    type Error = crate::Error;

    fn try_from(row: InstanceFileRow) -> crate::Result<Self> {
        Ok(Self {
            id: row.id,
            instance_id: row.instance_id,
            relative_path: row.relative_path,
            file_name: row.file_name,
            enabled: row.enabled == 1,
            sha1: row.sha1,
            size: unsigned(row.size, "size")?,
            missing: row.missing == 1,
            added_at: timestamp(row.added_at),
            modified_at: timestamp(row.modified_at),
        })
    }
}

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct ContentEntryRow {
    pub id: String,
    pub instance_id: String,
    pub content_set_id: String,
    pub file_id: Option<String>,
    pub project_type: String,
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub source_kind: String,
    pub server_requirement: String,
    pub client_requirement: String,
    pub enabled: i64,
    pub sides_unlocked: i64,
    pub server_override: i64,
    pub added_at: i64,
    pub modified_at: i64,
}

impl TryFrom<ContentEntryRow> for ContentEntry {
    type Error = crate::Error;

    fn try_from(row: ContentEntryRow) -> crate::Result<Self> {
        Ok(Self {
            id: row.id,
            instance_id: row.instance_id,
            content_set_id: row.content_set_id,
            file_id: row.file_id,
            project_type: project_type_from_str(&row.project_type)?,
            project_id: row.project_id,
            version_id: row.version_id,
            source_kind: ContentSourceKind::from_str(&row.source_kind)?,
            server_requirement: ContentRequirement::from_str(
                &row.server_requirement,
            )?,
            client_requirement: ContentRequirement::from_str(
                &row.client_requirement,
            )?,
            enabled: row.enabled == 1,
            sides_unlocked: row.sides_unlocked == 1,
            server_override: row.server_override == 1,
            added_at: timestamp(row.added_at),
            modified_at: timestamp(row.modified_at),
        })
    }
}

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct ContentUpdateCheckRow {
    pub content_entry_id: String,
    pub update_channel: String,
    pub update_version_id: Option<String>,
    pub checked_at: i64,
}

impl From<ContentUpdateCheckRow> for ContentUpdateCheck {
    fn from(row: ContentUpdateCheckRow) -> Self {
        Self {
            content_entry_id: row.content_entry_id,
            update_channel: ReleaseChannel::from_key(&row.update_channel),
            update_version_id: row.update_version_id,
            checked_at: timestamp(row.checked_at),
        }
    }
}

pub(crate) async fn get_applied_content_set<'e, E>(
    instance_id: &str,
    exec: E,
) -> crate::Result<Option<ContentSet>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let row = sqlx::query_as!(
        ContentSetRow,
        "
		SELECT cs.*
		FROM instances i
		INNER JOIN instance_content_sets cs
			ON cs.id = i.applied_content_set_id
		WHERE i.id = ?
		",
        instance_id,
    )
    .fetch_optional(exec)
    .await?;

    row.map(TryInto::try_into).transpose()
}

pub(crate) async fn get_content_set<'e, E>(
    content_set_id: &str,
    exec: E,
) -> crate::Result<Option<ContentSet>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let row = sqlx::query_as!(
        ContentSetRow,
        "
		SELECT *
		FROM instance_content_sets
		WHERE id = ?
		",
        content_set_id,
    )
    .fetch_optional(exec)
    .await?;

    row.map(TryInto::try_into).transpose()
}

pub(crate) async fn get_content_sets_for_instance<'e, E>(
    instance_id: &str,
    exec: E,
) -> crate::Result<Vec<ContentSet>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows = sqlx::query_as!(
        ContentSetRow,
        "
		SELECT *
		FROM instance_content_sets
		WHERE instance_id = ?
		ORDER BY created ASC, id ASC
		",
        instance_id,
    )
    .fetch_all(exec)
    .await?;

    rows.into_iter().map(TryInto::try_into).collect()
}

pub(crate) async fn insert_content_set(
    content_set: &ContentSet,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    let id = content_set.id.as_str();
    let instance_id = content_set.instance_id.as_str();
    let name = content_set.name.as_str();
    let source_kind = content_set.source_kind.as_str();
    let status = content_set.status.as_str();
    let game_version = content_set.game_version.as_str();
    let protocol_version =
        content_set.protocol_version.map(|value| value as i64);
    let loader = content_set.loader.as_str();
    let loader_version = content_set.loader_version.as_deref();
    let created = content_set.created.timestamp();
    let modified = content_set.modified.timestamp();

    sqlx::query!(
        "
		INSERT INTO instance_content_sets (
			id,
			instance_id,
			name,
			source_kind,
			status,
			game_version,
			protocol_version,
			loader,
			loader_version,
			created,
			modified
		)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
		",
        id,
        instance_id,
        name,
        source_kind,
        status,
        game_version,
        protocol_version,
        loader,
        loader_version,
        created,
        modified,
    )
    .execute(&mut **tx)
    .await?;

    Ok(())
}

pub(crate) async fn update_content_set(
    content_set: &ContentSet,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    let id = content_set.id.as_str();
    let name = content_set.name.as_str();
    let source_kind = content_set.source_kind.as_str();
    let status = content_set.status.as_str();
    let game_version = content_set.game_version.as_str();
    let protocol_version =
        content_set.protocol_version.map(|value| value as i64);
    let loader = content_set.loader.as_str();
    let loader_version = content_set.loader_version.as_deref();
    let modified = content_set.modified.timestamp();

    sqlx::query!(
        "
		UPDATE instance_content_sets
		SET
			name = ?,
			source_kind = ?,
			status = ?,
			game_version = ?,
			protocol_version = ?,
			loader = ?,
			loader_version = ?,
			modified = ?
		WHERE id = ?
		",
        name,
        source_kind,
        status,
        game_version,
        protocol_version,
        loader,
        loader_version,
        modified,
        id,
    )
    .execute(&mut **tx)
    .await?;

    Ok(())
}

pub(crate) async fn get_content_set_remote_refs<'e, E>(
    content_set_id: &str,
    exec: E,
) -> crate::Result<Vec<ContentSetRemoteRef>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows = sqlx::query_as!(
        ContentSetRemoteRefRow,
        "
		SELECT *
		FROM instance_content_set_remote_refs
		WHERE content_set_id = ?
		ORDER BY ref_type ASC
		",
        content_set_id,
    )
    .fetch_all(exec)
    .await?;

    rows.into_iter().map(TryInto::try_into).collect()
}

pub(crate) async fn upsert_content_set_remote_ref(
    remote_ref: &ContentSetRemoteRef,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    let content_set_id = remote_ref.content_set_id.as_str();
    let ref_type = remote_ref.ref_type.as_str();
    let ref_id = remote_ref.ref_id.as_str();

    sqlx::query!(
        "
		INSERT INTO instance_content_set_remote_refs (
			content_set_id,
			ref_type,
			ref_id
		)
		VALUES (?, ?, ?)
		ON CONFLICT (content_set_id, ref_type) DO UPDATE SET
			ref_id = excluded.ref_id
		",
        content_set_id,
        ref_type,
        ref_id,
    )
    .execute(&mut **tx)
    .await?;

    Ok(())
}

pub(crate) async fn delete_content_set_remote_ref(
    content_set_id: &str,
    ref_type: ContentSetRemoteRefType,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    let ref_type = ref_type.as_str();

    sqlx::query!(
        "
		DELETE FROM instance_content_set_remote_refs
		WHERE content_set_id = ? AND ref_type = ?
		",
        content_set_id,
        ref_type,
    )
    .execute(&mut **tx)
    .await?;

    Ok(())
}

pub(crate) async fn get_content_set_sync_state<'e, E>(
    content_set_id: &str,
    exec: E,
) -> crate::Result<Option<ContentSetSyncState>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let row = sqlx::query_as!(
        ContentSetSyncStateRow,
        "
		SELECT *
		FROM instance_content_set_sync_state
		WHERE content_set_id = ?
		",
        content_set_id,
    )
    .fetch_optional(exec)
    .await?;

    row.map(TryInto::try_into).transpose()
}

pub(crate) async fn upsert_content_set_sync_state(
    sync_state: &ContentSetSyncState,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    let content_set_id = sync_state.content_set_id.as_str();
    let provider = sync_state.provider.as_str();
    let applied_update_id = sync_state.applied_update_id.as_deref();
    let applied_backtrack_version =
        sync_state.applied_backtrack_version.as_deref();
    let latest_available_update_id =
        sync_state.latest_available_update_id.as_deref();
    let checked_at = sync_state.checked_at.map(|value| value.timestamp());
    let status = sync_state.status.as_str();

    sqlx::query!(
        "
		INSERT INTO instance_content_set_sync_state (
			content_set_id,
			provider,
			applied_update_id,
			latest_available_update_id,
			checked_at,
			status,
			applied_backtrack_version
		)
		VALUES (?, ?, ?, ?, ?, ?, ?)
		ON CONFLICT (content_set_id) DO UPDATE SET
			provider = excluded.provider,
			applied_update_id = excluded.applied_update_id,
			latest_available_update_id = excluded.latest_available_update_id,
			checked_at = excluded.checked_at,
			status = excluded.status,
			applied_backtrack_version = excluded.applied_backtrack_version
		",
        content_set_id,
        provider,
        applied_update_id,
        latest_available_update_id,
        checked_at,
        status,
        applied_backtrack_version,
    )
    .execute(&mut **tx)
    .await?;

    Ok(())
}

pub(crate) async fn delete_content_set_sync_state(
    content_set_id: &str,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    sqlx::query!(
        "
		DELETE FROM instance_content_set_sync_state
		WHERE content_set_id = ?
		",
        content_set_id,
    )
    .execute(&mut **tx)
    .await?;

    Ok(())
}

pub async fn get_instance_files<'e, E>(
    instance_id: &str,
    exec: E,
) -> crate::Result<Vec<InstanceFile>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows = sqlx::query_as!(
        InstanceFileRow,
        "
		SELECT *
		FROM instance_files
		WHERE instance_id = ?
		ORDER BY relative_path ASC
		",
        instance_id,
    )
    .fetch_all(exec)
    .await?;

    rows.into_iter().map(TryInto::try_into).collect()
}

pub async fn get_locked_instance_file_ids(
    instance_id: &str,
    pool: &SqlitePool,
) -> crate::Result<HashSet<String>> {
    let file_ids = sqlx::query_scalar::<_, String>(
        "
		SELECT content_lock.file_id
		FROM instance_content_locks content_lock
		INNER JOIN instance_files file ON file.id = content_lock.file_id
		WHERE file.instance_id = ?
		",
    )
    .bind(instance_id)
    .fetch_all(pool)
    .await?;

    Ok(file_ids.into_iter().collect())
}

pub(crate) async fn is_instance_file_locked(
    instance_id: &str,
    relative_path: &str,
    pool: &SqlitePool,
) -> crate::Result<bool> {
    let locked = sqlx::query_scalar::<_, i64>(
        "
		SELECT EXISTS (
			SELECT 1
			FROM instance_content_locks content_lock
			INNER JOIN instance_files file ON file.id = content_lock.file_id
			WHERE file.instance_id = ? AND file.relative_path = ?
		)
		",
    )
    .bind(instance_id)
    .bind(relative_path)
    .fetch_one(pool)
    .await?;

    Ok(locked != 0)
}

pub(crate) async fn set_instance_file_locked(
    instance_id: &str,
    relative_path: &str,
    locked: bool,
    pool: &SqlitePool,
) -> crate::Result<()> {
    let file =
        get_instance_file_by_relative_path(instance_id, relative_path, pool)
            .await?
            .ok_or(ErrorKind::InputError(format!(
                "unknown content file {relative_path}"
            )))?;

    if locked {
        sqlx::query(
            "
			INSERT INTO instance_content_locks (file_id)
			VALUES (?)
			ON CONFLICT (file_id) DO NOTHING
			",
        )
        .bind(&file.id)
        .execute(pool)
        .await?;
    } else {
        sqlx::query(
            "
			DELETE FROM instance_content_locks
			WHERE file_id = ?
			",
        )
        .bind(&file.id)
        .execute(pool)
        .await?;
    }

    Ok(())
}

pub(crate) async fn set_instance_file_missing(
    file_id: &str,
    missing: bool,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<Option<InstanceFile>> {
    let missing = i64::from(missing);
    let modified_at = Utc::now().timestamp();
    let row = sqlx::query_as!(
        InstanceFileRow,
        "
		UPDATE instance_files
		SET
			missing = ?,
			modified_at = ?
		WHERE id = ?
		RETURNING
			id,
			instance_id,
			relative_path,
			file_name,
			enabled,
			sha1,
			size,
			missing,
			added_at,
			modified_at
		",
        missing,
        modified_at,
        file_id,
    )
    .fetch_optional(&mut **tx)
    .await?;

    row.map(TryInto::try_into).transpose()
}

pub(crate) async fn upsert_instance_file(
    file: &InstanceFile,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<InstanceFile> {
    let id = file.id.as_str();
    let instance_id = file.instance_id.as_str();
    let relative_path = file.relative_path.as_str();
    let file_name = file.file_name.as_str();
    let enabled = i64::from(file.enabled);
    let sha1 = file.sha1.as_str();
    let size = file.size as i64;
    let missing = i64::from(file.missing);
    let added_at = file.added_at.timestamp();
    let modified_at = file.modified_at.timestamp();

    let row = sqlx::query_as!(
        InstanceFileRow,
        "
		INSERT INTO instance_files (
			id,
			instance_id,
			relative_path,
			file_name,
			enabled,
			sha1,
			size,
			missing,
			added_at,
			modified_at
		)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
		ON CONFLICT (instance_id, relative_path) DO UPDATE SET
			file_name = excluded.file_name,
			enabled = excluded.enabled,
			sha1 = excluded.sha1,
			size = excluded.size,
			missing = excluded.missing,
			modified_at = excluded.modified_at
		RETURNING
			id,
			instance_id,
			relative_path,
			file_name,
			enabled,
			sha1,
			size,
			missing,
			added_at,
			modified_at
		",
        id,
        instance_id,
        relative_path,
        file_name,
        enabled,
        sha1,
        size,
        missing,
        added_at,
        modified_at,
    )
    .fetch_one(&mut **tx)
    .await?;

    row.try_into()
}

async fn insert_content_entry(
    entry: &ContentEntry,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    let id = entry.id.as_str();
    let instance_id = entry.instance_id.as_str();
    let content_set_id = entry.content_set_id.as_str();
    let file_id = entry.file_id.as_deref();
    let project_type = entry.project_type.get_name();
    let project_id = entry.project_id.as_deref();
    let version_id = entry.version_id.as_deref();
    let source_kind = entry.source_kind.as_str();
    let server_requirement = entry.server_requirement.as_str();
    let client_requirement = entry.client_requirement.as_str();
    let enabled = i64::from(entry.enabled);
    let sides_unlocked = i64::from(entry.sides_unlocked);
    let server_override = i64::from(entry.server_override);
    let added_at = entry.added_at.timestamp();
    let modified_at = entry.modified_at.timestamp();

    sqlx::query(
        "
		INSERT INTO instance_content_entries (
			id,
			instance_id,
			content_set_id,
			file_id,
			project_type,
			project_id,
			version_id,
			source_kind,
			server_requirement,
			client_requirement,
			enabled,
			sides_unlocked,
			server_override,
			added_at,
			modified_at
		)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
		",
    )
    .bind(id)
    .bind(instance_id)
    .bind(content_set_id)
    .bind(file_id)
    .bind(project_type)
    .bind(project_id)
    .bind(version_id)
    .bind(source_kind)
    .bind(server_requirement)
    .bind(client_requirement)
    .bind(enabled)
    .bind(sides_unlocked)
    .bind(server_override)
    .bind(added_at)
    .bind(modified_at)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

pub(crate) async fn restore_instance_content_snapshot(
    instance_id: &str,
    files: &[InstanceFile],
    entries: &[ContentEntry],
    pool: &SqlitePool,
) -> crate::Result<()> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    sqlx::query(
        "
		DELETE FROM instance_content_entries
		WHERE instance_id = ?
		",
    )
    .bind(instance_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "
		DELETE FROM instance_files
		WHERE instance_id = ?
		",
    )
    .bind(instance_id)
    .execute(&mut *tx)
    .await?;

    for file in files {
        upsert_instance_file(file, &mut tx).await?;
    }
    for entry in entries {
        insert_content_entry(entry, &mut tx).await?;
    }

    tx.commit().await?;
    Ok(())
}

pub async fn get_content_entries<'e, E>(
    content_set_id: &str,
    exec: E,
) -> crate::Result<Vec<ContentEntry>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows = sqlx::query_as!(
        ContentEntryRow,
        "
		SELECT
			id, instance_id, content_set_id, file_id, project_type AS \"project_type!\", project_id,
			version_id, source_kind, server_requirement, client_requirement, enabled,
			sides_unlocked, server_override, added_at, modified_at
		FROM instance_content_entries
		WHERE content_set_id = ? AND project_type IS NOT NULL
		ORDER BY added_at ASC, id ASC
		",
        content_set_id,
    )
    .fetch_all(exec)
    .await?;

    rows.into_iter().map(TryInto::try_into).collect()
}

pub(crate) async fn get_content_update_check<'e, E>(
    content_entry_id: &str,
    exec: E,
) -> crate::Result<Option<ContentUpdateCheck>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let row = sqlx::query_as!(
        ContentUpdateCheckRow,
        "
		SELECT *
		FROM instance_content_update_checks
		WHERE content_entry_id = ?
		",
        content_entry_id,
    )
    .fetch_optional(exec)
    .await?;

    Ok(row.map(Into::into))
}

pub(crate) struct UpsertInstanceFile<'a> {
    pub instance_id: &'a str,
    pub relative_path: &'a str,
    pub file_name: &'a str,
    pub enabled: bool,
    pub sha1: &'a str,
    pub size: u64,
    pub missing: bool,
}

pub async fn get_instance_file_by_relative_path(
    instance_id: &str,
    relative_path: &str,
    pool: &SqlitePool,
) -> crate::Result<Option<InstanceFile>> {
    let row = sqlx::query_as!(
        InstanceFileRow,
        "
		SELECT *
		FROM instance_files
		WHERE instance_id = ? AND relative_path = ?
		",
        instance_id,
        relative_path,
    )
    .fetch_optional(pool)
    .await?;

    row.map(TryInto::try_into).transpose()
}

pub(crate) async fn upsert_instance_file_from_parts(
    input: UpsertInstanceFile<'_>,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<InstanceFile> {
    let now = Utc::now();
    let file = InstanceFile {
        id: format!("instance-file:{}", Uuid::new_v4()),
        instance_id: input.instance_id.to_string(),
        relative_path: input.relative_path.to_string(),
        file_name: input.file_name.to_string(),
        enabled: input.enabled,
        sha1: input.sha1.to_string(),
        size: input.size,
        missing: input.missing,
        added_at: now,
        modified_at: now,
    };

    upsert_instance_file(&file, tx).await
}

pub(crate) async fn rename_instance_file(
    instance_id: &str,
    old_relative_path: &str,
    new_relative_path: &str,
    new_file_name: &str,
    enabled: bool,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<Option<InstanceFile>> {
    let enabled = i64::from(enabled);
    let modified_at = Utc::now().timestamp();

    let source_id = sqlx::query_scalar!(
        "
		SELECT id
		FROM instance_files
		WHERE instance_id = ? AND relative_path = ?
		",
        instance_id,
        old_relative_path,
    )
    .fetch_optional(&mut **tx)
    .await?;
    let target_id = sqlx::query_scalar!(
        "
		SELECT id
		FROM instance_files
		WHERE instance_id = ? AND relative_path = ?
		",
        instance_id,
        new_relative_path,
    )
    .fetch_optional(&mut **tx)
    .await?;

    if let (Some(source_id), Some(target_id)) =
        (source_id.as_deref(), target_id.as_deref())
        && source_id != target_id
    {
        sqlx::query!(
			"INSERT INTO instance_content_locks (file_id) SELECT ? WHERE EXISTS (SELECT 1 FROM instance_content_locks WHERE file_id = ?) ON CONFLICT (file_id) DO NOTHING",
			source_id,
			target_id,
		)
		.execute(&mut **tx)
		.await?;

        sqlx::query!(
            "
				DELETE FROM instance_content_entries
				WHERE id IN (
					SELECT target_entry.id
					FROM instance_content_entries target_entry
					WHERE target_entry.instance_id = ?
						AND target_entry.file_id = ?
						AND EXISTS (
							SELECT 1
							FROM instance_content_entries source_entry
							WHERE source_entry.instance_id = target_entry.instance_id
								AND source_entry.content_set_id = target_entry.content_set_id
								AND source_entry.file_id = ?
						)
				)
				",
            instance_id,
            target_id,
            source_id,
        )
        .execute(&mut **tx)
        .await?;

        sqlx::query!(
            "
				UPDATE instance_content_entries
				SET file_id = ?, modified_at = ?
				WHERE instance_id = ? AND file_id = ?
				",
            source_id,
            modified_at,
            instance_id,
            target_id,
        )
        .execute(&mut **tx)
        .await?;

        sqlx::query!(
            "
				DELETE FROM instance_files
				WHERE id = ?
				",
            target_id,
        )
        .execute(&mut **tx)
        .await?;
    }

    sqlx::query!(
        "
		UPDATE instance_files
		SET
			relative_path = ?,
			file_name = ?,
			enabled = ?,
			missing = 0,
			modified_at = ?
		WHERE instance_id = ? AND relative_path = ?
		",
        new_relative_path,
        new_file_name,
        enabled,
        modified_at,
        instance_id,
        old_relative_path,
    )
    .execute(&mut **tx)
    .await?;

    let row = sqlx::query_as!(
        InstanceFileRow,
        "
		SELECT *
		FROM instance_files
		WHERE instance_id = ? AND relative_path = ?
		",
        instance_id,
        new_relative_path,
    )
    .fetch_optional(&mut **tx)
    .await?;

    row.map(TryInto::try_into).transpose()
}

pub(crate) async fn remove_instance_file_by_relative_path(
    instance_id: &str,
    relative_path: &str,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    sqlx::query!(
        "
		DELETE FROM instance_files
		WHERE instance_id = ? AND relative_path = ?
		",
        instance_id,
        relative_path,
    )
    .execute(&mut **tx)
    .await?;

    Ok(())
}

pub(crate) struct UpsertContentEntry<'a> {
    pub instance_id: &'a str,
    pub content_set_id: &'a str,
    pub file_id: Option<&'a str>,
    pub project_type: ProjectType,
    pub project_id: Option<&'a str>,
    pub version_id: Option<&'a str>,
    pub source_kind: ContentSourceKind,
    pub server_requirement: ContentRequirement,
    pub client_requirement: ContentRequirement,
    pub enabled: bool,
}

pub(crate) async fn get_content_entry_by_id(
    id: &str,
    pool: &SqlitePool,
) -> crate::Result<Option<ContentEntry>> {
    let row = sqlx::query_as!(
        ContentEntryRow,
        "
		SELECT
			id, instance_id, content_set_id, file_id, project_type AS \"project_type!\", project_id,
			version_id, source_kind, server_requirement, client_requirement, enabled,
			sides_unlocked, server_override, added_at, modified_at
		FROM instance_content_entries
		WHERE id = ? AND project_type IS NOT NULL
		",
        id,
    )
    .fetch_optional(pool)
    .await?;

    row.map(TryInto::try_into).transpose()
}

pub(crate) async fn get_content_entry_by_file(
    content_set_id: &str,
    file_id: &str,
    pool: &SqlitePool,
) -> crate::Result<Option<ContentEntry>> {
    let row = sqlx::query_as!(
        ContentEntryRow,
        "
		SELECT
			id, instance_id, content_set_id, file_id, project_type AS \"project_type!\", project_id,
			version_id, source_kind, server_requirement, client_requirement, enabled,
			sides_unlocked, server_override, added_at, modified_at
		FROM instance_content_entries
		WHERE content_set_id = ? AND file_id = ? AND project_type IS NOT NULL
		ORDER BY modified_at DESC
		LIMIT 1
		",
        content_set_id,
        file_id,
    )
    .fetch_optional(pool)
    .await?;

    row.map(TryInto::try_into).transpose()
}

pub(crate) async fn upsert_content_entry_from_parts(
    input: UpsertContentEntry<'_>,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<ContentEntry> {
    let id = format!("content-entry:{}", Uuid::new_v4());
    let project_type = input.project_type.get_name();
    let source_kind = input.source_kind.as_str();
    let server_requirement = input.server_requirement.as_str();
    let client_requirement = input.client_requirement.as_str();
    let enabled = i64::from(input.enabled);
    let now = Utc::now().timestamp();
    let row = if let Some(file_id) = input.file_id {
        sqlx::query_as!(
            ContentEntryRow,
            "
			INSERT INTO instance_content_entries (
				id,
				instance_id,
				content_set_id,
				file_id,
				project_type,
				project_id,
				version_id,
				source_kind,
				server_requirement,
				client_requirement,
				enabled,
				added_at,
				modified_at
			)
			VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
			ON CONFLICT (content_set_id, file_id)
				WHERE file_id IS NOT NULL
			DO UPDATE SET
				instance_id = excluded.instance_id,
				project_type = excluded.project_type,
				project_id = excluded.project_id,
				version_id = excluded.version_id,
				source_kind = excluded.source_kind,
				server_requirement = excluded.server_requirement,
				client_requirement = excluded.client_requirement,
				enabled = excluded.enabled,
				modified_at = excluded.modified_at
			RETURNING
				id,
				instance_id,
				content_set_id,
				file_id,
				project_type AS \"project_type!\",
				project_id,
				version_id,
				source_kind,
				server_requirement,
				client_requirement,
				enabled,
				sides_unlocked,
				server_override,
				added_at,
				modified_at
			",
            id,
            input.instance_id,
            input.content_set_id,
            file_id,
            project_type,
            input.project_id,
            input.version_id,
            source_kind,
            server_requirement,
            client_requirement,
            enabled,
            now,
            now,
        )
        .fetch_one(&mut **tx)
        .await?
    } else if let (Some(project_id), Some(version_id)) =
        (input.project_id, input.version_id)
    {
        sqlx::query_as!(
            ContentEntryRow,
            "
			INSERT INTO instance_content_entries (
				id,
				instance_id,
				content_set_id,
				file_id,
				project_type,
				project_id,
				version_id,
				source_kind,
				server_requirement,
				client_requirement,
				enabled,
				added_at,
				modified_at
			)
			VALUES (?, ?, ?, NULL, ?, ?, ?, ?, ?, ?, ?, ?, ?)
			ON CONFLICT (content_set_id, project_id, version_id)
				WHERE file_id IS NULL
					AND project_id IS NOT NULL
					AND version_id IS NOT NULL
			DO UPDATE SET
				instance_id = excluded.instance_id,
				project_type = excluded.project_type,
				source_kind = excluded.source_kind,
				server_requirement = excluded.server_requirement,
				client_requirement = excluded.client_requirement,
				enabled = excluded.enabled,
				modified_at = excluded.modified_at
			RETURNING
				id,
				instance_id,
				content_set_id,
				file_id,
				project_type AS \"project_type!\",
				project_id,
				version_id,
				source_kind,
				server_requirement,
				client_requirement,
				enabled,
				sides_unlocked,
				server_override,
				added_at,
				modified_at
			",
            id,
            input.instance_id,
            input.content_set_id,
            project_type,
            project_id,
            version_id,
            source_kind,
            server_requirement,
            client_requirement,
            enabled,
            now,
            now,
        )
        .fetch_one(&mut **tx)
        .await?
    } else {
        sqlx::query_as!(
            ContentEntryRow,
            "
			INSERT INTO instance_content_entries (
				id,
				instance_id,
				content_set_id,
				file_id,
				project_type,
				project_id,
				version_id,
				source_kind,
				server_requirement,
				client_requirement,
				enabled,
				added_at,
				modified_at
			)
			VALUES (?, ?, ?, NULL, ?, ?, ?, ?, ?, ?, ?, ?, ?)
			RETURNING
				id,
				instance_id,
				content_set_id,
				file_id,
				project_type AS \"project_type!\",
				project_id,
				version_id,
				source_kind,
				server_requirement,
				client_requirement,
				enabled,
				sides_unlocked,
				server_override,
				added_at,
				modified_at
			",
            id,
            input.instance_id,
            input.content_set_id,
            project_type,
            input.project_id,
            input.version_id,
            source_kind,
            server_requirement,
            client_requirement,
            enabled,
            now,
            now,
        )
        .fetch_one(&mut **tx)
        .await?
    };

    row.try_into()
}

pub(crate) async fn set_content_entry_enabled_for_file(
    content_set_id: &str,
    file_id: &str,
    enabled: bool,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<bool> {
    let enabled = i64::from(enabled);
    let modified_at = Utc::now().timestamp();

    let result = sqlx::query!(
        "
		UPDATE instance_content_entries
		SET enabled = ?, modified_at = ?
		WHERE content_set_id = ? AND file_id = ?
		",
        enabled,
        modified_at,
        content_set_id,
        file_id,
    )
    .execute(&mut **tx)
    .await?;

    Ok(result.rows_affected() > 0)
}

pub(crate) async fn remove_content_entries_for_file(
    content_set_id: &str,
    file_id: &str,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    sqlx::query!(
        "
		DELETE FROM instance_content_entries
		WHERE content_set_id = ? AND file_id = ?
		",
        content_set_id,
        file_id,
    )
    .execute(&mut **tx)
    .await?;

    Ok(())
}

pub(crate) async fn upsert_content_update_check(
    content_entry_id: &str,
    update_channel: ReleaseChannel,
    update_version_id: Option<&str>,
    pool: &SqlitePool,
) -> crate::Result<()> {
    let update_channel = update_channel.key();
    let checked_at = Utc::now().timestamp();

    sqlx::query!(
        "
		INSERT INTO instance_content_update_checks (
			content_entry_id,
			update_channel,
			update_version_id,
			checked_at
		)
		VALUES (?, ?, ?, ?)
		ON CONFLICT(content_entry_id) DO UPDATE SET
			update_channel = excluded.update_channel,
			update_version_id = excluded.update_version_id,
			checked_at = excluded.checked_at
		",
        content_entry_id,
        update_channel,
        update_version_id,
        checked_at,
    )
    .execute(pool)
    .await?;

    Ok(())
}

// A server engine's record of the files it tracks: `instance_files` joined with the entry that
// says where each came from. Rows of the pack's client-only files are `missing`.

/// A file a server engine tracks, by its enabled path. This is what pack installs read and
/// write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TrackedFile {
    pub relative_path: String,
    /// `Local` for a file someone on the server added, or the source kind of the pack's set.
    pub source_kind: ContentSourceKind,
    /// As the engine placed it.
    pub sha1: String,
    /// As the engine placed it; 0 for a file it found there rather than placed.
    pub size: u64,
    pub project_id: Option<String>,
    pub version_id: Option<String>,
}

impl TrackedFile {
    /// Whether the pack the server follows put it there.
    pub fn by_pack(&self) -> bool {
        self.source_kind != ContentSourceKind::Local
    }
}

/// A file of the pack the server leaves out because it is client only. It is never on the
/// server's disk; the content list shows it as not installed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SkippedFile {
    pub relative_path: String,
    pub sha1: String,
    pub size: u64,
    pub project_id: Option<String>,
    pub version_id: Option<String>,
}

/// Which sides a tracked file is on for, from its entry. A file the engine does not track is on
/// for both, the default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EntrySides {
    /// Off for both sides: the file is `.disabled` on disk.
    pub enabled: bool,
    /// `Unsupported`: the file is `.disabled` on disk, and players still get it.
    pub server_requirement: ContentRequirement,
    /// `Unsupported`: the server runs it, and its share leaves it out.
    pub client_requirement: ContentRequirement,
    /// Someone set the sides by hand.
    pub unlocked: bool,
    /// A client-only file of the pack that someone turned on for the server, so it was placed.
    /// Pack updates keep it; turning the server side off makes it a [`SkippedFile`] again.
    pub server_override: bool,
}

impl Default for EntrySides {
    fn default() -> Self {
        Self {
            enabled: true,
            server_requirement: ContentRequirement::Required,
            client_requirement: ContentRequirement::Required,
            unlocked: false,
            server_override: false,
        }
    }
}

/// The pack files someone turned on for the server although they are client only: by path, and
/// by Modrinth project for the ones that have one, so a pack update that renames a file (a new
/// version's jar is usually named for it) keeps it on.
#[derive(Debug, Clone, Default)]
pub(crate) struct ServerOn {
    pub paths: HashSet<String>,
    pub projects: HashSet<String>,
}

#[derive(sqlx::FromRow)]
struct TrackedRow {
    relative_path: String,
    sha1: String,
    size: i64,
    source_kind: String,
    project_id: Option<String>,
    version_id: Option<String>,
    enabled: bool,
    server_requirement: String,
    client_requirement: String,
    sides_unlocked: bool,
    server_override: bool,
}

const TRACKED: &str = "SELECT f.relative_path, f.sha1, f.size, e.source_kind, \
     e.project_id, e.version_id, e.enabled, e.server_requirement, e.client_requirement, \
     e.sides_unlocked, e.server_override FROM instance_files f \
     JOIN instance_content_entries e ON e.file_id = f.id \
     WHERE f.instance_id = ? AND f.missing = 0";

impl TryFrom<TrackedRow> for (TrackedFile, EntrySides) {
    type Error = crate::Error;

    fn try_from(row: TrackedRow) -> crate::Result<Self> {
        Ok((
            TrackedFile {
                relative_path: row.relative_path,
                source_kind: ContentSourceKind::from_str(&row.source_kind)?,
                sha1: row.sha1,
                size: u64::try_from(row.size).unwrap_or_default(),
                project_id: row.project_id,
                version_id: row.version_id,
            },
            EntrySides {
                enabled: row.enabled,
                server_requirement: ContentRequirement::from_str(
                    &row.server_requirement,
                )?,
                client_requirement: ContentRequirement::from_str(
                    &row.client_requirement,
                )?,
                unlocked: row.sides_unlocked,
                server_override: row.server_override,
            },
        ))
    }
}

fn server_file_name(relative_path: &str) -> &str {
    relative_path.rsplit('/').next().unwrap_or(relative_path)
}

fn server_signed(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

/// Every file the engine tracks on the server, with its sides, by path. The pack's client-only
/// files are not among them.
pub(crate) async fn tracked_files(
    pool: &SqlitePool,
    instance_id: &str,
) -> crate::Result<Vec<(TrackedFile, EntrySides)>> {
    let rows: Vec<TrackedRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "{TRACKED} ORDER BY f.relative_path"
    )))
    .bind(instance_id)
    .fetch_all(pool)
    .await?;
    rows.into_iter().map(TryInto::try_into).collect()
}

pub(crate) async fn tracked_file(
    pool: &SqlitePool,
    instance_id: &str,
    relative_path: &str,
) -> crate::Result<Option<(TrackedFile, EntrySides)>> {
    let row: Option<TrackedRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "{TRACKED} AND f.relative_path = ?"
    )))
    .bind(instance_id)
    .bind(relative_path)
    .fetch_optional(pool)
    .await?;
    row.map(TryInto::try_into).transpose()
}

async fn remove_file_rows(
    tx: &mut SqliteConnection,
    instance_id: &str,
    relative_path: &str,
    missing: bool,
) -> sqlx::Result<()> {
    sqlx::query(
        "DELETE FROM instance_content_entries WHERE file_id IN (SELECT id FROM instance_files \
         WHERE instance_id = ? AND relative_path = ? AND missing = ?)",
    )
    .bind(instance_id)
    .bind(relative_path)
    .bind(missing)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "DELETE FROM instance_files WHERE instance_id = ? AND relative_path = ? AND missing = ?",
    )
    .bind(instance_id)
    .bind(relative_path)
    .bind(missing)
    .execute(&mut *tx)
    .await?;
    Ok(())
}

/// Record a file the engine placed or found, replacing what was recorded for its path but its
/// sides, which stay with the path. A client-only row of the pack at the path gives way to it.
pub(crate) async fn upsert_tracked_file(
    pool: &SqlitePool,
    instance_id: &str,
    content_set_id: &str,
    file: &TrackedFile,
) -> crate::Result<()> {
    let now = Utc::now().timestamp();
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    remove_file_rows(&mut tx, instance_id, &file.relative_path, true).await?;
    let file_id: String = sqlx::query_scalar(
        "INSERT INTO instance_files (id, instance_id, relative_path, file_name, enabled, sha1, \
         size, missing, added_at, modified_at) VALUES (?, ?, ?, ?, 1, ?, ?, 0, ?, ?) \
         ON CONFLICT (instance_id, relative_path) DO UPDATE SET sha1 = excluded.sha1, \
         size = excluded.size, modified_at = excluded.modified_at \
         RETURNING id",
    )
    .bind(format!("instance-file:{}", Uuid::new_v4()))
    .bind(instance_id)
    .bind(&file.relative_path)
    .bind(server_file_name(&file.relative_path))
    .bind(&file.sha1)
    .bind(server_signed(file.size))
    .bind(now)
    .bind(now)
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE instance_content_entries SET content_set_id = ? WHERE file_id = ?",
    )
    .bind(content_set_id)
    .bind(&file_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO instance_content_entries (id, instance_id, content_set_id, file_id, \
         project_type, project_id, version_id, source_kind, server_requirement, \
         client_requirement, enabled, added_at, modified_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'required', 'required', 1, ?, ?) \
         ON CONFLICT (content_set_id, file_id) WHERE file_id IS NOT NULL DO UPDATE SET \
         project_type = excluded.project_type, project_id = excluded.project_id, \
         version_id = excluded.version_id, source_kind = excluded.source_kind, \
         modified_at = excluded.modified_at",
    )
    .bind(format!("content-entry:{}", Uuid::new_v4()))
    .bind(instance_id)
    .bind(content_set_id)
    .bind(&file_id)
    .bind(server_addon_type(&file.relative_path).map(|kind| kind.get_name()))
    .bind(&file.project_id)
    .bind(&file.version_id)
    .bind(file.source_kind.as_str())
    .bind(now)
    .bind(now)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

/// Forget a tracked file, its sides with it.
pub(crate) async fn remove_tracked_file(
    pool: &SqlitePool,
    instance_id: &str,
    relative_path: &str,
) -> crate::Result<()> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    remove_file_rows(&mut tx, instance_id, relative_path, false).await?;
    tx.commit().await?;
    Ok(())
}

/// Record a tracked file's sides. `false` when the engine does not track the file.
pub(crate) async fn set_entry_sides(
    pool: &SqlitePool,
    instance_id: &str,
    relative_path: &str,
    sides: &EntrySides,
) -> crate::Result<bool> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let file_id: Option<String> = sqlx::query_scalar(
        "SELECT id FROM instance_files WHERE instance_id = ? AND relative_path = ? AND missing = 0",
    )
    .bind(instance_id)
    .bind(relative_path)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(file_id) = file_id else {
        return Ok(false);
    };
    let now = Utc::now().timestamp();
    sqlx::query(
        "UPDATE instance_content_entries SET enabled = ?, server_requirement = ?, \
         client_requirement = ?, sides_unlocked = ?, server_override = ?, modified_at = ? \
         WHERE file_id = ?",
    )
    .bind(sides.enabled)
    .bind(sides.server_requirement.as_str())
    .bind(sides.client_requirement.as_str())
    .bind(sides.unlocked)
    .bind(sides.server_override)
    .bind(now)
    .bind(&file_id)
    .execute(&mut *tx)
    .await?;
    // The file runs on the server unless it is off for both or for the server alone.
    sqlx::query("UPDATE instance_files SET enabled = ? WHERE id = ?")
        .bind(
            sides.enabled
                && sides.server_requirement != ContentRequirement::Unsupported,
        )
        .bind(&file_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(true)
}

/// Record the client-only files of the pack the server follows, in place of those recorded
/// before. A path the engine already tracks keeps its row.
pub(crate) async fn set_skipped_files(
    pool: &SqlitePool,
    instance_id: &str,
    content_set_id: &str,
    skipped: &[SkippedFile],
) -> crate::Result<()> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    sqlx::query(
        "DELETE FROM instance_content_entries WHERE file_id IN (SELECT id FROM instance_files \
         WHERE instance_id = ? AND missing = 1)",
    )
    .bind(instance_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "DELETE FROM instance_files WHERE instance_id = ? AND missing = 1",
    )
    .bind(instance_id)
    .execute(&mut *tx)
    .await?;
    for file in skipped {
        insert_skipped_file(&mut tx, instance_id, content_set_id, file).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Record one client-only file of the pack, unless the engine tracks its path.
async fn insert_skipped_file(
    tx: &mut SqliteConnection,
    instance_id: &str,
    content_set_id: &str,
    file: &SkippedFile,
) -> crate::Result<()> {
    let now = Utc::now().timestamp();
    let file_id: Option<String> = sqlx::query_scalar(
        "INSERT OR IGNORE INTO instance_files (id, instance_id, relative_path, file_name, \
         enabled, sha1, size, missing, added_at, modified_at) \
         VALUES (?, ?, ?, ?, 0, ?, ?, 1, ?, ?) RETURNING id",
    )
    .bind(format!("instance-file:{}", Uuid::new_v4()))
    .bind(instance_id)
    .bind(&file.relative_path)
    .bind(server_file_name(&file.relative_path))
    .bind(&file.sha1)
    .bind(server_signed(file.size))
    .bind(now)
    .bind(now)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(file_id) = file_id else {
        return Ok(());
    };
    sqlx::query(
        "INSERT INTO instance_content_entries (id, instance_id, content_set_id, file_id, \
         project_type, project_id, version_id, source_kind, server_requirement, \
         client_requirement, enabled, added_at, modified_at) \
         SELECT ?, ?, id, ?, ?, ?, ?, source_kind, 'unsupported', 'required', 1, ?, ? \
         FROM instance_content_sets WHERE id = ?",
    )
    .bind(format!("content-entry:{}", Uuid::new_v4()))
    .bind(instance_id)
    .bind(&file_id)
    .bind(server_addon_type(&file.relative_path).map(|kind| kind.get_name()))
    .bind(&file.project_id)
    .bind(&file.version_id)
    .bind(now)
    .bind(now)
    .bind(content_set_id)
    .execute(&mut *tx)
    .await?;
    Ok(())
}

/// Forget the tracked file at `file`'s path and record it as a client-only file of the pack
/// again: someone turned the server side of an [`EntrySides::server_override`] file off.
pub(crate) async fn return_to_client_only(
    pool: &SqlitePool,
    instance_id: &str,
    content_set_id: &str,
    file: &SkippedFile,
) -> crate::Result<()> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    remove_file_rows(&mut tx, instance_id, &file.relative_path, false).await?;
    insert_skipped_file(&mut tx, instance_id, content_set_id, file).await?;
    tx.commit().await?;
    Ok(())
}

/// The pack's client-only files someone turned on for the server.
pub(crate) async fn server_overrides(
    pool: &SqlitePool,
    instance_id: &str,
) -> crate::Result<ServerOn> {
    let rows: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT f.relative_path, e.project_id FROM instance_files f \
         JOIN instance_content_entries e ON e.file_id = f.id \
         WHERE f.instance_id = ? AND f.missing = 0 AND e.server_override = 1 \
         AND e.source_kind != 'local'",
    )
    .bind(instance_id)
    .fetch_all(pool)
    .await?;
    let mut on = ServerOn::default();
    for (path, project_id) in rows {
        on.paths.insert(path);
        on.projects.extend(project_id);
    }
    Ok(on)
}

/// Mark the pack files at `paths` as turned on for the server by hand
/// ([`EntrySides::server_override`]): a pack update placed them only because they were.
pub(crate) async fn set_server_overrides(
    pool: &SqlitePool,
    instance_id: &str,
    paths: &HashSet<String>,
) -> crate::Result<()> {
    for path in paths {
        sqlx::query(
            "UPDATE instance_content_entries SET server_override = 1, sides_unlocked = 1 \
             WHERE file_id = (SELECT id FROM instance_files \
             WHERE instance_id = ? AND relative_path = ? AND missing = 0)",
        )
        .bind(instance_id)
        .bind(path)
        .execute(pool)
        .await?;
    }
    Ok(())
}

/// The client-only files of the pack the server follows, by path.
pub(crate) async fn skipped_files(
    pool: &SqlitePool,
    instance_id: &str,
) -> crate::Result<Vec<SkippedFile>> {
    let rows: Vec<(String, String, i64, Option<String>, Option<String>)> =
        sqlx::query_as(
            "SELECT f.relative_path, f.sha1, f.size, e.project_id, e.version_id \
             FROM instance_files f JOIN instance_content_entries e ON e.file_id = f.id \
             WHERE f.instance_id = ? AND f.missing = 1 ORDER BY f.relative_path",
        )
        .bind(instance_id)
        .fetch_all(pool)
        .await?;
    Ok(rows
        .into_iter()
        .map(|(relative_path, sha1, size, project_id, version_id)| {
            SkippedFile {
                relative_path,
                sha1,
                size: u64::try_from(size).unwrap_or_default(),
                project_id,
                version_id,
            }
        })
        .collect())
}

fn project_type_from_str(value: &str) -> crate::Result<ProjectType> {
    ProjectType::from_name(value).ok_or(
        ErrorKind::InputError(format!("unknown content project type {value}"))
            .into(),
    )
}

fn timestamp(value: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(value, 0)
        .single()
        .unwrap_or_else(Utc::now)
}

fn optional_timestamp(value: i64) -> Option<DateTime<Utc>> {
    Utc.timestamp_opt(value, 0).single()
}

fn unsigned(value: i64, column: &str) -> crate::Result<u64> {
    if value < 0 {
        bail!(ErrorKind::InputError(format!(
            "expected {column} to be non-negative"
        )));
    }

    Ok(value as u64)
}
