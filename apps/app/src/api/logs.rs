pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("logs")
        .js_init_script(include_str!("webview-logs.js"))
        .build()
}
