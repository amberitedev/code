//! Serves `crate::server::endpoints::skins` over `crate::skins::minecraft_skins`.

use std::sync::Arc;

use axum::{Router, extract::State};
use contracts::{
    ApiError, Empty, Json, Raw,
    server::{Req, route},
};
use engine::Engine;

use crate::accounts::api_error;
use crate::server::{
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
    [
        route::<ListCapes, _, _, _>(list_capes),
        route::<ListSkins, _, _, _>(list_skins),
        route::<SaveCustomSkin, _, _, _>(save_custom_skin),
        route::<RemoveCustomSkin, _, _, _>(remove_custom_skin),
        route::<SetCustomSkinOrder, _, _, _>(set_custom_skin_order),
        route::<AddAndEquipCustomSkin, _, _, _>(add_and_equip_custom_skin),
        route::<EquipSkin, _, _, _>(equip_skin),
        route::<UnequipSkin, _, _, _>(unequip_skin),
        route::<FlushPendingSkinChange, _, _, _>(flush_pending_skin_change),
        route::<NormalizeSkinTexture, _, _, _>(normalize_skin_texture),
    ]
    .into_iter()
    .fold(Router::new(), |router, (path, method)| {
        router.route(path, method)
    })
}

async fn list_capes(
    State(engine): State<Arc<Engine>>,
    _: Req<ListCapes>,
) -> Answer<Json<Vec<Cape>>> {
    minecraft_skins::get_available_capes(&engine)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn list_skins(
    State(engine): State<Arc<Engine>>,
    _: Req<ListSkins>,
) -> Answer<Json<Vec<Skin>>> {
    minecraft_skins::get_available_skins(&engine)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn save_custom_skin(
    State(engine): State<Arc<Engine>>,
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
    State(engine): State<Arc<Engine>>,
    Req { body, .. }: Req<RemoveCustomSkin>,
) -> Answer<Empty> {
    minecraft_skins::remove_custom_skin(&engine, body)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

async fn set_custom_skin_order(
    State(engine): State<Arc<Engine>>,
    Req { body, .. }: Req<SetCustomSkinOrder>,
) -> Answer<Empty> {
    minecraft_skins::set_custom_skin_order(&engine, body)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

async fn add_and_equip_custom_skin(
    State(engine): State<Arc<Engine>>,
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
    State(engine): State<Arc<Engine>>,
    Req { body, .. }: Req<EquipSkin>,
) -> Answer<Empty> {
    minecraft_skins::equip_skin(&engine, body)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

async fn unequip_skin(
    State(engine): State<Arc<Engine>>,
    _: Req<UnequipSkin>,
) -> Answer<Empty> {
    minecraft_skins::unequip_skin(&engine)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

async fn flush_pending_skin_change(
    State(engine): State<Arc<Engine>>,
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
