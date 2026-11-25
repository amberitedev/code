//! The selected Minecraft account's skins and capes.

use crate::skins::minecraft_skins::{
    Bytes, Cape, MinecraftSkinVariant, Skin, UrlOrBlob,
};
use contracts::{Empty, Json, Raw};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

contracts::endpoints! {
    group skins;

    ListCapes: Get "/app/skins/capes", None, (), (), Empty => Json<Vec<Cape>>;
    ListSkins: Get "/app/skins", None, (), (), Empty => Json<Vec<Skin>>;
    /// Saves a custom skin without equipping it.
    SaveCustomSkin: Post "/app/skins", None, (), (), Json<CustomSkinSave> => Json<Skin>;
    RemoveCustomSkin: Delete "/app/skins", None, (), (), Json<Skin> => Empty;
    /// Orders the custom skins by texture key.
    SetCustomSkinOrder: Put "/app/skins/order", None, (), (), Json<Vec<String>> => Empty;
    /// Saves a custom skin and equips it.
    AddAndEquipCustomSkin: Post "/app/skins/equipped", None, (), (), Json<NewCustomSkin> => Json<Skin>;
    EquipSkin: Put "/app/skins/equipped", None, (), (), Json<Skin> => Empty;
    /// Resets the account to a default skin and takes off its cape.
    UnequipSkin: Delete "/app/skins/equipped", None, (), (), Empty => Empty;
    /// Sends a debounced skin change to Mojang now, for one account or all of them.
    FlushPendingSkinChange: Post "/app/skins/flush", None, (), SkinFlushQuery, Empty => Empty;
    /// The texture as a 64x64 PNG for display.
    NormalizeSkinTexture: Post "/app/skins/normalize", None, (), (), Json<UrlOrBlob> => Raw;
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct NewCustomSkin {
    pub texture_blob: Bytes,
    pub variant: MinecraftSkinVariant,
    #[cfg_attr(feature = "export-ts", ts(optional = nullable))]
    pub cape: Option<Cape>,
}

/// A custom skin's new texture, model and cape; `replace_texture` gives it a new texture key.
#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct CustomSkinSave {
    pub skin: Skin,
    pub texture_blob: Bytes,
    pub variant: MinecraftSkinVariant,
    #[cfg_attr(feature = "export-ts", ts(optional = nullable))]
    pub cape: Option<Cape>,
    pub replace_texture: bool,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct SkinFlushQuery {
    #[cfg_attr(feature = "export-ts", ts(optional = nullable))]
    pub profile_id: Option<Uuid>,
}
