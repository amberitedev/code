use super::{PackLibrary, SyncedPack};
use crate::ErrorKind;
use crate::state::content_store::{
    StoredFileHandle, content_file_path, hash_file, validate_instance_path,
    validate_relative,
};
use crate::state::instances::adapters::sqlite::{content_rows, instance_rows};
use crate::state::{CachedEntry, Engine};
use crate::util::io;
use anyhow::bail;
use bytes::Bytes;
use std::path::PathBuf;

fn directory(state: &Engine) -> PathBuf {
    state.directories.synced_options_dir().join("packs")
}

pub(super) async fn read_library(state: &Engine) -> crate::Result<PackLibrary> {
    match io::read(directory(state).join("packs.json")).await {
        Ok(bytes) => {
            let mut library: PackLibrary = serde_json::from_slice(&bytes)?;
            for pack in library.packs.values_mut() {
                pack.item.file_name = pack
                    .item
                    .file_name
                    .trim_end_matches(".disabled")
                    .to_string();
                pack.item.file_path = pack
                    .item
                    .file_path
                    .trim_end_matches(".disabled")
                    .to_string();
            }
            for placements in library.instances.values_mut() {
                for placement in placements.values_mut() {
                    placement.path = placement
                        .path
                        .trim_end_matches(".disabled")
                        .to_string();
                    if let Some(path) =
                        &mut placement.resource_pack_selection_path
                    {
                        *path = path.trim_end_matches(".disabled").to_string();
                    }
                }
            }
            Ok(library)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(PackLibrary::default())
        }
        Err(error) => Err(error.into()),
    }
}

pub(super) async fn write_library(
    library: &PackLibrary,
    state: &Engine,
) -> crate::Result<()> {
    let _lease = state.content_store.lease().await;
    let previous = read_library(state).await?;
    let mut renamed = false;
    let result: crate::Result<()> = async {
		for (id, pack) in &library.packs {
			let saved = previous.packs.get(id).is_some_and(|previous| {
				previous.sha1 == pack.sha1 && previous.blob_sha512 == pack.blob_sha512
			});
			let stored_file = match state.content_store.lookup(
				Some(&pack.blob_sha512),
				Some(pack.item.size),
			).await? {
				Some(stored_file) => stored_file,
				// Stays retained; read_stored_file restores the object when an instance needs it.
				None if saved => continue,
				None => bail!(ErrorKind::InputError(
					"a synced pack needs repair or re-import before its library can be saved".to_string(),
				)),
			};
			state.content_store.retain(
				"synced-pack",
				id,
				std::slice::from_ref(&stored_file.metadata.sha512),
			).await?;
		}
		io::create_dir_all(directory(state)).await?;
		let destination = directory(state).join("packs.json");
		let bytes = serde_json::to_vec(&library)?;
		if !io::read(&destination).await.is_ok_and(|current| current == bytes) {
			let temporary = directory(state)
				.join(format!(".packs-{}.tmp", uuid::Uuid::new_v4()));
			io::write(&temporary, bytes).await?;
			tokio::fs::File::options()
				.write(true)
				.open(&temporary)
				.await?
				.sync_all()
				.await?;
			tokio::fs::rename(&temporary, &destination).await?;
			renamed = true;
			state.synced_options.packs.revision
				.fetch_add(1, std::sync::atomic::Ordering::Release);
		}
		crate::state::content_store::sync_directory(&directory(state)).await?;
		for id in previous.packs.keys().filter(|id| !library.packs.contains_key(*id)) {
			state.content_store.release("synced-pack", id).await?;
		}
		for (id, pack) in &library.packs {
			let hash = &pack.blob_sha512;
			state.content_store.replace_retained(
				"synced-pack", id, std::slice::from_ref(hash),
			).await?;
			state.content_store.release("synced-cache", hash).await?;
		}
		Ok(())
	}.await;
    if result.is_err() {
        let repair: crate::Result<()> = async {
            // Keep both versions retained until the renamed library is durable.
            if renamed {
                crate::state::content_store::sync_directory(&directory(state))
                    .await?;
            }
            let durable = read_library(state).await?;
            for id in previous.packs.keys().chain(library.packs.keys()) {
                let hashes = durable
                    .packs
                    .get(id)
                    .map(|pack| pack.blob_sha512.clone())
                    .into_iter()
                    .collect::<Vec<_>>();
                state
                    .content_store
                    .replace_retained("synced-pack", id, &hashes)
                    .await?;
            }
            Ok(())
        }
        .await;
        if let Err(error) = repair {
            tracing::warn!(%error, "Synced pack retention will be reconciled on the next retention pass");
        }
    }
    result
}

pub(super) async fn cache_bytes(
    bytes: Bytes,
    state: &Engine,
) -> crate::Result<String> {
    let stored_file = state.content_store.store_bytes(&bytes).await?;
    state
        .content_store
        .retain(
            "synced-cache",
            &stored_file.metadata.sha512,
            std::slice::from_ref(&stored_file.metadata.sha512),
        )
        .await?;
    Ok(stored_file.metadata.sha512.clone())
}

pub(super) async fn read_stored_file(
    pack: &SyncedPack,
    state: &Engine,
) -> crate::Result<StoredFileHandle> {
    if let Some(stored_file) = state
        .content_store
        .lookup(Some(&pack.blob_sha512), Some(pack.item.size))
        .await?
    {
        return Ok(stored_file);
    }
    let library = read_library(state).await?;
    let id = library
        .packs
        .iter()
        .find(|(_, candidate)| candidate.sha1 == pack.sha1)
        .map(|(id, _)| id.as_str());
    match recover_pack_from_instances(id, pack, &library, state).await {
        Ok(stored_file) => return Ok(stored_file),
        Err(error) => {
            tracing::debug!("Local synced-pack recovery deferred: {error}")
        }
    }
    let version = pack.item.version.as_ref().ok_or(ErrorKind::InputError(
        "the synced pack is missing or damaged; repair or re-import it"
            .to_string(),
    ))?;
    let version = CachedEntry::get_version(
        &version.id,
        None,
        &state.pool,
        &state.api_semaphore,
        &state.sessions,
    )
    .await?
    .ok_or(ErrorKind::InputError(
        "the synced pack's original version is unavailable; re-import it"
            .to_string(),
    ))?;
    let file = version
        .files
        .iter()
        .find(|file| file.hashes.get("sha512") == Some(&pack.blob_sha512))
        .ok_or(ErrorKind::InputError(
            "the synced pack's original file is unavailable; re-import it"
                .to_string(),
        ))?;
    state
        .content_store
        .require_staging_space(u64::from(file.size))?;
    Ok(state
        .content_store
        .get_or_download_file(
            &[file.url.as_str()],
            file.hashes.get("sha512").map(String::as_str),
            Some(u64::from(file.size)),
            None,
            &state.fetch_semaphore,
            &state.sessions,
            None,
        )
        .await?
        .stored_file)
}

pub(crate) async fn reconcile_retention(state: &Engine) -> crate::Result<()> {
    let _guard = state.synced_options.lock().await;
    if tokio::fs::try_exists(directory(state)).await? {
        crate::state::content_store::sync_directory(&directory(state)).await?;
    }
    let library = read_library(state).await?;
    for owner in
        crate::state::content_store::retained_owners(&state.pool, "synced-pack")
            .await?
    {
        let hashes = library
            .packs
            .get(&owner)
            .map(|pack| pack.blob_sha512.clone())
            .into_iter()
            .collect::<Vec<_>>();
        state
            .content_store
            .replace_retained("synced-pack", &owner, &hashes)
            .await?;
    }
    for owner in crate::state::content_store::retained_owners(
        &state.pool,
        "synced-cache",
    )
    .await?
    {
        if !library.packs.values().any(|pack| pack.blob_sha512 == owner) {
            state.content_store.release("synced-cache", &owner).await?;
        }
    }
    Ok(())
}

async fn recover_pack_from_instances(
    id: Option<&str>,
    pack: &SyncedPack,
    library: &PackLibrary,
    state: &Engine,
) -> crate::Result<StoredFileHandle> {
    for instance in instance_rows::list_instances(&state.pool).await? {
        validate_instance_path(&instance.path)?;
        let base = state.directories.instances_dir().join(&instance.path);
        if let Some(placement) = library
            .instances
            .get(&instance.id)
            .and_then(|placements| id.and_then(|id| placements.get(id)))
            && !placement.path.is_empty()
            && validate_relative(&placement.path).is_ok()
        {
            let source =
                base.join(crate::state::content_store::file_path_on_disk(
                    &placement.path,
                    placement.enabled,
                ));
            if let Some(stored_file) =
                import_matching_pack(&source, pack, state).await?
            {
                return Ok(stored_file);
            }
        }
        for file in
            content_rows::get_instance_files(&instance.id, &state.pool).await?
        {
            if file.sha1 == pack.sha1
                && validate_relative(&file.relative_path).is_ok()
                && let Some(stored_file) = import_matching_pack(
                    &base.join(content_file_path(&file)),
                    pack,
                    state,
                )
                .await?
            {
                return Ok(stored_file);
            }
        }
    }
    bail!(ErrorKind::InputError(format!(
        "synced pack {} is missing or damaged; restore or re-import it",
        pack.item.file_name
    )))
}

async fn import_matching_pack(
    source: &std::path::Path,
    pack: &SyncedPack,
    state: &Engine,
) -> crate::Result<Option<StoredFileHandle>> {
    let Ok(crate::state::content_store::FileHashes { sha512, .. }) =
        hash_file(source).await
    else {
        return Ok(None);
    };
    if sha512 != pack.blob_sha512 {
        return Ok(None);
    }
    let stored_file = state.content_store.store_file(source).await?;
    if stored_file.metadata.sha512 != sha512 {
        bail!(ErrorKind::InputError(
            "the synced pack changed while it was being restored; retry after closing the instance"
                .to_string(),
        ));
    }
    Ok(Some(stored_file))
}
