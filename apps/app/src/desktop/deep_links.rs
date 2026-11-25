use std::path::PathBuf;

use anyhow::{Context, bail};
use engine::{ErrorKind, EventSink, emit_warning, util::io};
use serde::{Deserialize, Serialize};
use url::form_urlencoded;
use urlencoding::decode;

/// A Modrinth link or file the App was asked to open, which the frontend acts on.
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(tag = "event")]
#[cfg_attr(feature = "export-ts", ts(tag = "event"))]
pub enum CommandPayload {
    InstallMod {
        id: String,
    },
    InstallVersion {
        id: String,
    },
    InstallModpack {
        id: String,
    },
    InstallServer {
        id: String,
    },
    LaunchInstance {
        id: String,
        server: Option<String>,
        singleplayer_world: Option<String>,
    },
    InstallSharedInstanceInvite {
        invite_id: String,
    },
    RunMRPack {
        // run or install .mrpack
        path: String,
    },
    PlayHostingServer {
        server_id: String,
        world_id: String,
    },
}

/// Handles external functions (such as through URL deep linkage)
/// Link is extracted value (link) in somewhat URL format, such as
/// subdomain1/subdomain2
/// (Does not include modrinth://)
pub async fn handle_url(
    events: &EventSink,
    sublink: &str,
) -> engine::Result<CommandPayload> {
    Ok(match sublink.split_once('/') {
        // /mod/{id}   -    Installs a mod of mod id
        Some(("mod", id)) => CommandPayload::InstallMod { id: id.to_string() },
        // /version/{id}   -    Installs a specific version of id
        Some(("version", id)) => {
            CommandPayload::InstallVersion { id: id.to_string() }
        }
        // /modpack/{id}   -    Installs a modpack of modpack id
        Some(("modpack", id)) => {
            CommandPayload::InstallModpack { id: id.to_string() }
        }
        // /server/{id}   -    Opens a server project page and triggers play flow
        Some(("server", id)) => {
            CommandPayload::InstallServer { id: id.to_string() }
        }
        Some(("hosting", path)) => {
            let (server_id, world_id) = path.split_once('/').ok_or(
                ErrorKind::InputError("missing hosting world ID".to_string()),
            )?;
            let server_id = uuid::Uuid::parse_str(server_id).context(
                ErrorKind::InputError("invalid hosting server ID".to_string()),
            )?;
            let world_id = uuid::Uuid::parse_str(world_id).context(
                ErrorKind::InputError("invalid hosting world ID".to_string()),
            )?;
            CommandPayload::PlayHostingServer {
                server_id: server_id.to_string(),
                world_id: world_id.to_string(),
            }
        }
        // /share/{invite_id}
        Some(("share", raw)) => {
            let (raw, _) = raw.split_once('?').unwrap_or((raw, ""));

            match decode(raw) {
                Ok(decoded) => CommandPayload::InstallSharedInstanceInvite {
                    invite_id: decoded.to_string(),
                },
                Err(e) => {
                    emit_warning(
                        events,
                        &format!(
                            "Invalid UTF-8 in shared instance invite path: {e}"
                        ),
                    )
                    .await?;
                    return Err(e).context(ErrorKind::InputError(
                        "invalid UTF-8 in shared instance invite path"
                            .to_string(),
                    ));
                }
            }
        }
        // /launch/instance/{id}   -    Launches an instance
        Some(("launch", rest)) if rest.starts_with("instance/") => {
            let raw = rest.trim_start_matches("instance/");
            let (raw, query) = raw.split_once('?').unwrap_or((raw, ""));
            let mut server = None;
            let mut singleplayer_world = None;

            for (key, value) in form_urlencoded::parse(query.as_bytes()) {
                match &*key {
                    "server" => server = Some(value.into_owned()),
                    "singleplayer_world" => {
                        singleplayer_world = Some(value.into_owned());
                    }
                    _ => {}
                }
            }

            if server.is_some() && singleplayer_world.is_some() {
                emit_warning(
                    events,
                    "Invalid command, cannot launch both a server and a singleplayer world",
                )
                .await?;
                bail!(ErrorKind::InputError(
                    "cannot launch both a server and a singleplayer world"
                        .to_string(),
                ));
            }

            match decode(raw) {
                Ok(decoded) => CommandPayload::LaunchInstance {
                    id: decoded.to_string(),
                    server,
                    singleplayer_world,
                },
                Err(e) => {
                    emit_warning(
                        events,
                        &format!("Invalid UTF-8 in instance path: {e}"),
                    )
                    .await?;
                    return Err(e).context(ErrorKind::InputError(
                        "invalid UTF-8 in instance path".to_string(),
                    ));
                }
            }
        }
        _ => {
            emit_warning(
                events,
                &format!("Invalid command, unrecognized path: {sublink}"),
            )
            .await?;
            bail!(ErrorKind::InputError(format!(
                "invalid command, unrecognized path {sublink}"
            )));
        }
    })
}

pub async fn parse_command(
    events: &EventSink,
    command_string: &str,
) -> engine::Result<CommandPayload> {
    tracing::debug!("Parsing external command");

    // modrinth://some-command
    // This occurs when following a web redirect link
    if let Some(sublink) = command_string.strip_prefix("modrinth://") {
        Ok(handle_url(events, sublink).await?)
    } else {
        // We assume anything else is a filepath to an .mrpack file
        let path = PathBuf::from(command_string);
        let path = io::canonicalize(path)?;
        if let Some(ext) = path.extension()
            && ext == "mrpack"
        {
            return Ok(CommandPayload::RunMRPack {
                path: path.to_string_lossy().into_owned(),
            });
        }
        emit_warning(
            events,
            &format!(
                "Invalid command, unrecognized filetype: {}",
                path.display()
            ),
        )
        .await?;
        bail!(ErrorKind::InputError(format!(
            "invalid command, unrecognized filetype {}",
            path.display()
        )))
    }
}
