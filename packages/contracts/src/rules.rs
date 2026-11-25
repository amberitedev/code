//! What a share may carry. `sharing_client` applies them, and the backend and the UI read the same
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
    /// The folders a modpack file members do not get may be named in, by path.
    REMOVED_FILE_FOLDERS: [&str; 5] = [
        "mods",
        "plugins",
        "datapacks",
        "shaderpacks",
        "resourcepacks",
    ];
    /// The endings of a modpack file members do not get, named by path: an addon archive, on or
    /// turned off.
    REMOVED_FILE_SUFFIXES: [&str; 4] =
        [".jar", ".zip", ".jar.disabled", ".zip.disabled"];
}
