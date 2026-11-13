//! The App's own Endpoints, generated to `packages/contracts/ts/app.ts`, which
//! `cargo test -p theseus_gui` keeps fresh. Regenerate it with
//! `cargo test -p theseus_gui -- --ignored write_ts`.

/// Declares each route as a unit struct and, for the tests, the entries of its TypeScript module
/// object, keyed by the name with a lowercase first letter.
macro_rules! endpoints {
    ($(
        $(#[doc = $doc:literal])*
        $name:ident: $method:ident $path:literal, $params:ty, $query:ty, $body:ty => $response:ty;
    )+) => {
        $(
            $(#[doc = $doc])*
            pub struct $name;

            impl contracts::Endpoint for $name {
                const METHOD: contracts::Method = contracts::Method::$method;
                const PATH: &'static str = $path;
                const AUTH: contracts::Auth = contracts::Auth::None;
                type Params = $params;
                type Query = $query;
                type Body = $body;
                type Response = $response;
            }
        )+

        #[cfg(test)]
        pub(super) fn ts_entries() -> String {
            [$({
                let name = stringify!($name);
                format!(
                    "\t{}{}: {},\n",
                    name[..1].to_lowercase(),
                    &name[1..],
                    contracts::ts::endpoint::<$name>(),
                )
            }),+]
            .concat()
        }
    };
}

pub mod accounts;
pub mod boot;
pub mod cores;
pub mod desktop;
pub mod events;
pub mod files;
pub mod friends;
pub mod game_options;
pub mod health;
pub mod import;
pub mod launch;
pub mod library;
pub mod onboarding;
pub mod ops;
pub mod screenshots;
pub mod settings;
pub mod skins;
pub mod synced_options;
pub mod synced_packs;
pub mod updates;

/// The `{id}` of a route's path.
#[derive(serde::Serialize, serde::Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct IdParams {
    pub id: String,
}

#[cfg(test)]
mod tests {
    use contracts::ts::{config, endpoint};
    use ts_rs::TS;

    use super::{
        IdParams,
        accounts::{self, MinecraftAccountParams, ModrinthSignIn},
        boot::{Boot, GetBoot, Platform},
        cores::{
            CheckCoreSession, ConnectCore, CoreSessionKey, CoreTarget,
            NewCoreSession, SignInToCore, SignOutOfCore,
        },
        desktop::{
            AppFolder, CheckNetworkMetered, CreateShortcut, DialogFilter,
            ExternalLink, GetJavaAt, GetOpeningCommand, JavaAtPath, JavaTest,
            LocalPath, LogFromWebview, NewShortcut, OpenDialog, OpenFolder,
            OpenPath, OpenUrl, PickFiles, PickSavePath, RevealPath, SaveDialog,
            TestJavaAt, WebviewLog, WebviewLogLevel,
        },
        events::EventSocket,
        files::{
            CacheRemoteIcon, FileStat, GetLocalFile, GetThumbnail,
            LocalFileQuery, ReadLocalFile, RemoteIcon, StatLocalFile,
            ThumbnailQuery,
        },
        friends,
        game_options::{self, GameSettingLabelsRequest},
        health::CheckHealth,
        import::{
            CheckImportableInstance, GetDefaultLauncherPath, ImportInstance,
            ImportJob, ImportableInstance, InstanceToImport, Launcher,
            LauncherFolder, ListImportableInstances,
        },
        launch::{
            JoinServer, JoinSingleplayerWorld, ServerJoin, SingleplayerJoin,
        },
        library::{
            InstanceVisibility, ListHiddenInstances, ListRecentIconConfigs,
            RecentIconConfigs, SetInstanceHidden,
        },
        onboarding::{
            GetOnboardingChecklist, OnboardingProgress,
            SkipCreateInstanceOnboarding,
        },
        ops::{
            self, DebugExport, ExternalSave, InstanceFileSave, LocalUpload,
            OpResponse,
        },
        screenshots::{
            self, NewScreenshotGroup, ScreenshotEdit, ScreenshotExport,
            ScreenshotFile, ScreenshotGroupUpdate, ScreenshotMove,
        },
        settings::{CancelDirectoryChange, GetSettings, SetSettings},
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
    use crate::server::events::{
        AmberiteCommand, AppEvent, Command, LocalEvent,
    };
    use engine::instance::{
        EditableGameSetting, GameOptionCanonicalValue, GameOptionCompatibility,
        GameOptionCompatibilityBucket, GameOptionCompatibilityReason,
        GameOptionCompatibilityStatus, GameOptionEditorChoice,
        GameOptionEditorDefinition, GameOptionKind, GameOptionMappingKind,
        GameOptionValidationIssue, GameOptionValueState,
        GameOptionsSourceCandidate, GameOptionsSourceIssue,
        GameSettingCategory, GameSettingChange, GameSettingLocaleLabel,
        GameSettingLocaleLabels, GameSettingSource, GameSettingSourceProject,
        GameSettingsEditorState, GlobalSyncedOptions, PackSyncPreview,
        PackSyncTarget, SaveGameSettingsResult, ScreenshotEditSaveMode,
        ScreenshotGroup, ScreenshotGroupImport,
        ScreenshotGroupMembershipUpdate, ScreenshotKey, SyncedOptionCapability,
        SyncedOptionJoinAction, SyncedOptionJoinPreview,
        SyncedOptionJoinResolution, SyncedOptionsOverview, SyncedServer,
        UpdateGameSettingsRequest,
    };
    use engine::minecraft_skins::{
        Cape, MinecraftSkinVariant, Skin, SkinSource, UrlOrBlob,
    };
    use engine::prelude::InstanceSyncedOption;
    use engine::{
        minecraft_auth::{
            MinecraftAccountSummary, MinecraftCape,
            MinecraftCharacterExpressionState, MinecraftProfile, MinecraftSkin,
        },
        mr_auth::ModrinthAuthFlow,
        prelude::ModrinthCredentials,
    };

    const PATH: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../packages/contracts/ts/app.ts"
    );

    fn file() -> String {
        let cfg = config();
        let declarations = [
            Boot::decl(&cfg),
            Platform::decl(&cfg),
            CoreTarget::decl(&cfg),
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
            OnboardingProgress::decl(&cfg),
            JavaTest::decl(&cfg),
            JavaAtPath::decl(&cfg),
            Launcher::decl(&cfg),
            LauncherFolder::decl(&cfg),
            ImportableInstance::decl(&cfg),
            InstanceToImport::decl(&cfg),
            ImportJob::decl(&cfg),
        ]
        .map(|declaration| format!("\nexport {declaration}\n"))
        .concat();
        format!(
            "// Generated from apps/app by ts-rs. Do not edit.\n\
             // Regenerate: cargo test -p theseus_gui -- --ignored write_ts\n\n\
             import type {{ ApiError, CoreSession, Endpoint, SharedIconConfig, UserFriend, UserStatus }} from './contracts'\n\
             import type {{ CommandPayload, DesyncServerMode, EngineEvent, FriendPayload, ImportLauncherType, InstallJobSnapshot, InstanceGroupsChangedPayload, JavaVersion, OnboardingChecklist, Settings }} from './engine'\n\
             {declarations}\n\
             export const health = {{\n\
             \tcheckHealth: {},\n\
             }}\n\n\
             export const boot = {{\n\
             \tgetBoot: {},\n\
             }}\n\n\
             export const pairedCores = {{\n\
             \tconnectCore: {},\n\
             \tsignInToCore: {},\n\
             \tsignOutOfCore: {},\n\
             \tcheckCoreSession: {},\n\
             }}\n\n\
             export const events = {{\n\
             \teventSocket: {},\n\
             }}\n\n\
             export const localFiles = {{\n\
             \tgetLocalFile: {},\n\
             \tgetThumbnail: {},\n\
             \tcacheRemoteIcon: {},\n\
             \treadLocalFile: {},\n\
             \tstatLocalFile: {},\n\
             }}\n\n\
             export const desktop = {{\n\
             \tpickFiles: {},\n\
             \tpickSavePath: {},\n\
             \topenUrl: {},\n\
             \topenPath: {},\n\
             \trevealPath: {},\n\
             \topenFolder: {},\n\
             \tcreateShortcut: {},\n\
             \tcheckNetworkMetered: {},\n\
             \tlogFromWebview: {},\n\
             \tgetOpeningCommand: {},\n\
             \tgetJavaAt: {},\n\
             \ttestJavaAt: {},\n\
             }}\n\n\
             export const library = {{\n\
             \tlistRecentIconConfigs: {},\n\
             \tlistHiddenInstances: {},\n\
             \tsetInstanceHidden: {},\n\
             }}\n\n\
             export const gameOptions = {{\n{}}}\n\n\
             export const syncedOptions = {{\n{}}}\n\n\
             export const syncedPacks = {{\n{}}}\n\n\
             export const screenshots = {{\n{}}}\n\n\
             export const friends = {{\n{}}}\n\n\
             export const skins = {{\n{}}}\n\n\
             export const ops = {{\n{}}}\n\n\
             export const updates = {{\n{}}}\n\n\
             export const localAccounts = {{\n{}}}\n\n\
             export const launch = {{\n\
             \tjoinServer: {},\n\
             \tjoinSingleplayerWorld: {},\n\
             }}\n\n\
             export const settings = {{\n\
             \tgetSettings: {},\n\
             \tsetSettings: {},\n\
             \tcancelDirectoryChange: {},\n\
             }}\n\n\
             export const onboarding = {{\n\
             \tgetOnboardingChecklist: {},\n\
             \tskipCreateInstanceOnboarding: {},\n\
             }}\n\n\
             export const launcherImport = {{\n\
             \tlistImportableInstances: {},\n\
             \tcheckImportableInstance: {},\n\
             \tgetDefaultLauncherPath: {},\n\
             \timportInstance: {},\n\
             }}\n",
            endpoint::<CheckHealth>(),
            endpoint::<GetBoot>(),
            endpoint::<ConnectCore>(),
            endpoint::<SignInToCore>(),
            endpoint::<SignOutOfCore>(),
            endpoint::<CheckCoreSession>(),
            endpoint::<EventSocket>(),
            endpoint::<GetLocalFile>(),
            endpoint::<GetThumbnail>(),
            endpoint::<CacheRemoteIcon>(),
            endpoint::<ReadLocalFile>(),
            endpoint::<StatLocalFile>(),
            endpoint::<PickFiles>(),
            endpoint::<PickSavePath>(),
            endpoint::<OpenUrl>(),
            endpoint::<OpenPath>(),
            endpoint::<RevealPath>(),
            endpoint::<OpenFolder>(),
            endpoint::<CreateShortcut>(),
            endpoint::<CheckNetworkMetered>(),
            endpoint::<LogFromWebview>(),
            endpoint::<GetOpeningCommand>(),
            endpoint::<GetJavaAt>(),
            endpoint::<TestJavaAt>(),
            endpoint::<ListRecentIconConfigs>(),
            endpoint::<ListHiddenInstances>(),
            endpoint::<SetInstanceHidden>(),
            game_options::ts_entries(),
            synced_options::ts_entries(),
            synced_packs::ts_entries(),
            screenshots::ts_entries(),
            friends::ts_entries(),
            skins::ts_entries(),
            ops::ts_entries(),
            updates::ts_entries(),
            accounts::ts_entries(),
            endpoint::<JoinServer>(),
            endpoint::<JoinSingleplayerWorld>(),
            endpoint::<GetSettings>(),
            endpoint::<SetSettings>(),
            endpoint::<CancelDirectoryChange>(),
            endpoint::<GetOnboardingChecklist>(),
            endpoint::<SkipCreateInstanceOnboarding>(),
            endpoint::<ListImportableInstances>(),
            endpoint::<CheckImportableInstance>(),
            endpoint::<GetDefaultLauncherPath>(),
            endpoint::<ImportInstance>(),
        )
    }

    #[test]
    fn ts_is_up_to_date() {
        let written = std::fs::read_to_string(PATH).unwrap_or_default();
        assert!(
            written.replace("\r\n", "\n") == file(),
            "{PATH} is out of date. Run: cargo test -p theseus_gui -- --ignored write_ts"
        );
    }

    #[test]
    #[ignore = "writes the generated file; run on purpose"]
    fn write_ts() {
        std::fs::write(PATH, file()).unwrap();
    }
}
