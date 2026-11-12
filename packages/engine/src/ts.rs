//! The engine's wire types, generated to `packages/contracts/ts/engine.ts`, which
//! `cargo test -p engine` keeps fresh. Regenerate it with
//! `cargo test -p engine -- --ignored write_ts`.

use contracts::ts::config;
use ts_rs::TS;

use crate::{
    api::pack::import::ImportLauncherType,
    endpoints::{
        groups::{
            self, InstanceGroupRef, InstanceGroupUpdate, NewInstanceGroup,
        },
        instances::{
            self, Instance, InstanceIds, InstanceLink, InstanceRef,
            SharedInstanceAttachment,
        },
        loading,
    },
    event::*,
    install::{
        InstallErrorContext, InstallErrorView, InstallJavaStep, InstallJobKind,
        InstallJobSnapshot, InstallJobStatus, InstallPhaseDetails,
        InstallPhaseId, InstallProgress, InstallProgressSecondary,
        model::{InstallApiErrorDetails, InstallJobDisplay, InstallTarget},
    },
    instance::{InstanceGroup, InstanceGroupMembershipUpdate},
    state::{
        ContentSetSyncStatus, Hooks, IconBackground, IconConfig,
        InstanceInstallStage, InstanceSyncedOptions, InstanceTabVisibility,
        Log4jEvent, MemorySettings, ModLoader, OnboardingChecklist,
        ReleaseChannel, SharedInstanceRole, WindowSize,
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
        IconConfig::decl(&cfg),
        IconBackground::decl(&cfg),
        InstanceRef::decl(&cfg),
        InstanceIds::decl(&cfg),
        InstanceGroup::decl(&cfg),
        InstanceGroupMembershipUpdate::decl(&cfg),
        InstanceGroupRef::decl(&cfg),
        NewInstanceGroup::decl(&cfg),
        InstanceGroupUpdate::decl(&cfg),
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
    ]
    .map(|(name, entries)| format!("\nexport const {name} = {{\n{entries}}}\n"))
    .concat();
    format!(
        "// Generated from packages/engine by ts-rs. Do not edit.\n\
         // Regenerate: cargo test -p engine -- --ignored write_ts\n\n\
         import type {{ Endpoint }} from './contracts'\n{declarations}{modules}"
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
