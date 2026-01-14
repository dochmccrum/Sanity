mod commands;
mod database;

use database::Database;
use tauri::Manager;

/// Initialize and run the Tauri application
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // Get the app data directory for database storage
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to get app data directory");

            // Initialize the database
            let db = Database::new(&app_data_dir)
                .expect("Failed to initialize database");

            // Store database as managed state
            app.manage(db);

            // Enable asset protocol for serving local files
            #[cfg(debug_assertions)]
            {
                println!("App data directory: {:?}", app_data_dir);
                println!("Assets directory: {:?}", app_data_dir.join(".assets"));
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Note commands
            commands::get_all_notes,
            commands::get_note,
            commands::get_notes_by_folder,
            commands::save_note,
            commands::delete_note,
            // Folder commands
            commands::get_all_folders,
            commands::get_folder,
            commands::get_folders_by_parent,
            commands::save_folder,
            commands::delete_folder,
            // Asset commands
            commands::save_image_asset,
            commands::save_image_bytes,
            commands::delete_asset,
            commands::list_assets,
            commands::get_assets_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
