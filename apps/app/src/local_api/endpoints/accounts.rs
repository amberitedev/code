//! The Minecraft and Modrinth accounts signed in on this computer.

use crate::accounts::{
    minecraft_auth::MinecraftAccountSummary, mr_auth::ModrinthAuthFlow,
    state::ModrinthCredentials,
};
use contracts::{Empty, Json};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::IdParams;

contracts::endpoints! {
    group localAccounts;

    /// Fails when Mojang's session server cannot be reached.
    CheckMinecraftReachable: Get "/app/accounts/minecraft/reachable", None, (), (), Empty => Empty;
    /// Opens Microsoft's sign-in window on the desktop; closing it answers `login_canceled`.
    SignInToMinecraft: Post "/app/accounts/minecraft/login", None, (), (), Empty => Json<MinecraftAccountSummary>;
    ListMinecraftAccounts: Get "/app/accounts/minecraft", None, (), (), Empty => Json<Vec<MinecraftAccountSummary>>;
    GetDefaultMinecraftAccount: Get "/app/accounts/minecraft/default", None, (), (), Empty => Json<Option<Uuid>>;
    SetDefaultMinecraftAccount: Put "/app/accounts/minecraft/default", None, (), (), Json<Uuid> => Empty;
    RemoveMinecraftAccount: Delete "/app/accounts/minecraft/{id}", None, MinecraftAccountParams, (), Empty => Empty;
    /// Opens Modrinth's sign-in page in the browser and waits for it to hand back a session;
    /// cancelling answers `login_canceled`.
    SignInToModrinth: Post "/app/accounts/modrinth/login", None, (), (), Json<ModrinthSignIn> => Json<ModrinthCredentials>;
    CancelModrinthSignIn: Delete "/app/accounts/modrinth/login", None, (), (), Empty => Empty;
    /// Signs every Modrinth account out, keeping them on the list.
    SignOutOfModrinth: Post "/app/accounts/modrinth/logout", None, (), (), Empty => Empty;
    GetActiveModrinthAccount: Get "/app/accounts/modrinth/active", None, (), (), Empty => Json<Option<ModrinthCredentials>>;
    ListModrinthAccounts: Get "/app/accounts/modrinth", None, (), (), Empty => Json<Vec<ModrinthCredentials>>;
    /// Takes the user id.
    SetActiveModrinthAccount: Put "/app/accounts/modrinth/active", None, (), (), Json<String> => Empty;
    RemoveModrinthAccount: Delete "/app/accounts/modrinth/{id}", None, IdParams, (), Empty => Empty;
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct MinecraftAccountParams {
    pub id: Uuid,
}

#[derive(Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct ModrinthSignIn {
    pub flow: ModrinthAuthFlow,
    pub add_account: bool,
}
