//! Theseus tag management interface
use crate::ErrorKind;
use crate::state::CachedEntry;
pub use crate::{
    State,
    state::{Category, DonationPlatform, GameVersion, Loader},
};

/// Get category tags
#[tracing::instrument(skip(state))]
pub async fn get_category_tags(state: &State) -> crate::Result<Vec<Category>> {
    let categories = CachedEntry::get_categories(
        None,
        &state.pool,
        &state.api_semaphore,
        &state.sessions,
    )
    .await?
    .ok_or(ErrorKind::NoValueFor("category tags".to_string()))?;

    Ok(categories)
}

/// Get report type tags
#[tracing::instrument(skip(state))]
pub async fn get_report_type_tags(state: &State) -> crate::Result<Vec<String>> {
    let report_types = CachedEntry::get_report_types(
        None,
        &state.pool,
        &state.api_semaphore,
        &state.sessions,
    )
    .await?
    .ok_or(ErrorKind::NoValueFor("report type tags".to_string()))?;

    Ok(report_types)
}

/// Get loader tags
#[tracing::instrument(skip(state))]
pub async fn get_loader_tags(state: &State) -> crate::Result<Vec<Loader>> {
    let loaders = CachedEntry::get_loaders(
        None,
        &state.pool,
        &state.api_semaphore,
        &state.sessions,
    )
    .await?
    .ok_or(ErrorKind::NoValueFor("loader tags".to_string()))?;

    Ok(loaders)
}

/// Get game version tags
#[tracing::instrument(skip(state))]
pub async fn get_game_version_tags(
    state: &State,
) -> crate::Result<Vec<GameVersion>> {
    let game_versions = CachedEntry::get_game_versions(
        None,
        &state.pool,
        &state.api_semaphore,
        &state.sessions,
    )
    .await?
    .ok_or(ErrorKind::NoValueFor("game version tags".to_string()))?;

    Ok(game_versions)
}

/// Get donation platform tags
#[tracing::instrument(skip(state))]
pub async fn get_donation_platform_tags(
    state: &State,
) -> crate::Result<Vec<DonationPlatform>> {
    let donation_platforms = CachedEntry::get_donation_platforms(
        None,
        &state.pool,
        &state.api_semaphore,
        &state.sessions,
    )
    .await?
    .ok_or(ErrorKind::NoValueFor("donation platform tags".to_string()))?;

    Ok(donation_platforms)
}
