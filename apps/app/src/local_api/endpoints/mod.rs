//! The App's own Endpoints, generated to `packages/contracts/ts/app.ts`, which
//! `cargo test -p amberite-app` keeps fresh. Regenerate it with
//! `WRITE_TS=1 node scripts/dev-runner.ts test amberite-app`.

pub mod accounts;
pub mod boot;
pub mod cores;
pub mod desktop;
pub mod events;
pub mod files;
pub mod friends;
pub mod game_options;
pub mod groups;
pub mod health;
pub mod import;
pub mod install;
pub mod instance_paths;
pub mod launch;
pub mod library;
pub mod onboarding;
pub mod ops;
pub mod screenshots;
pub mod settings;
pub mod sharing;
pub mod skins;
pub mod synced_options;
pub mod synced_packs;
pub mod updates;

/// The `{id}` of a route's path.
#[derive(serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct IdParams {
    pub id: String,
}

#[cfg(all(test, feature = "export-ts"))]
mod tests {
    use contracts::ts::config;
    use ts_rs::TS;

    use super::{
        IdParams,
        accounts::{self, MinecraftAccountParams, ModrinthSignIn},
        boot::{self, Boot, Platform},
        cores::{
            self, CoreRoute, CoreSessionKey, CoreTarget, NewCoreSession,
            Reached,
        },
        desktop::{
            self, AppFolder, DialogFilter, ExternalLink, JavaAtPath, JavaTest,
            LocalPath, NewShortcut, OpenDialog, SaveDialog, WebviewLog,
            WebviewLogLevel,
        },
        events,
        files::{self, FileStat, LocalFileQuery, RemoteIcon, ThumbnailQuery},
        friends,
        game_options::{self, GameSettingLabelsRequest},
        groups::{
            self, InstanceGroupRef, InstanceGroupUpdate, NewInstanceGroup,
        },
        health,
        import::{
            self, ImportJob, ImportableInstance, InstanceToImport, Launcher,
            LauncherFolder,
        },
        install::{
            self, InstallCreateInstanceRequest, InstallModpackPreview,
            InstallPostInstallEdit, PackInstall,
        },
        instance_paths::{
            self, ContentPathQuery, LocalContentFile, LocalIcon, LocalProject,
            MrpackExport,
        },
        launch::{self, InstancePlay, ServerJoin, SingleplayerJoin},
        library::{
            self, GeneratedIcon, GeneratedInstanceIcon, InstanceVisibility,
            RecentIconConfigs, RecentWorldsQuery,
        },
        onboarding,
        ops::{
            self, DebugExport, ExternalSave, InstanceFileSave, LocalUpload,
            OpResponse,
        },
        screenshots::{
            self, NewScreenshotGroup, ScreenshotEdit, ScreenshotExport,
            ScreenshotFile, ScreenshotGroupUpdate, ScreenshotMove,
        },
        settings,
        sharing::{self, SharePublish},
        skins::{self, CustomSkinSave, NewCustomSkin, SkinFlushQuery},
        synced_options::{
            self, CommandHistory, GlobalSyncedOptionUpdate,
            InstanceSyncedOptionUpdate, SyncedOptionQuery,
            SyncedOptionsRebuild,
        },
        synced_packs::{
            self, DesyncQuery, InstancePack, InstancePackParams, NewSyncedPack,
            SyncedPackQuery, SyncedPackType, SyncedPackUpdate, SyncedPacks,
        },
        updates::{self, AvailableUpdate, RestartAfterUpdate, UpdateDownload},
    };
    use crate::accounts::{
        mr_auth::ModrinthAuthFlow,
        state::{
            MinecraftAccountSummary, MinecraftCape,
            MinecraftCharacterExpressionState, MinecraftProfile, MinecraftSkin,
            ModrinthCredentials,
        },
    };
    use crate::desktop::deep_links::CommandPayload;
    use crate::friends::{FriendPayload, FriendStatusPayload};
    use crate::library::groups::{
        InstanceGroup, InstanceGroupMembershipUpdate,
        InstanceGroupsChangedPayload,
    };
    use crate::local_api::events::{
        AmberiteCommand, AppEvent, Command, LocalEvent,
    };
    use crate::onboarding::state::OnboardingChecklist;
    use crate::screenshots::{
        ScreenshotEditSaveMode, ScreenshotGroup, ScreenshotGroupImport,
        ScreenshotGroupMembershipUpdate, ScreenshotKey,
    };
    use crate::settings::{DefaultPage, FeatureFlag, Settings, Theme};
    use crate::skins::minecraft_skins::{
        Cape, MinecraftSkinVariant, Skin, SkinSource, UrlOrBlob,
    };
    use crate::synced_options::{
        EditableGameSetting, GameOptionCanonicalValue, GameOptionCompatibility,
        GameOptionCompatibilityBucket, GameOptionCompatibilityReason,
        GameOptionCompatibilityStatus, GameOptionEditorChoice,
        GameOptionEditorDefinition, GameOptionKind, GameOptionMappingKind,
        GameOptionValidationIssue, GameOptionValueState,
        GameOptionsSourceCandidate, GameOptionsSourceIssue,
        GameSettingCategory, GameSettingChange, GameSettingLocaleLabel,
        GameSettingLocaleLabels, GameSettingSource, GameSettingSourceProject,
        GameSettingsEditorState, GlobalSyncedOptions, PackSyncPreview,
        PackSyncTarget, SaveGameSettingsResult, SyncedOptionCapability,
        SyncedOptionJoinAction, SyncedOptionJoinPreview,
        SyncedOptionJoinResolution, SyncedOptionsOverview, SyncedServer,
        UpdateGameSettingsRequest,
    };
    use engine::pack::install_from::CreatePackLocation;
    use engine::prelude::InstanceSyncedOption;

    const DIR: &str =
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../packages/contracts/ts");

    fn file() -> String {
        let cfg = config();
        let declarations = [
            Boot::decl(&cfg),
            Platform::decl(&cfg),
            CoreTarget::decl(&cfg),
            CoreRoute::decl(&cfg),
            Reached::decl(&cfg),
            NewCoreSession::decl(&cfg),
            CoreSessionKey::decl(&cfg),
            LocalFileQuery::decl(&cfg),
            ThumbnailQuery::decl(&cfg),
            RemoteIcon::decl(&cfg),
            FileStat::decl(&cfg),
            OpenDialog::decl(&cfg),
            SaveDialog::decl(&cfg),
            DialogFilter::decl(&cfg),
            ExternalLink::decl(&cfg),
            LocalPath::decl(&cfg),
            AppFolder::decl(&cfg),
            NewShortcut::decl(&cfg),
            WebviewLog::decl(&cfg),
            WebviewLogLevel::decl(&cfg),
            LocalEvent::decl(&cfg),
            AppEvent::decl(&cfg),
            Command::decl(&cfg),
            AmberiteCommand::decl(&cfg),
            RecentIconConfigs::decl(&cfg),
            IdParams::decl(&cfg),
            GameSettingsEditorState::decl(&cfg),
            GameSettingCategory::decl(&cfg),
            EditableGameSetting::decl(&cfg),
            GameOptionKind::decl(&cfg),
            GameOptionCanonicalValue::decl(&cfg),
            GameOptionValueState::decl(&cfg),
            GameOptionEditorDefinition::decl(&cfg),
            GameOptionEditorChoice::decl(&cfg),
            GameOptionCompatibility::decl(&cfg),
            GameOptionCompatibilityBucket::decl(&cfg),
            GameOptionCompatibilityStatus::decl(&cfg),
            GameOptionMappingKind::decl(&cfg),
            GameOptionCompatibilityReason::decl(&cfg),
            GameOptionValidationIssue::decl(&cfg),
            UpdateGameSettingsRequest::decl(&cfg),
            GameSettingChange::decl(&cfg),
            SaveGameSettingsResult::decl(&cfg),
            GameOptionsSourceCandidate::decl(&cfg),
            GameOptionsSourceIssue::decl(&cfg),
            GameSettingLabelsRequest::decl(&cfg),
            GameSettingLocaleLabels::decl(&cfg),
            GameSettingLocaleLabel::decl(&cfg),
            GameSettingSource::decl(&cfg),
            GameSettingSourceProject::decl(&cfg),
            InstanceSyncedOption::decl(&cfg),
            InstanceSyncedOptionUpdate::decl(&cfg),
            SyncedOptionJoinResolution::decl(&cfg),
            SyncedOptionQuery::decl(&cfg),
            SyncedOptionJoinPreview::decl(&cfg),
            SyncedOptionJoinAction::decl(&cfg),
            SyncedOptionsOverview::decl(&cfg),
            SyncedOptionCapability::decl(&cfg),
            GlobalSyncedOptions::decl(&cfg),
            GlobalSyncedOptionUpdate::decl(&cfg),
            SyncedOptionsRebuild::decl(&cfg),
            CommandHistory::decl(&cfg),
            SyncedServer::decl(&cfg),
            SyncedPackType::decl(&cfg),
            SyncedPackQuery::decl(&cfg),
            SyncedPacks::decl(&cfg),
            NewSyncedPack::decl(&cfg),
            SyncedPackUpdate::decl(&cfg),
            InstancePack::decl(&cfg),
            InstancePackParams::decl(&cfg),
            PackSyncPreview::decl(&cfg),
            PackSyncTarget::decl(&cfg),
            DesyncQuery::decl(&cfg),
            ScreenshotFile::decl(&cfg),
            ScreenshotKey::decl(&cfg),
            ScreenshotEdit::decl(&cfg),
            ScreenshotEditSaveMode::decl(&cfg),
            ScreenshotExport::decl(&cfg),
            ScreenshotMove::decl(&cfg),
            ScreenshotGroup::decl(&cfg),
            NewScreenshotGroup::decl(&cfg),
            ScreenshotGroupUpdate::decl(&cfg),
            ScreenshotGroupMembershipUpdate::decl(&cfg),
            ScreenshotGroupImport::decl(&cfg),
            Cape::decl(&cfg),
            Skin::decl(&cfg),
            MinecraftSkinVariant::decl(&cfg),
            SkinSource::decl(&cfg),
            NewCustomSkin::decl(&cfg),
            CustomSkinSave::decl(&cfg),
            SkinFlushQuery::decl(&cfg),
            UrlOrBlob::decl(&cfg),
            DebugExport::decl(&cfg),
            ExternalSave::decl(&cfg),
            LocalUpload::decl(&cfg),
            InstanceFileSave::decl(&cfg),
            OpResponse::decl(&cfg),
            AvailableUpdate::decl(&cfg),
            UpdateDownload::decl(&cfg),
            RestartAfterUpdate::decl(&cfg),
            MinecraftAccountSummary::decl(&cfg),
            MinecraftProfile::decl(&cfg),
            MinecraftSkin::decl(&cfg),
            MinecraftCape::decl(&cfg),
            MinecraftCharacterExpressionState::decl(&cfg),
            MinecraftAccountParams::decl(&cfg),
            ModrinthCredentials::decl(&cfg),
            ModrinthSignIn::decl(&cfg),
            ModrinthAuthFlow::decl(&cfg),
            ServerJoin::decl(&cfg),
            SingleplayerJoin::decl(&cfg),
            InstanceVisibility::decl(&cfg),
            RecentWorldsQuery::decl(&cfg),
            InstanceGroup::decl(&cfg),
            InstanceGroupMembershipUpdate::decl(&cfg),
            InstanceGroupRef::decl(&cfg),
            NewInstanceGroup::decl(&cfg),
            InstanceGroupUpdate::decl(&cfg),
            InstanceGroupsChangedPayload::decl(&cfg),
            OnboardingChecklist::decl(&cfg),
            FriendPayload::decl(&cfg),
            FriendStatusPayload::decl(&cfg),
            CommandPayload::decl(&cfg),
            Settings::decl(&cfg),
            Theme::decl(&cfg),
            DefaultPage::decl(&cfg),
            FeatureFlag::decl(&cfg),
            JavaTest::decl(&cfg),
            JavaAtPath::decl(&cfg),
            Launcher::decl(&cfg),
            LauncherFolder::decl(&cfg),
            ImportableInstance::decl(&cfg),
            InstanceToImport::decl(&cfg),
            ImportJob::decl(&cfg),
            CreatePackLocation::decl(&cfg),
            InstallModpackPreview::decl(&cfg),
            InstallCreateInstanceRequest::decl(&cfg),
            PackInstall::decl(&cfg),
            InstallPostInstallEdit::decl(&cfg),
            InstancePlay::decl(&cfg),
            ContentPathQuery::decl(&cfg),
            LocalProject::decl(&cfg),
            LocalContentFile::decl(&cfg),
            MrpackExport::decl(&cfg),
            LocalIcon::decl(&cfg),
            GeneratedIcon::decl(&cfg),
            GeneratedInstanceIcon::decl(&cfg),
            SharePublish::decl(&cfg),
        ]
        .map(|declaration| format!("\nexport {declaration}\n"))
        .concat();
        let modules: String = [
            health::ts_module(),
            boot::ts_module(),
            cores::ts_module(),
            events::ts_module(),
            files::ts_module(),
            desktop::ts_module(),
            library::ts_module(),
            groups::ts_module(),
            game_options::ts_module(),
            synced_options::ts_module(),
            synced_packs::ts_module(),
            screenshots::ts_module(),
            friends::ts_module(),
            skins::ts_module(),
            ops::ts_module(),
            updates::ts_module(),
            accounts::ts_module(),
            launch::ts_module(),
            settings::ts_module(),
            onboarding::ts_module(),
            import::ts_module(),
            install::ts_module(),
            instance_paths::ts_module(),
            sharing::ts_module(),
        ]
        .map(|module| module.ts(true))
        .concat();
        let earlier = ["protocol", "backend", "core", "engine"].map(|name| {
            let path = format!("{DIR}/{name}.ts");
            (name, std::fs::read_to_string(path).unwrap_or_default())
        });
        let earlier = earlier
            .each_ref()
            .map(|(name, contents)| (*name, contents.as_str()));
        contracts::ts::file(
            "amberite-app",
            &format!("{declarations}{modules}"),
            &earlier,
        )
    }

    #[test]
    fn ts_is_up_to_date() {
        contracts::ts::check(&format!("{DIR}/app.ts"), &file());
    }
}
