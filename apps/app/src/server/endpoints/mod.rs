//! The App's own Endpoints, generated to `packages/contracts/ts/app.ts`, which
//! `cargo test -p theseus_gui` keeps fresh. Regenerate it with
//! `cargo test -p theseus_gui -- --ignored write_ts`.

pub mod boot;
pub mod cores;
pub mod desktop;
pub mod events;
pub mod files;
pub mod health;

#[cfg(test)]
mod tests {
    use contracts::ts::{config, endpoint};
    use ts_rs::TS;

    use super::{
        boot::{Boot, GetBoot, Platform},
        cores::{
            CheckCoreSession, ConnectCore, CoreSessionKey, CoreTarget,
            NewCoreSession, SignInToCore, SignOutOfCore,
        },
        desktop::{
            AppFolder, CheckNetworkMetered, CreateShortcut, DialogFilter,
            ExternalLink, LocalPath, LogFromWebview, NewShortcut, OpenDialog,
            OpenFolder, OpenPath, OpenUrl, PickFiles, PickSavePath, RevealPath,
            SaveDialog, WebviewLog, WebviewLogLevel,
        },
        events::EventSocket,
        files::{
            CacheRemoteIcon, FileStat, GetLocalFile, GetThumbnail,
            LocalFileQuery, ReadLocalFile, RemoteIcon, StatLocalFile,
            ThumbnailQuery,
        },
        health::CheckHealth,
    };
    use crate::server::events::{
        AmberiteCommand, AppEvent, Command, LocalEvent,
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
        ]
        .map(|declaration| format!("\nexport {declaration}\n"))
        .concat();
        format!(
            "// Generated from apps/app by ts-rs. Do not edit.\n\
             // Regenerate: cargo test -p theseus_gui -- --ignored write_ts\n\n\
             import type {{ CoreSession, Endpoint }} from './contracts'\n\
             import type {{ CommandPayload, EngineEvent, FriendPayload, InstanceGroupsChangedPayload, OnboardingChecklist }} from './engine'\n\
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
