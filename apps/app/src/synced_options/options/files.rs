use crate::synced_options::state::SyncedInstance;
pub(crate) use engine::server_list::{
    nbt_from_bytes, nbt_to_bytes, read_nbt_file,
};
use engine::state::{InstanceInstallStage, InstanceMetadata, SyncedOption};
use engine::util::io;
use engine::{Engine, ErrorKind};
use sha1_smol::Sha1;
use std::path::{Path, PathBuf};

pub(in crate::synced_options) struct SyncCheckpoint {
    pub expected_sha1: String,
    pub merge_base: Option<Vec<u8>>,
    pub source_revision: i64,
    pub status: CheckpointStatus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::synced_options) enum CheckpointStatus {
    Pending,
    Ready,
}

impl CheckpointStatus {
    fn from_str(value: &str) -> Option<Self> {
        match value {
            "pending" => Some(Self::Pending),
            "ready" => Some(Self::Ready),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(in crate::synced_options) enum LinkMode {
    Symbolic,
    #[cfg(windows)]
    Hard,
    #[cfg(windows)]
    Copy,
}

impl LinkMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Symbolic => "symbolic",
            #[cfg(windows)]
            Self::Hard => "hard",
            #[cfg(windows)]
            Self::Copy => "copy",
        }
    }
}

pub(in crate::synced_options) fn safe_instance_id(instance_id: &str) -> String {
    instance_id.replace([':', '/', '\\'], "_")
}

pub(in crate::synced_options) fn instance_dir(
    metadata: &InstanceMetadata,
    state: &Engine,
) -> PathBuf {
    state
        .directories
        .instances_dir()
        .join(&metadata.instance.path)
}

pub(in crate::synced_options) fn sync_files_are_protected(
    metadata: &InstanceMetadata,
) -> bool {
    matches!(
        metadata.instance.install_stage,
        InstanceInstallStage::MinecraftInstalling
            | InstanceInstallStage::PackInstalling
    )
}

pub(in crate::synced_options) async fn instance_is_running(
    metadata: &InstanceMetadata,
    state: &Engine,
) -> engine::Result<bool> {
    engine::state::instance_has_running_process(&metadata.instance.id, state)
        .await
}

pub(in crate::synced_options) fn option_can_apply_while_running(
    option: SyncedOption,
) -> bool {
    matches!(
        option,
        SyncedOption::GameOptions
            | SyncedOption::Screenshots
            | SyncedOption::ResourcePacks
    )
}

pub(in crate::synced_options) fn instance_option_enabled(
    metadata: &SyncedInstance,
    option: SyncedOption,
) -> bool {
    match option {
        SyncedOption::GameOptions => metadata.synced.game_options,
        SyncedOption::CommandHistory => metadata.synced.command_history,
        SyncedOption::MultiplayerServers => metadata.synced.multiplayer_servers,
        SyncedOption::CreativeHotbars => metadata.synced.creative_hotbars,
        SyncedOption::Screenshots => metadata.synced.screenshots,
        SyncedOption::ResourcePacks => metadata.synced.resource_packs,
        SyncedOption::DataPacks => metadata.synced.data_packs,
    }
}

pub(in crate::synced_options) async fn ensure_link(
    source: &Path,
    target: &Path,
) -> engine::Result<LinkMode> {
    if let Some(parent) = target.parent() {
        io::create_dir_all(parent).await?;
    }
    if tokio::fs::symlink_metadata(target)
        .await
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
        && tokio::fs::read_link(target)
            .await
            .is_ok_and(|current| current == source)
    {
        return Ok(LinkMode::Symbolic);
    }
    if tokio::fs::symlink_metadata(target).await.is_ok() {
        io::remove_file(target).await?;
    }

    #[cfg(unix)]
    {
        tokio::fs::symlink(source, target).await?;
        Ok(LinkMode::Symbolic)
    }
    #[cfg(windows)]
    {
        if tokio::fs::symlink_file(source, target).await.is_ok() {
            return Ok(LinkMode::Symbolic);
        }
        if tokio::fs::hard_link(source, target).await.is_ok() {
            return Ok(LinkMode::Hard);
        }
        io::copy(source, target).await?;
        Ok(LinkMode::Copy)
    }
}

pub(in crate::synced_options) async fn detach_link(
    source: &Path,
    target: &Path,
) -> engine::Result<()> {
    if tokio::fs::symlink_metadata(target).await.is_err() {
        return Ok(());
    }
    let contents = if target.exists() {
        Some(io::read(target).await?)
    } else if source.exists() {
        Some(io::read(source).await?)
    } else {
        None
    };
    io::remove_file(target).await?;
    if let Some(contents) = contents {
        io::write(target, contents).await?;
    }
    Ok(())
}

pub(in crate::synced_options) async fn begin_checkpoint(
    instance_id: &str,
    option: SyncedOption,
    variant: &str,
    expected_sha1: &str,
    merge_base: Option<&[u8]>,
    source_revision: i64,
    state: &Engine,
) -> engine::Result<()> {
    let option_name = option.as_str();
    sqlx::query!(
        "
		INSERT INTO instance_sync_checkpoints
			(instance_id, feature, variant, expected_sha1, merge_base,
			 source_revision, status, link_mode)
		VALUES (?, ?, ?, ?, ?, ?, 'pending', NULL)
		ON CONFLICT(instance_id, feature, variant) DO UPDATE SET
			expected_sha1 = excluded.expected_sha1,
			merge_base = excluded.merge_base,
			source_revision = excluded.source_revision,
			status = 'pending',
			link_mode = NULL
		",
        instance_id,
        option_name,
        variant,
        expected_sha1,
        merge_base,
        source_revision,
    )
    .execute(crate::db::pool(state))
    .await?;
    Ok(())
}

pub(in crate::synced_options) async fn finish_checkpoint(
    instance_id: &str,
    option: SyncedOption,
    variant: &str,
    mode: LinkMode,
    state: &Engine,
) -> engine::Result<()> {
    let option_name = option.as_str();
    let link_mode = mode.as_str();
    sqlx::query!(
        "
		UPDATE instance_sync_checkpoints
		SET status = 'ready', link_mode = ?
		WHERE instance_id = ? AND feature = ? AND variant = ?
		",
        link_mode,
        instance_id,
        option_name,
        variant,
    )
    .execute(crate::db::pool(state))
    .await?;
    Ok(())
}

pub(in crate::synced_options) async fn finish_plain_checkpoint(
    instance_id: &str,
    option: SyncedOption,
    variant: &str,
    state: &Engine,
) -> engine::Result<()> {
    let option_name = option.as_str();
    sqlx::query!(
        "
		UPDATE instance_sync_checkpoints
		SET status = 'ready', link_mode = NULL
		WHERE instance_id = ? AND feature = ? AND variant = ?
		",
        instance_id,
        option_name,
        variant,
    )
    .execute(crate::db::pool(state))
    .await?;
    Ok(())
}

pub(in crate::synced_options) async fn checkpoint(
    instance_id: &str,
    option: SyncedOption,
    variant: &str,
    state: &Engine,
) -> engine::Result<Option<SyncCheckpoint>> {
    let option_name = option.as_str();
    let row = sqlx::query!(
        r#"
		SELECT expected_sha1, merge_base,
			source_revision AS "source_revision!: i64", status
		FROM instance_sync_checkpoints
		WHERE instance_id = ? AND feature = ? AND variant = ?
		"#,
        instance_id,
        option_name,
        variant,
    )
    .fetch_optional(crate::db::pool(state))
    .await?;
    row.map(|row| {
        Ok(SyncCheckpoint {
            expected_sha1: row.expected_sha1,
            merge_base: row.merge_base,
            source_revision: row.source_revision,
            status: CheckpointStatus::from_str(&row.status).ok_or(
                ErrorKind::InputError(format!(
                    "unknown sync checkpoint status {}",
                    row.status
                )),
            )?,
        })
    })
    .transpose()
}

pub(in crate::synced_options) async fn sha1_file(
    path: &Path,
) -> engine::Result<String> {
    Ok(Sha1::from(io::read(path).await?).digest().to_string())
}

pub(in crate::synced_options) fn sha1_bytes(bytes: &[u8]) -> String {
    Sha1::from(bytes).digest().to_string()
}
