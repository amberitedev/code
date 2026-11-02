//! What is newer than what a server has: for its addons, and for the Modrinth pack it follows.

use std::collections::HashMap;

use bedrock::content::least_stable;
use modrinth_content_management::{ContentType, VersionType};
use tracing::warn;

use super::apply_content_install::{loaders, target};
use crate::state::servers::{
    Content, Error, TrackedFile,
    adapters::{
        filesystem::content_files::Listed,
        network::modrinth::{self, Version},
        sqlite::server_rows,
    },
};

/// The newest version for each listed file Modrinth knows, by path, on the channel it updates
/// on: the least stable of the server's and the one its installed version (in `installed`, by
/// id) is on. Each channel falls back one step at a time, as in the App. Best effort: empty
/// when Modrinth cannot be reached.
pub async fn check_content_updates(
    content: &Content,
    server_id: &str,
    listed: &[Listed],
    rows: &[TrackedFile],
    installed: &HashMap<String, Version>,
) -> HashMap<String, Version> {
    let Ok(target) = target(content, server_id).await else {
        return HashMap::default();
    };
    let by_path: HashMap<&str, &TrackedFile> = rows
        .iter()
        .map(|row| (row.relative_path.as_str(), row))
        .collect();
    // The files to check, by kind and channel, since each asks Modrinth on its own.
    let mut groups: HashMap<
        (ContentType, VersionType),
        Vec<(&Listed, &TrackedFile)>,
    > = HashMap::new();
    for item in listed {
        let Some(row) = by_path
            .get(item.path.as_str())
            .filter(|row| row.project_id.is_some())
        else {
            continue;
        };
        let channel = row
            .version_id
            .as_ref()
            .and_then(|id| installed.get(id))
            .map_or(target.channel, |version| {
                least_stable(target.channel, version.version_type)
            });
        groups
            .entry((item.kind, channel))
            .or_default()
            .push((item, row));
    }
    let mut found = HashMap::new();
    for ((kind, channel), files) in groups {
        let sha1s: Vec<String> =
            files.iter().map(|(_, row)| row.sha1.clone()).collect();
        match content
            .modrinth
            .updates_on_channel(
                &sha1s,
                &loaders(&target, kind),
                &target.game_version,
                channel,
            )
            .await
        {
            Ok(mut newer) => {
                for (item, row) in files {
                    if let Some(version) = newer.remove(&row.sha1)
                        && row.version_id.as_deref()
                            != Some(version.id.as_str())
                    {
                        found.insert(item.path.clone(), version);
                    }
                }
            }
            Err(error) => {
                warn!(server = %server_id, %error, "could not check for updates")
            }
        }
    }
    found
}

/// The version of the Modrinth pack `project_id` to offer over `installed` on server `id`, as the
/// App picks it (`bedrock::content::pack_update`) on the server's update channel. Its Minecraft
/// may differ from the server's; [`pack`] guards that.
pub async fn check_modpack_update(
    content: &Content,
    id: &str,
    project_id: &str,
    installed: &modrinth::Version,
) -> Result<Option<String>, Error> {
    let server = server_rows::get(&content.db, id)
        .await?
        .ok_or_else(|| Error::NotFound(format!("There is no server {id}.")))?;
    let versions = content.modrinth.project_versions(project_id).await?;
    Ok(bedrock::content::pack_update(
        installed,
        &versions,
        server.update_channel,
    )
    .map(|version| version.id.clone()))
}
