use std::{collections::HashSet, path::Path, sync::Arc};

use uuid::Uuid;

use crate::{
    application::{
        mod_service::{verify_sha512, ModError, ModInfo},
        state::AppState,
    },
    infrastructure::minecraft::modrinth_api::{
        ModrinthClient, ModrinthFile, ModrinthVersion,
    },
};

struct PendingVersion {
    version: ModrinthVersion,
    root: bool,
    install: bool,
}

pub async fn add_mod_version(
    state: &Arc<AppState>,
    instance_id: &str,
    version_id: &str,
) -> Result<ModInfo, ModError> {
    let (data_dir, game_version, loader) =
        super::mod_service::instance_info(state, instance_id).await?;
    let modrinth = ModrinthClient::new(state.http.clone());
    let version = modrinth.get_version(version_id).await?;
    install_with_dependencies(
        state,
        instance_id,
        &data_dir,
        &game_version,
        &loader,
        &modrinth,
        version,
    )
    .await
}

pub async fn add_mod_project(
    state: &Arc<AppState>,
    instance_id: &str,
    project_id: &str,
) -> Result<ModInfo, ModError> {
    let (data_dir, game_version, loader) =
        super::mod_service::instance_info(state, instance_id).await?;
    let modrinth = ModrinthClient::new(state.http.clone());
    let version =
        latest_version(&modrinth, project_id, &game_version, &loader).await?;
    install_with_dependencies(
        state,
        instance_id,
        &data_dir,
        &game_version,
        &loader,
        &modrinth,
        version,
    )
    .await
}

async fn install_with_dependencies(
    state: &Arc<AppState>,
    instance_id: &str,
    data_dir: &Path,
    game_version: &str,
    loader: &str,
    modrinth: &ModrinthClient,
    root: ModrinthVersion,
) -> Result<ModInfo, ModError> {
    let mut seen = HashSet::new();
    let mut root_info = None;
    let mut stack = vec![PendingVersion {
        version: root,
        root: true,
        install: false,
    }];

    while let Some(pending) = stack.pop() {
        if pending.install {
            let info = install_version(
                state,
                instance_id,
                data_dir,
                modrinth,
                &pending.version,
            )
            .await?;
            if pending.root {
                root_info = Some(info);
            }
            continue;
        }

        if !seen.insert(pending.version.id.clone()) {
            continue;
        }

        stack.push(PendingVersion {
            version: pending.version.clone(),
            root: pending.root,
            install: true,
        });
        for dep in pending
            .version
            .dependencies
            .iter()
            .filter(|dep| dep.dependency_type == "required")
        {
            if let Some(version_id) = &dep.version_id {
                if !version_installed(state, instance_id, version_id).await? {
                    stack.push(PendingVersion {
                        version: modrinth.get_version(version_id).await?,
                        root: false,
                        install: false,
                    });
                }
            } else if let Some(project_id) = &dep.project_id {
                if !project_installed(state, instance_id, project_id).await? {
                    let version = latest_version(
                        modrinth,
                        project_id,
                        game_version,
                        loader,
                    )
                    .await?;
                    stack.push(PendingVersion {
                        version,
                        root: false,
                        install: false,
                    });
                }
            }
        }
    }

    root_info.ok_or(ModError::ModNotFound)
}

async fn latest_version(
    modrinth: &ModrinthClient,
    project_id: &str,
    game_version: &str,
    loader: &str,
) -> Result<ModrinthVersion, ModError> {
    modrinth
        .list_versions(project_id, Some(game_version), Some(loader))
        .await?
        .into_iter()
        .next()
        .ok_or(ModError::ModNotFound)
}

async fn install_version(
    state: &Arc<AppState>,
    instance_id: &str,
    data_dir: &Path,
    modrinth: &ModrinthClient,
    version: &ModrinthVersion,
) -> Result<ModInfo, ModError> {
    let project = modrinth.get_project(&version.project_id).await?;
    if project.server_side.as_deref() == Some("unsupported") {
        return Err(ModError::ClientOnly);
    }
    let file = primary_file(version)?;
    sanitize_filename(&file.filename)?;
    let bytes = state
        .http
        .get(&file.url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    let sha512 =
        verify_sha512(&file.filename, &bytes, file.hashes.sha512.as_deref())?;
    let mods_dir = data_dir.join("mods");
    tokio::fs::create_dir_all(&mods_dir).await?;
    tokio::fs::write(mods_dir.join(&file.filename), &bytes).await?;
    let id = Uuid::new_v4().to_string();
    sqlx::query("INSERT OR REPLACE INTO mods (id,instance_id,filename,display_name,modrinth_project_id,modrinth_version_id,version_number,client_side,server_side,sha512,enabled,installed_at) VALUES (?,?,?,?,?,?,?,?,?,?,1,?)")
		.bind(&id).bind(instance_id).bind(&file.filename).bind(&project.title)
		.bind(&version.project_id).bind(&version.id).bind(&version.version_number)
		.bind(project.client_side.as_deref()).bind(project.server_side.as_deref())
		.bind(&sha512).bind(chrono::Utc::now().to_rfc3339()).execute(&state.pool).await?;
    Ok(ModInfo {
        id: Some(id),
        filename: file.filename.clone(),
        display_name: Some(project.title),
        version_number: Some(version.version_number.clone()),
        enabled: true,
        tracked: true,
        client_side: project.client_side,
        server_side: project.server_side,
        modrinth_project_id: Some(version.project_id.clone()),
        modrinth_version_id: Some(version.id.clone()),
        update_available: Some(false),
    })
}

fn sanitize_filename(name: &str) -> Result<(), ModError> {
    if name.is_empty()
        || name.contains("..")
        || name.contains('/')
        || name.contains('\\')
    {
        return Err(ModError::InvalidFilename);
    }
    Ok(())
}

fn primary_file(version: &ModrinthVersion) -> Result<&ModrinthFile, ModError> {
    version
        .files
        .iter()
        .find(|file| file.primary)
        .or_else(|| version.files.first())
        .ok_or(ModError::ModNotFound)
}

async fn version_installed(
    state: &Arc<AppState>,
    instance_id: &str,
    version_id: &str,
) -> Result<bool, ModError> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mods WHERE instance_id=? AND modrinth_version_id=?")
		.bind(instance_id).bind(version_id).fetch_one(&state.pool).await?;
    Ok(count > 0)
}

async fn project_installed(
    state: &Arc<AppState>,
    instance_id: &str,
    project_id: &str,
) -> Result<bool, ModError> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mods WHERE instance_id=? AND modrinth_project_id=?")
		.bind(instance_id).bind(project_id).fetch_one(&state.pool).await?;
    Ok(count > 0)
}
