use std::{collections::HashSet, path::PathBuf, sync::Arc};

use sha2::Digest;
use serde::Serialize;
use sqlx::Row;
use uuid::Uuid;

use crate::{
    application::state::AppState,
    infrastructure::minecraft::modrinth_api::{ModrinthClient, ModrinthError},
};

#[derive(Debug, Serialize)]
pub struct ModInfo {
    pub id: Option<String>, pub filename: String, pub display_name: Option<String>,
    pub version_number: Option<String>, pub enabled: bool, pub tracked: bool,
    pub client_side: Option<String>, pub server_side: Option<String>,
    pub modrinth_project_id: Option<String>, pub update_available: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct UpdateAllResult {
    pub updated: Vec<String>,
    pub already_latest: Vec<String>,
    pub failed: Vec<serde_json::Value>,
}

#[derive(Debug, thiserror::Error)]
pub enum ModError {
    #[error("io: {0}")] Io(#[from] std::io::Error),
    #[error("db: {0}")] Db(#[from] sqlx::Error),
    #[error("http: {0}")] Http(#[from] reqwest::Error),
    #[error("modrinth: {0}")] Modrinth(#[from] ModrinthError),
    #[error("instance not found")] InstanceNotFound,
    #[error("mod not found")] ModNotFound,
    #[error("client-only mod")] ClientOnly,
    #[error("no modrinth project id")] NoModrinthId,
}

async fn instance_info(state: &Arc<AppState>, id: &str) -> Result<(PathBuf, String, String), ModError> {
    let row = sqlx::query_as::<_, (String, String, String)>(
        "SELECT data_dir, game_version, loader FROM instances WHERE id = ?",
    ).bind(id).fetch_optional(&state.pool).await?.ok_or(ModError::InstanceNotFound)?;
    Ok((PathBuf::from(row.0), row.1, row.2))
}

pub async fn list_mods(state: &Arc<AppState>, instance_id: &str) -> Result<Vec<ModInfo>, ModError> {
    let (data_dir, _, _) = instance_info(state, instance_id).await?;
    let rows = sqlx::query("SELECT id, filename, display_name, modrinth_project_id, version_number, enabled, client_side, server_side FROM mods WHERE instance_id = ?")
        .bind(instance_id).fetch_all(&state.pool).await?;
    let mut tracked: HashSet<String> = HashSet::new();
    let mut infos: Vec<ModInfo> = rows.iter().map(|r| {
        let fname: String = r.get("filename");
        tracked.insert(fname.clone());
        ModInfo { id: Some(r.get("id")), filename: fname, display_name: r.get("display_name"),
            version_number: r.get("version_number"), enabled: r.get::<i64, _>("enabled") != 0,
            tracked: true, client_side: r.get("client_side"), server_side: r.get("server_side"),
            modrinth_project_id: r.get("modrinth_project_id"), update_available: None }
    }).collect();
    if let Ok(mut rd) = tokio::fs::read_dir(data_dir.join("mods")).await {
        while let Ok(Some(e)) = rd.next_entry().await {
            let n = e.file_name().to_string_lossy().to_string();
            if (n.ends_with(".jar") || n.ends_with(".jar.disabled")) && !tracked.contains(&n) {
                infos.push(ModInfo { id: None, filename: n, display_name: None, version_number: None,
                    enabled: true, tracked: false, client_side: None, server_side: None,
                    modrinth_project_id: None, update_available: None });
            }
        }
    }
    Ok(infos)
}

pub async fn add_mod(state: &Arc<AppState>, instance_id: &str, version_id: &str) -> Result<ModInfo, ModError> {
    let (data_dir, _, _) = instance_info(state, instance_id).await?;
    let modrinth = ModrinthClient::new(state.http.clone());
    let ver = modrinth.get_version(version_id).await?;
    let proj = modrinth.get_project(&ver.project_id).await?;
    if proj.server_side.as_deref() == Some("unsupported") { return Err(ModError::ClientOnly); }
    let file = ver.files.iter().find(|f| f.primary).or_else(|| ver.files.first()).ok_or(ModError::ModNotFound)?;
    let bytes = state.http.get(&file.url).send().await?.error_for_status()?.bytes().await?;
    let mods_dir = data_dir.join("mods");
    tokio::fs::create_dir_all(&mods_dir).await?;
    tokio::fs::write(mods_dir.join(&file.filename), &bytes).await?;
    let sha512 = file.hashes.sha512.clone().unwrap_or_else(|| hex::encode(sha2::Sha512::digest(&bytes)));
    let id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO mods (id,instance_id,filename,display_name,modrinth_project_id,modrinth_version_id,version_number,client_side,server_side,sha512,enabled,installed_at) VALUES (?,?,?,?,?,?,?,?,?,?,1,?)")
        .bind(&id).bind(instance_id).bind(&file.filename).bind(&proj.title)
        .bind(&ver.project_id).bind(version_id).bind(&ver.version_number)
        .bind(proj.client_side.as_deref()).bind(proj.server_side.as_deref())
        .bind(&sha512).bind(chrono::Utc::now().to_rfc3339()).execute(&state.pool).await?;
    Ok(ModInfo { id: Some(id), filename: file.filename.clone(), display_name: Some(proj.title),
        version_number: Some(ver.version_number), enabled: true, tracked: true,
        client_side: proj.client_side, server_side: proj.server_side,
        modrinth_project_id: Some(ver.project_id), update_available: Some(false) })
}

pub async fn upload_mod(state: &Arc<AppState>, instance_id: &str, filename: &str, data: bytes::Bytes) -> Result<(), ModError> {
    let (data_dir, _, _) = instance_info(state, instance_id).await?;
    let mods_dir = data_dir.join("mods");
    tokio::fs::create_dir_all(&mods_dir).await?;
    tokio::fs::write(mods_dir.join(filename), &data).await?;
    let sha512 = hex::encode(sha2::Sha512::digest(&data));
    sqlx::query("INSERT OR REPLACE INTO mods (id,instance_id,filename,sha512,enabled,installed_at) VALUES (?,?,?,?,1,?)")
        .bind(Uuid::new_v4().to_string()).bind(instance_id).bind(filename)
        .bind(sha512).bind(chrono::Utc::now().to_rfc3339()).execute(&state.pool).await?;
    Ok(())
}

pub async fn delete_mod(state: &Arc<AppState>, instance_id: &str, filename: &str) -> Result<bool, ModError> {
    let (data_dir, _, _) = instance_info(state, instance_id).await?;
    let mods_dir = data_dir.join("mods");
    let jar = mods_dir.join(filename);
    let dis = mods_dir.join(format!("{filename}.disabled"));
    if jar.exists() { tokio::fs::remove_file(&jar).await?; }
    else if dis.exists() { tokio::fs::remove_file(&dis).await?; }
    else {
        let c: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mods WHERE instance_id=? AND filename=?")
            .bind(instance_id).bind(filename).fetch_one(&state.pool).await?;
        if c == 0 { return Err(ModError::ModNotFound); }
    }
    sqlx::query("DELETE FROM mods WHERE instance_id=? AND filename=?")
        .bind(instance_id).bind(filename).execute(&state.pool).await?;
    let uid = instance_id.parse::<Uuid>().map_err(|_| ModError::InstanceNotFound)?;
    Ok(state.instances.contains_key(&crate::domain::instance::InstanceId(uid)))
}

pub async fn toggle_mod(state: &Arc<AppState>, instance_id: &str, filename: &str, enabled: bool) -> Result<(), ModError> {
    let (data_dir, _, _) = instance_info(state, instance_id).await?;
    let mods_dir = data_dir.join("mods");
    let jar = mods_dir.join(filename);
    let dis = mods_dir.join(format!("{filename}.disabled"));
    if enabled {
        if dis.exists() { tokio::fs::rename(&dis, &jar).await?; }
        else if !jar.exists() { return Err(ModError::ModNotFound); }
    } else {
        if jar.exists() { tokio::fs::rename(&jar, &dis).await?; }
        else if !dis.exists() { return Err(ModError::ModNotFound); }
    }
    sqlx::query("UPDATE mods SET enabled=? WHERE instance_id=? AND filename=?")
        .bind(enabled as i64).bind(instance_id).bind(filename).execute(&state.pool).await?;
    Ok(())
}

/// M6: update a single mod; returns true if updated, false if already latest.
pub async fn update_mod(state: &Arc<AppState>, instance_id: &str, filename: &str) -> Result<bool, ModError> {
    let (data_dir, gv, loader) = instance_info(state, instance_id).await?;
    let (pid, vid): (String, String) = sqlx::query_as(
        "SELECT modrinth_project_id, modrinth_version_id FROM mods WHERE instance_id=? AND filename=?"
    ).bind(instance_id).bind(filename).fetch_optional(&state.pool).await?.ok_or(ModError::ModNotFound)?;
    if pid.is_empty() { return Err(ModError::NoModrinthId); }
    let modrinth = ModrinthClient::new(state.http.clone());
    let versions = modrinth.list_versions(&pid, Some(&gv), Some(&loader)).await?;
    let latest = versions.first().ok_or(ModError::ModNotFound)?;
    if latest.id == vid { return Ok(false); }
    let file = latest.files.iter().find(|f| f.primary).or_else(|| latest.files.first()).ok_or(ModError::ModNotFound)?;
    let bytes = state.http.get(&file.url).send().await?.error_for_status()?.bytes().await?;
    let mods_dir = data_dir.join("mods");
    let tmp = mods_dir.join(format!("{}.tmp", file.filename));
    tokio::fs::write(&tmp, &bytes).await?;
    let _ = tokio::fs::remove_file(mods_dir.join(filename)).await;
    let _ = tokio::fs::remove_file(mods_dir.join(format!("{filename}.disabled"))).await;
    tokio::fs::rename(&tmp, mods_dir.join(&file.filename)).await?;
    let sha512 = file.hashes.sha512.clone().unwrap_or_else(|| hex::encode(sha2::Sha512::digest(&bytes)));
    sqlx::query("UPDATE mods SET filename=?,modrinth_version_id=?,version_number=?,sha512=? WHERE instance_id=? AND filename=?")
        .bind(&file.filename).bind(&latest.id).bind(&latest.version_number)
        .bind(&sha512).bind(instance_id).bind(filename).execute(&state.pool).await?;
    Ok(true)
}

pub async fn update_all_mods(state: &Arc<AppState>, instance_id: &str) -> Result<UpdateAllResult, ModError> {
    let filenames: Vec<String> = sqlx::query_scalar(
        "SELECT filename FROM mods WHERE instance_id=? AND modrinth_version_id IS NOT NULL AND modrinth_version_id != ''"
    ).bind(instance_id).fetch_all(&state.pool).await?;
    let mut res = UpdateAllResult { updated: vec![], already_latest: vec![], failed: vec![] };
    for fname in filenames {
        match update_mod(state, instance_id, &fname).await {
            Ok(true) => res.updated.push(fname),
            Ok(false) => res.already_latest.push(fname),
            Err(e) => res.failed.push(serde_json::json!({"filename": fname, "error": e.to_string()})),
        }
    }
    Ok(res)
}
