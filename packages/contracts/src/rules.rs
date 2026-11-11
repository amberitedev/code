//! What a server's share may carry. `sharing_client::rules` applies them; the UI reads the same
//! values from the generated TypeScript.

crate::consts! {
    /// The folder a server's shared files live in.
    SERVER_SHARE_FOLDER: &str = "config";
    /// Extensions of the config files a server's share may carry.
    SHAREABLE_CONFIG_EXTENSIONS: [&str; 14] = [
        "json",
        "json5",
        "jsonc",
        "yml",
        "yaml",
        "css",
        "toml",
        "txt",
        "ini",
        "cfg",
        "conf",
        "properties",
        "xml",
        "nbt",
    ];
}
