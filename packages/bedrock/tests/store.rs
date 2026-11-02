use bedrock::upstream::store::{self, DirectoryInfo, SCHEMA, State};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

#[tokio::test]
async fn retained_content_survives_cleanup_until_released() {
    let dir = tempfile::tempdir().unwrap();
    let pool = SqlitePoolOptions::new()
        .connect_with(
            SqliteConnectOptions::new()
                .filename(dir.path().join("store.db"))
                .create_if_missing(true),
        )
        .await
        .unwrap();
    sqlx::raw_sql(SCHEMA).execute(&pool).await.unwrap();
    let dirs = DirectoryInfo {
        settings_dir: dir.path().to_path_buf(),
        config_dir: dir.path().to_path_buf(),
    };
    let content = store::open(&dirs, pool.clone()).await.unwrap();
    let state = State::new(pool);

    let stored = store::store_bytes(&content, b"a pushed config")
        .await
        .unwrap();
    let sha512 = stored.sha512().to_owned();
    assert_eq!(std::fs::read(stored.path()).unwrap(), b"a pushed config");
    drop(stored);
    store::retain(
        &content,
        "share_history",
        "1",
        std::slice::from_ref(&sha512),
    )
    .await
    .unwrap();
    content.cleanup(&state, true).await.unwrap();
    assert!(store::lookup(&content, &sha512).await.unwrap().is_some());

    store::release(&content, "share_history", "1")
        .await
        .unwrap();
    content.cleanup(&state, true).await.unwrap();
    assert!(store::lookup(&content, &sha512).await.unwrap().is_none());
}
