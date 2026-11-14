use contracts::{Empty, Json};

use super::endpoints;
use crate::state::{Category, GameVersion, Loader};

endpoints! {
    GetCategoryTags: Get "/tags/categories", (), (), Empty => Json<Vec<Category>>;
    GetLoaderTags: Get "/tags/loaders", (), (), Empty => Json<Vec<Loader>>;
    GetGameVersionTags: Get "/tags/game-versions", (), (), Empty => Json<Vec<GameVersion>>;
}
