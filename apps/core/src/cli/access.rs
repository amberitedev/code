//! `copal access ...`: who may use each server, and the sessions they signed in with. Inviting
//! someone needs their Amberite account, so invites come from the App. A user is named by their
//! id or their name.

use axum::http::Method;
use clap::Subcommand;
use color_eyre::eyre::{Result, bail};

use super::{Client, Output, local_time, servers};
use crate::api::{
    access::{ChangeRole, Member, Role, ServerAccess},
    sessions::Session,
};

#[derive(Debug, Subcommand)]
pub enum AccessCommand {
    /// Who may use a server, and with which role.
    List { server: String },
    /// Change someone's role on a server.
    Role {
        server: String,
        user: String,
        role: Role,
    },
    /// Take someone off a server. Their sessions on this Core end at once.
    Remove { server: String, user: String },
    /// Every signed-in device, or one user's.
    Sessions {
        #[arg(long)]
        user: Option<String>,
    },
    /// End a session. The device has to sign in again.
    Revoke { session: String },
}

pub async fn run(client: &Client, command: AccessCommand) -> Result<Output> {
    match command {
        AccessCommand::List { server } => {
            let server = servers::find(client, &server).await?;
            let access: ServerAccess = client
                .get(&format!("/servers/{}/access", server.server_id))
                .await?;
            let mut lines = vec![match &access.owner {
                Some(owner) => format!("{}  owner  {}", owner.id, owner.name),
                None => "This Core is not paired with an owner yet".into(),
            }];
            lines.extend(access.members.iter().map(|member| {
                format!(
                    "{}  {:<6} {}",
                    member.user.id,
                    role_name(member.role),
                    member.user.name
                )
            }));
            Ok(Output::new(lines.join("\n"), &access))
        }
        AccessCommand::Role { server, user, role } => {
            let server = servers::find(client, &server).await?;
            let member = member(client, &server.server_id, &user).await?;
            let member: Member = client
                .call(
                    Method::PATCH,
                    &format!(
                        "/servers/{}/access/{}",
                        server.server_id, member.user.id
                    ),
                    Some(&ChangeRole { role }),
                )
                .await?;
            Ok(Output::new(
                format!(
                    "{} is now {} on {}",
                    member.user.name,
                    role_name(member.role),
                    server.name
                ),
                &member,
            ))
        }
        AccessCommand::Remove { server, user } => {
            let server = servers::find(client, &server).await?;
            let member = member(client, &server.server_id, &user).await?;
            client
                .call::<()>(
                    Method::DELETE,
                    &format!(
                        "/servers/{}/access/{}",
                        server.server_id, member.user.id
                    ),
                    None::<&()>,
                )
                .await?;
            Ok(Output::new(
                format!(
                    "{} was removed from {}",
                    member.user.name, server.name
                ),
                &member,
            ))
        }
        AccessCommand::Sessions { user } => {
            let mut sessions: Vec<Session> = client.get("/sessions").await?;
            if let Some(user) = user {
                let name = user.to_lowercase();
                sessions.retain(|session| {
                    session.user.id == user
                        || session.user.name.to_lowercase() == name
                });
            }
            let text = if sessions.is_empty() {
                "No sessions".to_owned()
            } else {
                sessions
                    .iter()
                    .map(|session| {
                        format!(
                            "{}  {:<16} {:<24} last used {}",
                            session.id,
                            session.user.name,
                            session.device_name,
                            local_time(session.last_used_at)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            Ok(Output::new(text, &sessions))
        }
        AccessCommand::Revoke { session } => {
            let sessions: Vec<Session> = client.get("/sessions").await?;
            let matches: Vec<&Session> = sessions
                .iter()
                .filter(|candidate| candidate.id.starts_with(&session))
                .collect();
            let [session] = matches.as_slice() else {
                bail!(
                    "{session} names {} sessions. See copal access sessions",
                    if matches.is_empty() { "no" } else { "several" }
                );
            };
            client
                .call::<()>(
                    Method::DELETE,
                    &format!("/sessions/{}", session.id),
                    None::<&()>,
                )
                .await?;
            Ok(Output::new(
                format!(
                    "Ended the session of {} on {}",
                    session.user.name, session.device_name
                ),
                session,
            ))
        }
    }
}

/// The member of a server `query` names: their id or their name.
async fn member(
    client: &Client,
    server_id: &uuid::Uuid,
    query: &str,
) -> Result<Member> {
    let access: ServerAccess =
        client.get(&format!("/servers/{server_id}/access")).await?;
    let lower = query.to_lowercase();
    let mut matches: Vec<Member> = access
        .members
        .into_iter()
        .filter(|member| {
            member.user.id == query || member.user.name.to_lowercase() == lower
        })
        .collect();
    match matches.len() {
        1 => Ok(matches.remove(0)),
        0 => bail!("{query} has no role on this server. See copal access list"),
        _ => bail!("several people are called {query}; use their id"),
    }
}

fn role_name(role: Role) -> &'static str {
    match role {
        Role::Viewer => "viewer",
        Role::Editor => "editor",
    }
}
