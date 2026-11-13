//! The Minecraft and Modrinth accounts signed in on this computer.

use contracts::{Empty, Json};
use engine::{
    minecraft_auth::MinecraftAccountSummary, mr_auth::ModrinthAuthFlow,
    prelude::ModrinthCredentials,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::IdParams;

endpoints! {
    /// Fails when Mojang's session server cannot be reached.
    CheckMinecraftReachable: Get "/app/accounts/minecraft/reachable", (), (),
        Empty => Empty;
    /// Opens Microsoft's sign-in window on the desktop; closing it answers `sign_in_cancelled`.
    SignInToMinecraft: Post "/app/accounts/minecraft/login", (), (), Empty
        => Json<MinecraftAccountSummary>;
    ListMinecraftAccounts: Get "/app/accounts/minecraft", (), (), Empty
        => Json<Vec<MinecraftAccountSummary>>;
    GetDefaultMinecraftAccount: Get "/app/accounts/minecraft/default", (), (),
        Empty => Json<Option<Uuid>>;
    SetDefaultMinecraftAccount: Put "/app/accounts/minecraft/default", (), (),
        Json<Uuid> => Empty;
    RemoveMinecraftAccount: Delete "/app/accounts/minecraft/{id}",
        MinecraftAccountParams, (), Empty => Empty;
    /// Opens Modrinth's sign-in page in the browser and waits for it to hand back a session;
    /// cancelling answers `sign_in_cancelled`.
    SignInToModrinth: Post "/app/accounts/modrinth/login", (), (),
        Json<ModrinthSignIn> => Json<ModrinthCredentials>;
    CancelModrinthSignIn: Delete "/app/accounts/modrinth/login", (), (), Empty
        => Empty;
    /// Signs every Modrinth account out, keeping them on the list.
    SignOutOfModrinth: Post "/app/accounts/modrinth/logout", (), (), Empty
        => Empty;
    GetActiveModrinthAccount: Get "/app/accounts/modrinth/active", (), (),
        Empty => Json<Option<ModrinthCredentials>>;
    ListModrinthAccounts: Get "/app/accounts/modrinth", (), (), Empty
        => Json<Vec<ModrinthCredentials>>;
    /// Takes the user id.
    SetActiveModrinthAccount: Put "/app/accounts/modrinth/active", (), (),
        Json<String> => Empty;
    RemoveModrinthAccount: Delete "/app/accounts/modrinth/{id}", IdParams, (),
        Empty => Empty;
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct MinecraftAccountParams {
    pub id: Uuid,
}

#[derive(Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ModrinthSignIn {
    pub flow: ModrinthAuthFlow,
    pub add_account: bool,
}
