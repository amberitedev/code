use contracts::{Empty, Json};

use crate::state::{Category, GameVersion, Loader};

contracts::endpoints! {
    group tags;

    GetCategoryTags: Get "/tags/categories", Session, (), (), Empty => Json<Vec<Category>>;
    GetLoaderTags: Get "/tags/loaders", Session, (), (), Empty => Json<Vec<Loader>>;
    GetGameVersionTags: Get "/tags/game-versions", Session, (), (), Empty => Json<Vec<GameVersion>>;
}
