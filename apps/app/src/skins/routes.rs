//! Serves `crate::local_api::endpoints::skins` over `crate::skins::minecraft_skins`.

use crate::state::Desktop;

use axum::{Router, extract::State};
use contracts::{
    ApiError, Empty, Json, Raw,
    server::{Mount, Req},
};

use crate::accounts::api_error;
use crate::local_api::{
    AppState,
    endpoints::skins::{
        AddAndEquipCustomSkin, EquipSkin, FlushPendingSkinChange, ListCapes,
        ListSkins, NormalizeSkinTexture, RemoveCustomSkin, SaveCustomSkin,
        SetCustomSkinOrder, UnequipSkin,
    },
};
use crate::skins::minecraft_skins::{self, Cape, Skin};

type Answer<T> = Result<T, ApiError>;

pub fn routes() -> Router<AppState> {
    Router::new()
        .endpoint::<ListCapes, _, _>(list_capes)
        .endpoint::<ListSkins, _, _>(list_skins)
        .endpoint::<SaveCustomSkin, _, _>(save_custom_skin)
        .endpoint::<RemoveCustomSkin, _, _>(remove_custom_skin)
        .endpoint::<SetCustomSkinOrder, _, _>(set_custom_skin_order)
        .endpoint::<AddAndEquipCustomSkin, _, _>(add_and_equip_custom_skin)
        .endpoint::<EquipSkin, _, _>(equip_skin)
        .endpoint::<UnequipSkin, _, _>(unequip_skin)
        .endpoint::<FlushPendingSkinChange, _, _>(flush_pending_skin_change)
        .endpoint::<NormalizeSkinTexture, _, _>(normalize_skin_texture)
}

async fn list_capes(
    State(engine): State<Desktop>,
    _: Req<ListCapes>,
) -> Answer<Json<Vec<Cape>>> {
    minecraft_skins::get_available_capes(&engine)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn list_skins(
    State(engine): State<Desktop>,
    _: Req<ListSkins>,
) -> Answer<Json<Vec<Skin>>> {
    minecraft_skins::get_available_skins(&engine)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn save_custom_skin(
    State(engine): State<Desktop>,
    Req { body, .. }: Req<SaveCustomSkin>,
) -> Answer<Json<Skin>> {
    minecraft_skins::save_custom_skin(
        &engine,
        body.skin,
        body.texture_blob,
        body.variant,
        body.cape,
        body.replace_texture,
    )
    .await
    .map(Json)
    .map_err(api_error)
}

async fn remove_custom_skin(
    State(engine): State<Desktop>,
    Req { body, .. }: Req<RemoveCustomSkin>,
) -> Answer<Empty> {
    minecraft_skins::remove_custom_skin(&engine, body)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

async fn set_custom_skin_order(
    State(engine): State<Desktop>,
    Req { body, .. }: Req<SetCustomSkinOrder>,
) -> Answer<Empty> {
    minecraft_skins::set_custom_skin_order(&engine, body)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

async fn add_and_equip_custom_skin(
    State(engine): State<Desktop>,
    Req { body, .. }: Req<AddAndEquipCustomSkin>,
) -> Answer<Json<Skin>> {
    minecraft_skins::add_and_equip_custom_skin(
        &engine,
        body.texture_blob,
        body.variant,
        body.cape,
    )
    .await
    .map(Json)
    .map_err(api_error)
}

async fn equip_skin(
    State(engine): State<Desktop>,
    Req { body, .. }: Req<EquipSkin>,
) -> Answer<Empty> {
    minecraft_skins::equip_skin(&engine, body)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

async fn unequip_skin(
    State(engine): State<Desktop>,
    _: Req<UnequipSkin>,
) -> Answer<Empty> {
    minecraft_skins::unequip_skin(&engine)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

async fn flush_pending_skin_change(
    State(engine): State<Desktop>,
    Req { query, .. }: Req<FlushPendingSkinChange>,
) -> Answer<Empty> {
    match query.profile_id {
        Some(profile_id) => {
            minecraft_skins::flush_pending_skin_change_for_profile(
                &engine, profile_id,
            )
            .await
        }
        None => minecraft_skins::flush_pending_skin_change(&engine).await,
    }
    .map(|()| Empty)
    .map_err(api_error)
}

async fn normalize_skin_texture(
    Req { body, .. }: Req<NormalizeSkinTexture>,
) -> Answer<Raw> {
    minecraft_skins::normalize_skin_texture(&body)
        .await
        .map(|bytes| Raw {
            content_type: "image/png".to_owned(),
            bytes,
        })
        .map_err(api_error)
}
