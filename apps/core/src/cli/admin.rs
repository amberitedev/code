use std::sync::Arc;

use color_eyre::eyre::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::{
    application::{
        access_service,
        instance_service::repair_instance,
        instance_status_service::{
            kill_instance, restart_instance, send_command, start_instance,
            stop_instance,
        },
        invite_service,
        state::AppState,
    },
    domain::instance::{InstanceId, InstanceRecord},
};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum Request {
    Status,
    ListInstances,
    StartInstance { id: String },
    StopInstance { id: String },
    RestartInstance { id: String },
    KillInstance { id: String },
    RepairInstance { id: String },
    SendConsoleCommand { id: String, command: String },
    AttachConsole { id: String },
    ListMembers,
    RemoveMember { user_id: String },
    ListRoles,
    ListInvitations,
    ReviewInvitation { id: String, accept: bool },
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub(crate) struct MemberInfo {
    pub(crate) user_id: String,
    pub(crate) display_name: Option<String>,
    pub(crate) role: String,
    pub(crate) status: String,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub(crate) struct RoleInfo {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) retired_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub(crate) struct InvitationInfo {
    pub(crate) id: String,
    pub(crate) invitee_user_id: String,
    pub(crate) invitee_display_name: Option<String>,
    pub(crate) status: String,
    pub(crate) expires_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum Response {
    Status {
        core_id: String,
        running_instances: usize,
    },
    Instances {
        instances: Vec<InstanceRecord>,
    },
    Members {
        members: Vec<MemberInfo>,
    },
    Roles {
        roles: Vec<RoleInfo>,
    },
    Invitations {
        invitations: Vec<InvitationInfo>,
    },
    Ok,
    Error {
        message: String,
    },
}

#[allow(dead_code)]
pub(crate) async fn dispatch(
    state: &Arc<AppState>,
    request: Request,
) -> Result<Response> {
    match request {
        Request::Status => Ok(Response::Status {
            core_id: state.core_id.clone(),
            running_instances: state.instances.len(),
        }),
        Request::ListInstances => state
            .instance_store
            .list()
            .await
            .map(|instances| Response::Instances { instances })
            .map_err(Into::into),
        Request::StartInstance { id } => {
            let id = id.parse::<InstanceId>()?;
            start_instance(state, &id).await?;
            Ok(Response::Ok)
        }
        Request::StopInstance { id } => {
            let id = id.parse::<InstanceId>()?;
            stop_instance(state, &id).await?;
            Ok(Response::Ok)
        }
        Request::RestartInstance { id } => {
            let id = id.parse::<InstanceId>()?;
            restart_instance(state, &id).await?;
            Ok(Response::Ok)
        }
		Request::KillInstance { id } => {
			let id = id.parse::<InstanceId>()?;
			kill_instance(state, &id).await?;
			Ok(Response::Ok)
		}
		Request::RepairInstance { id } => {
			let id = id.parse::<InstanceId>()?;
			repair_instance(state, &id).await?;
			Ok(Response::Ok)
		}
        Request::SendConsoleCommand { id, command } => {
            let id = id.parse::<InstanceId>()?;
            send_command(state, &id, command).await?;
            Ok(Response::Ok)
        }
        Request::AttachConsole { .. } => bail!("Console attachments are streaming requests."),
		Request::ListMembers => Ok(Response::Members { members: sqlx::query_as("SELECT user_id, display_name, role, status FROM core_members ORDER BY role, display_name, user_id").fetch_all(&state.pool).await? }),
		Request::RemoveMember { user_id } => {
			let owner = state.owner_user_id().await.ok_or_else(|| color_eyre::eyre::eyre!("Core has no configured owner."))?;
			access_service::remove_core_access(state, &owner, &user_id).await?;
			Ok(Response::Ok)
		}
		Request::ListRoles => Ok(Response::Roles { roles: sqlx::query_as("SELECT id, name, description, retired_at FROM core_roles ORDER BY created_at").fetch_all(&state.pool).await? }),
		Request::ListInvitations => Ok(Response::Invitations { invitations: sqlx::query_as("SELECT id, invitee_user_id, invitee_display_name, status, expires_at FROM core_invitations ORDER BY created_at DESC").fetch_all(&state.pool).await? }),
		Request::ReviewInvitation { id, accept } => {
			let owner = state.owner_user_id().await.ok_or_else(|| color_eyre::eyre::eyre!("Core has no configured owner."))?;
			invite_service::review(state, &owner, &id, accept).await?;
			Ok(Response::Ok)
		}
    }
}

#[cfg(unix)]
pub(crate) fn socket_path(state: &AppState) -> std::path::PathBuf {
    state.config.data_dir.join("copal-admin.sock")
}

#[cfg(unix)]
pub(crate) async fn serve(state: Arc<AppState>) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    use tokio::{
        io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
        net::UnixListener,
    };
    let path = socket_path(&state);
    if path.exists() {
        tokio::fs::remove_file(&path).await?;
    }
    let listener = UnixListener::bind(&path)?;
    let mut permissions = tokio::fs::metadata(&path).await?.permissions();
    permissions.set_mode(0o660);
    tokio::fs::set_permissions(&path, permissions).await?;
    loop {
        let (stream, _) = listener.accept().await?;
        let state = Arc::clone(&state);
        tokio::spawn(async move {
            let (read, mut write) = stream.into_split();
            let mut line = String::new();
            let mut reader = BufReader::new(read);
            let request = match reader.read_line(&mut line).await {
                Ok(0) => return,
                Ok(_) => match serde_json::from_str(&line) {
                    Ok(request) => request,
                    Err(error) => {
                        let response = Response::Error {
                            message: format!(
                                "Invalid local admin request: {error}"
                            ),
                        };
                        let json = serde_json::to_string(&response)
                            .unwrap_or_default();
                        let _ = write
                            .write_all(format!("{json}\n").as_bytes())
                            .await;
                        return;
                    }
                },
                Err(_) => return,
            };
            if let Request::AttachConsole { id } = request {
                let _ =
                    stream_console(&mut reader, &mut write, state, id).await;
                return;
            }
            let response =
                dispatch(&state, request).await.unwrap_or_else(|error| {
                    Response::Error {
                        message: error.to_string(),
                    }
                });
            if let Ok(json) = serde_json::to_string(&response) {
                let _ = write.write_all(format!("{json}\n").as_bytes()).await;
            }
        });
    }
}

#[cfg(unix)]
async fn stream_console<R, W>(
    reader: &mut R,
    write: &mut W,
    state: Arc<AppState>,
    id: String,
) -> Result<()>
where
    R: tokio::io::AsyncBufRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

    let id = id.parse::<InstanceId>()?;
    let mut receiver = state.broadcaster.subscribe();
    let mut input = String::new();
    write.write_all(b"{\"type\":\"connected\"}\n").await?;
    loop {
        tokio::select! {
            event = receiver.recv() => match event {
                Ok(crate::domain::event::Event::InstanceOutput { instance_id, line }) if instance_id == id => {
                    write.write_all(format!("{}\n", serde_json::json!({"type":"output","data":line})).as_bytes()).await?;
                }
                Ok(crate::domain::event::Event::StatusChanged { instance_id, status }) if instance_id == id => {
                    write.write_all(format!("{}\n", serde_json::json!({"type":"status","data":status})).as_bytes()).await?;
                }
                Err(_) => break,
                _ => {}
            },
            read = reader.read_line(&mut input) => {
                if read? == 0 { break; }
                if let Ok(Request::SendConsoleCommand { id: command_id, command }) = serde_json::from_str(&input) {
                    if command_id == id.to_string() {
                        let _ = send_command(&state, &id, command).await;
                    }
                }
                input.clear();
            }
        }
    }
    Ok(())
}

#[cfg(unix)]
pub(crate) async fn attach_console(
    data_dir: &std::path::Path,
    id: String,
) -> Result<()> {
    use tokio::{
        io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
        net::UnixStream,
    };
    let stream = UnixStream::connect(data_dir.join("copal-admin.sock")).await?;
    let (read, mut write) = stream.into_split();
    write
        .write_all(
            format!(
                "{}\n",
                serde_json::to_string(&Request::AttachConsole { id })?
            )
            .as_bytes(),
        )
        .await?;
    let mut reader = BufReader::new(read);
    let mut stdin = BufReader::new(tokio::io::stdin());
    let mut line = String::new();
    let mut command = String::new();
    loop {
        tokio::select! {
            read = reader.read_line(&mut line) => {
                if read? == 0 { break; }
                let frame: serde_json::Value = serde_json::from_str(&line)?;
                if let Some(output) = frame.get("data").and_then(|value| value.as_str()) { println!("{output}"); }
                line.clear();
            }
            read = stdin.read_line(&mut command) => {
                if read? == 0 { break; }
                let text = command.trim().to_owned();
                if !text.is_empty() {
                    let request = Request::SendConsoleCommand { id: id.clone(), command: text };
                    write.write_all(format!("{}\n", serde_json::to_string(&request)?).as_bytes()).await?;
                }
                command.clear();
            }
        }
    }
    Ok(())
}

#[cfg(not(unix))]
pub(crate) async fn attach_console(
    _data_dir: &std::path::Path,
    _id: String,
) -> Result<()> {
    bail!("Live console attachment is currently supported on Unix only.")
}

#[cfg(unix)]
pub(crate) async fn request(
    data_dir: &std::path::Path,
    request: Request,
) -> Result<Response> {
    use tokio::{
        io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
        net::UnixStream,
    };
    let path = data_dir.join("copal-admin.sock");
    let stream = UnixStream::connect(&path).await.map_err(|error| {
        color_eyre::eyre::eyre!(
            "Could not connect to local Copal admin socket at {}: {error}",
            path.display()
        )
    })?;
    let (read, mut write) = stream.into_split();
    write
        .write_all(format!("{}\n", serde_json::to_string(&request)?).as_bytes())
        .await?;
    let mut line = String::new();
    BufReader::new(read).read_line(&mut line).await?;
    let response: Response = serde_json::from_str(&line)?;
    if let Response::Error { message } = &response {
        bail!("{message}")
    }
    Ok(response)
}

#[cfg(not(unix))]
pub(crate) async fn request(
    _data_dir: &std::path::Path,
    _request: Request,
) -> Result<Response> {
    bail!("The local admin endpoint is currently supported on Unix only.")
}
