//! Per-server roles: who besides the owner may see or manage each server.
//!
//! - `GET /access/me`: the caller's own access. [`MyAccess`].
//! - `GET /servers/{id}/access`: the owner and members of a server. [`ServerAccess`].
//! - `POST /servers/{id}/access`: invite someone, which grants the role at once. [`Invite`] in,
//!   [`Member`] out. Owner only, dangerous.
//! - `PATCH /servers/{id}/access/{user_id}`: change a role. [`ChangeRole`] in, [`Member`] out.
//!   Owner only, dangerous.
//! - `DELETE /servers/{id}/access/{user_id}`: remove someone; their sessions end. Owner only,
//!   dangerous.
//!
//! A dangerous action carries a fresh ticket in the `X-Amberite-Ticket` header, so every route
//! asks for it the same way whatever its method and body.

#![allow(dead_code)] // Wired in step 3: access.

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

/// A role on one server. The owner has every role on every server and is never a member.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Sees status, players, content, and backups. No console: it carries chat and player IPs.
    Viewer,
    /// Everything a viewer can, plus running, console, files, settings, and backups. Runs code on
    /// the host machine.
    Editor,
}

/// A user as their last ticket named them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct User {
    pub id: String,
    pub name: String,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
pub struct Member {
    pub user: User,
    pub role: Role,
    /// Unix seconds.
    pub added_at: i64,
}

#[derive(Debug, Clone, Serialize, TS)]
pub struct ServerAccess {
    pub owner: User,
    pub members: Vec<Member>,
}

#[derive(Debug, Clone, Deserialize, TS)]
pub struct Invite {
    /// Shown until the invited user signs in and their own ticket names them.
    pub user: User,
    pub role: Role,
}

#[derive(Debug, Clone, Deserialize, TS)]
pub struct ChangeRole {
    pub role: Role,
}

#[derive(Debug, Clone, Serialize, TS)]
pub struct MyAccess {
    pub user: User,
    /// The owner of this Core, with every role on every server.
    pub owner: bool,
    /// Empty for the owner.
    pub servers: Vec<ServerRole>,
}

#[derive(Debug, Clone, Serialize, TS)]
pub struct ServerRole {
    pub server_id: Uuid,
    pub role: Role,
}
