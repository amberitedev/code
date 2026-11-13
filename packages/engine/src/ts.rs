//! The engine's wire types, generated to `packages/contracts/ts/engine.ts`, which
//! `cargo test -p engine` keeps fresh. Regenerate it with
//! `cargo test -p engine -- --ignored write_ts`.

use contracts::ts::config;
use minecraft_meta_manifest::fabriclike::{
    GameVersion as LoaderGameVersion, LoaderVersion, VersionGroup,
};
use ts_rs::TS;

use crate::{
    api::pack::import::ImportLauncherType,
    endpoints::{
        cache::{self, CacheKey, CacheKeys, CacheRead, CacheSearch},
        groups::{
            self, InstanceGroupRef, InstanceGroupUpdate, NewInstanceGroup,
        },
        instances::{
            self, Instance, InstanceIds, InstanceLink, InstanceRef,
            SharedInstanceAttachment,
        },
        loading,
        metadata::{self, LoaderRef},
        store, tags,
        worlds::{
            self, InstanceServerDesync, InstanceServerRef,
            InstanceServerUpdate, ManagedInstanceServer, NewInstanceServer,
            RecentWorldsQuery, ServerStatusQuery, WorldDisplayStatusUpdate,
            WorldRename, WorldSaveRef,
        },
    },
    event::*,
    install::{
        InstallErrorContext, InstallErrorView, InstallJavaStep, InstallJobKind,
        InstallJobSnapshot, InstallJobStatus, InstallPhaseDetails,
        InstallPhaseId, InstallProgress, InstallProgressSecondary,
        model::{InstallApiErrorDetails, InstallJobDisplay, InstallTarget},
    },
    instance::{
        DesyncServerMode, InstanceGroup, InstanceGroupMembershipUpdate,
        ServerSource,
    },
    metadata::Manifest,
    state::{
        CacheBehaviour, CacheValueType, Category, ContentSetSyncStatus,
        DefaultPage, Dependency, DependencyType, DonationLink, FeatureFlag,
        FileType, GalleryItem, GameVersion, Hooks, InstanceInstallStage,
        InstanceSyncedOptions, InstanceTabVisibility, License, Loader,
        Log4jEvent, MemorySettings, ModLoader, OnboardingChecklist,
        Organization, Project, ProjectV3, ReleaseChannel, SearchEntry,
        SearchResult, SearchResultV3, SearchResults, SearchResultsV3, Settings,
        SharedInstanceRole, SideType, TeamMember, Theme, User, Version,
        VersionFile, WindowSize, content_store::StoreUsage,
    },
    worlds::{
        DisplayStatus, ProtocolVersion, ServerGameProfile, ServerPackStatus,
        ServerPlayers, ServerStatus, ServerVersion, SingleplayerGameMode,
        World, WorldDetails, WorldType, WorldWithInstance,
    },
};

const PATH: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../contracts/ts/engine.ts");

fn file() -> String {
    let cfg = config();
    let declarations = [
        Event::decl(&cfg),
        LoadingPayload::decl(&cfg),
        LoadingBarType::decl(&cfg),
        ProcessPayload::decl(&cfg),
        ProcessPayloadType::decl(&cfg),
        InstancePayload::decl(&cfg),
        InstancePayloadType::decl(&cfg),
        InstallJobSnapshot::decl(&cfg),
        InstallJobKind::decl(&cfg),
        InstallJobStatus::decl(&cfg),
        InstallTarget::decl(&cfg),
        InstallPhaseId::decl(&cfg),
        InstallProgress::decl(&cfg),
        InstallProgressSecondary::decl(&cfg),
        InstallPhaseDetails::decl(&cfg),
        InstallJavaStep::decl(&cfg),
        InstallJobDisplay::decl(&cfg),
        InstallErrorView::decl(&cfg),
        InstallApiErrorDetails::decl(&cfg),
        InstallErrorContext::decl(&cfg),
        ImportLauncherType::decl(&cfg),
        ModLoader::decl(&cfg),
        WarningPayload::decl(&cfg),
        LogPayload::decl(&cfg),
        LogEvent::decl(&cfg),
        Log4jEvent::decl(&cfg),
        LoadingBar::decl(&cfg),
        Instance::decl(&cfg),
        InstanceLink::decl(&cfg),
        InstanceInstallStage::decl(&cfg),
        SharedInstanceAttachment::decl(&cfg),
        SharedInstanceRole::decl(&cfg),
        ContentSetSyncStatus::decl(&cfg),
        InstanceSyncedOptions::decl(&cfg),
        InstanceTabVisibility::decl(&cfg),
        ReleaseChannel::decl(&cfg),
        MemorySettings::decl(&cfg),
        WindowSize::decl(&cfg),
        Hooks::decl(&cfg),
        InstanceRef::decl(&cfg),
        InstanceIds::decl(&cfg),
        InstanceGroup::decl(&cfg),
        InstanceGroupMembershipUpdate::decl(&cfg),
        InstanceGroupRef::decl(&cfg),
        NewInstanceGroup::decl(&cfg),
        InstanceGroupUpdate::decl(&cfg),
        World::decl(&cfg),
        WorldWithInstance::decl(&cfg),
        WorldDetails::decl(&cfg),
        WorldType::decl(&cfg),
        DisplayStatus::decl(&cfg),
        SingleplayerGameMode::decl(&cfg),
        ServerPackStatus::decl(&cfg),
        ServerSource::decl(&cfg),
        DesyncServerMode::decl(&cfg),
        ProtocolVersion::decl(&cfg),
        ServerStatus::decl(&cfg),
        ServerPlayers::decl(&cfg),
        ServerGameProfile::decl(&cfg),
        ServerVersion::decl(&cfg),
        RecentWorldsQuery::decl(&cfg),
        ServerStatusQuery::decl(&cfg),
        WorldSaveRef::decl(&cfg),
        InstanceServerRef::decl(&cfg),
        WorldDisplayStatusUpdate::decl(&cfg),
        WorldRename::decl(&cfg),
        NewInstanceServer::decl(&cfg),
        ManagedInstanceServer::decl(&cfg),
        InstanceServerUpdate::decl(&cfg),
        InstanceServerDesync::decl(&cfg),
        StoreUsage::decl(&cfg),
        Settings::decl(&cfg),
        Theme::decl(&cfg),
        DefaultPage::decl(&cfg),
        FeatureFlag::decl(&cfg),
        Category::decl(&cfg),
        Loader::decl(&cfg),
        GameVersion::decl(&cfg),
        LoaderRef::decl(&cfg),
        Manifest::decl(&cfg),
        LoaderGameVersion::decl(&cfg),
        LoaderVersion::decl(&cfg),
        VersionGroup::decl(&cfg),
        CacheKey::decl(&cfg),
        CacheKeys::decl(&cfg),
        CacheRead::decl(&cfg),
        CacheSearch::decl(&cfg),
        CacheBehaviour::decl(&cfg),
        CacheValueType::decl(&cfg),
        Project::decl(&cfg),
        ProjectV3::decl(&cfg),
        License::decl(&cfg),
        GalleryItem::decl(&cfg),
        DonationLink::decl(&cfg),
        SideType::decl(&cfg),
        Version::decl(&cfg),
        VersionFile::decl(&cfg),
        FileType::decl(&cfg),
        Dependency::decl(&cfg),
        DependencyType::decl(&cfg),
        User::decl(&cfg),
        TeamMember::decl(&cfg),
        Organization::decl(&cfg),
        SearchResults::decl(&cfg),
        SearchResult::decl(&cfg),
        SearchEntry::decl(&cfg),
        SearchResultsV3::decl(&cfg),
        SearchResultV3::decl(&cfg),
        serde_json::Value::decl(&cfg),
        // The payloads of the App's events that engine code still raises.
        FriendPayload::decl(&cfg),
        FriendStatusPayload::decl(&cfg),
        OnboardingChecklist::decl(&cfg),
        InstanceGroupsChangedPayload::decl(&cfg),
        CommandPayload::decl(&cfg),
    ]
    .map(|declaration| format!("\nexport {declaration}\n"))
    .concat();
    let modules = [
        ("instances", instances::ts_entries()),
        ("instanceGroups", groups::ts_entries()),
        ("loadingBars", loading::ts_entries()),
        ("worlds", worlds::ts_entries()),
        ("contentStore", store::ts_entries()),
        ("tags", tags::ts_entries()),
        ("metadata", metadata::ts_entries()),
        ("cache", cache::ts_entries()),
    ]
    .map(|(name, entries)| format!("\nexport const {name} = {{\n{entries}}}\n"))
    .concat();
    format!(
        "// Generated from packages/engine by ts-rs. Do not edit.\n\
         // Regenerate: cargo test -p engine -- --ignored write_ts\n\n\
         import type {{ Endpoint, SharedIconConfig }} from './contracts'\n{declarations}{modules}"
    )
}

#[test]
fn ts_is_up_to_date() {
    let written = std::fs::read_to_string(PATH).unwrap_or_default();
    assert!(
        written.replace("\r\n", "\n") == file(),
        "{PATH} is out of date. Run: cargo test -p engine -- --ignored write_ts"
    );
}

#[test]
#[ignore = "writes the generated file; run on purpose"]
fn write_ts() {
    std::fs::write(PATH, file()).unwrap();
}
