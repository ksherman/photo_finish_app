mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::list_volumes,
            commands::list_directory,
            commands::get_file_count,
            commands::copy_files_to_destination,
            commands::rename_folder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
