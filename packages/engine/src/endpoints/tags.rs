use contracts::{Empty, Json};

use crate::state::{Category, GameVersion, Loader};

contracts::endpoints! {
    group tags;

    GetCategoryTags: Get "/tags/categories", Session, (), (), Empty => Json<Vec<Category>>, client_only;
    GetLoaderTags: Get "/tags/loaders", Session, (), (), Empty => Json<Vec<Loader>>, client_only;
    GetGameVersionTags: Get "/tags/game-versions", Session, (), (), Empty => Json<Vec<GameVersion>>, client_only;
}
