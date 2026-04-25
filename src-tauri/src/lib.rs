use std::fs;
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
async fn pick_pdf(app: tauri::AppHandle) -> Result<Vec<u8>, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter("PDF", &["pdf"])
        .set_title("Open PDF")
        .pick_file(move |path| {
            let _ = tx.send(path);
        });

    match rx.await {
        Ok(Some(path)) => {
            let path_str = match path {
                tauri_plugin_dialog::FilePath::Path(p) => p.to_string_lossy().to_string(),
                tauri_plugin_dialog::FilePath::Url(u) => u.to_string(),
            };
            fs::read(&path_str).map_err(|e| format!("Failed to read file: {}", e))
        }
        Ok(None) => Err("No file was picked".to_string()),
        Err(_) => Err("Failed to receive file path".to_string()),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![pick_pdf])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
