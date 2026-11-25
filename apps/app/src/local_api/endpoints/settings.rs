use contracts::{Empty, Json};

use crate::settings::Settings;

contracts::endpoints! {
    group settings;

    GetSettings: Get "/app/settings", None, (), (), Empty => Json<Settings>;
    /// Replaces every setting.
    SetSettings: Put "/app/settings", None, (), (), Json<Settings> => Empty;
    /// Keeps the previous data folder after a move failed. It answers while the engine cannot start.
    CancelDirectoryChange: Delete "/app/settings/directory-change", None, (), (), Empty => Empty;
}
