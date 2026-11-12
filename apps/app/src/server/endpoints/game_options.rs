//! The game settings editor, for the settings every synced instance shares and for one instance's
//! own.

use contracts::{Empty, Json};
use engine::instance::{
    GameOptionsSourceCandidate, GameSettingLocaleLabels,
    GameSettingsEditorState, SaveGameSettingsResult, UpdateGameSettingsRequest,
};
use serde::{Deserialize, Serialize};

use super::IdParams;

endpoints! {
    /// The instances whose `options.txt` can seed the shared settings.
    ListGameOptionsSources: Get "/app/game-options/sources", (), (), Empty
        => Json<Vec<GameOptionsSourceCandidate>>;
    GetSyncedGameOptions: Get "/app/game-options", (), (), Empty
        => Json<GameSettingsEditorState>;
    /// The editor as it would be after the changes, without saving them.
    PreviewSyncedGameOptions: Post "/app/game-options/preview", (), (),
        Json<UpdateGameSettingsRequest> => Json<GameSettingsEditorState>;
    SaveSyncedGameOptions: Patch "/app/game-options", (), (),
        Json<UpdateGameSettingsRequest> => Json<SaveGameSettingsResult>;
    GetInstanceGameOptions: Get "/app/instances/{id}/game-options",
        IdParams, (), Empty => Json<GameSettingsEditorState>;
    PreviewInstanceGameOptions: Post "/app/instances/{id}/game-options/preview",
        IdParams, (), Json<UpdateGameSettingsRequest>
        => Json<GameSettingsEditorState>;
    SaveInstanceGameOptions: Patch "/app/instances/{id}/game-options",
        IdParams, (), Json<UpdateGameSettingsRequest>
        => Json<SaveGameSettingsResult>;
    /// The settings' names and choices in `locale`, read from the game and mod files.
    GetGameSettingLabels: Post "/app/game-options/labels", (), (),
        Json<GameSettingLabelsRequest> => Json<GameSettingLocaleLabels>;
}

/// `instance_id` reads the labels of that instance's files; `refresh_sources` rescans them.
#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct GameSettingLabelsRequest {
    #[cfg_attr(test, ts(optional))]
    pub instance_id: Option<String>,
    pub locale: String,
    pub option_ids: Vec<String>,
    pub refresh_sources: bool,
}
