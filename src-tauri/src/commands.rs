use crate::database::{assets, Database, Note, NoteSummary, NoteInput, Folder, FolderInput};
use tauri::{Manager, State};

/// Error type for command responses
#[derive(Debug, serde::Serialize)]
pub struct CommandError {
    pub message: String,
}

impl From<rusqlite::Error> for CommandError {
    fn from(err: rusqlite::Error) -> Self {
        CommandError {
            message: format!("Database error: {}", err),
        }
    }
}

impl From<String> for CommandError {
    fn from(err: String) -> Self {
        CommandError { message: err }
    }
}

// ============================================================================
// Note Commands
// ============================================================================

/// Get all notes from the database
#[tauri::command]
pub async fn get_all_notes(db: State<'_, Database>) -> Result<Vec<NoteSummary>, CommandError> {
    db.get_all_notes().map_err(|e| e.into())
}

/// Get a single note by ID
#[tauri::command]
pub async fn get_note(db: State<'_, Database>, id: String) -> Result<Option<Note>, CommandError> {
    db.get_note_by_id(&id).map_err(|e| e.into())
}

/// Get notes by folder ID (pass null for root-level notes)
#[tauri::command]
pub async fn get_notes_by_folder(
    db: State<'_, Database>,
    folder_id: Option<String>,
) -> Result<Vec<NoteSummary>, CommandError> {
    db.get_notes_by_folder(folder_id.as_deref())
        .map_err(|e| e.into())
}

/// Save a note (create or update)
#[tauri::command]
pub async fn save_note(db: State<'_, Database>, note: NoteInput) -> Result<Note, CommandError> {
    db.save_note(note).map_err(|e| e.into())
}

/// Delete a note by ID
#[tauri::command]
pub async fn delete_note(db: State<'_, Database>, id: String) -> Result<bool, CommandError> {
    db.delete_note(&id).map_err(|e| e.into())
}

/// Move a note to a folder
#[tauri::command]
pub async fn move_note(
    db: State<'_, Database>, 
    id: String, 
    folder_id: Option<String>
) -> Result<(), CommandError> {
    db.move_note(&id, folder_id.as_deref()).map_err(|e| e.into())
}

/// Get notes updated since an RFC3339 timestamp. Includes deleted notes.
#[tauri::command]
pub async fn get_notes_updated_since(
    db: State<'_, Database>,
    since: Option<String>,
) -> Result<Vec<Note>, CommandError> {
    db.get_notes_updated_since(since.as_deref())
        .map_err(|e| e.into())
}

/// Apply notes pulled from a remote sync.
#[tauri::command]
pub async fn apply_sync_notes(
    db: State<'_, Database>,
    notes: Vec<Note>,
) -> Result<(), CommandError> {
    db.apply_sync_notes(notes).map_err(|e| e.into())
}

// ============================================================================
// Folder Commands
// ============================================================================

/// Get all folders from the database
#[tauri::command]
pub async fn get_all_folders(db: State<'_, Database>) -> Result<Vec<Folder>, CommandError> {
    db.get_all_folders().map_err(|e| e.into())
}

/// Get a single folder by ID
#[tauri::command]
pub async fn get_folder(db: State<'_, Database>, id: String) -> Result<Option<Folder>, CommandError> {
    db.get_folder_by_id(&id).map_err(|e| e.into())
}

/// Get all child folders of a parent folder (or root folders if parent_id is None)
#[tauri::command]
pub async fn get_folders_by_parent(
    db: State<'_, Database>,
    parent_id: Option<String>,
) -> Result<Vec<Folder>, CommandError> {
    let parent_ref = parent_id.as_deref();
    db.get_folders_by_parent(parent_ref).map_err(|e| e.into())
}

/// Save a folder (create or update)
#[tauri::command]
pub async fn save_folder(db: State<'_, Database>, folder: FolderInput) -> Result<Folder, CommandError> {
    db.save_folder(folder).map_err(|e| e.into())
}

/// Delete a folder by ID
#[tauri::command]
pub async fn delete_folder(db: State<'_, Database>, id: String) -> Result<(), CommandError> {
    db.delete_folder(&id).map_err(|e| e.into())
}

// ============================================================================
// Asset Commands
// ============================================================================

/// Save an image asset from base64 data
/// Returns the asset info including the local URI for the frontend
#[tauri::command]
pub async fn save_image_asset(
    app_handle: tauri::AppHandle,
    base64_data: String,
    file_extension: String,
) -> Result<assets::AssetResult, CommandError> {
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| CommandError {
            message: format!("Failed to get app data directory: {}", e),
        })?;

    assets::save_image_asset(&app_data_dir, &base64_data, &file_extension)
        .map_err(|e| e.into())
}

/// Save raw image bytes as an asset
#[tauri::command]
pub async fn save_image_bytes(
    app_handle: tauri::AppHandle,
    data: Vec<u8>,
    file_extension: String,
) -> Result<assets::AssetResult, CommandError> {
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| CommandError {
            message: format!("Failed to get app data directory: {}", e),
        })?;

    assets::save_image_bytes(&app_data_dir, &data, &file_extension)
        .map_err(|e| e.into())
}

/// Save an image asset from a file path
#[tauri::command]
pub async fn save_image_from_path(
    app_handle: tauri::AppHandle,
    path: String,
) -> Result<assets::AssetResult, CommandError> {
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| CommandError {
            message: format!("Failed to get app data directory: {}", e),
        })?;

    let data = std::fs::read(&path)
        .map_err(|e| CommandError {
            message: format!("Failed to read file: {}", e),
        })?;

    let file_extension = std::path::Path::new(&path)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("png")
        .to_string();

    assets::save_image_bytes(&app_data_dir, &data, &file_extension)
        .map_err(|e| e.into())
}

/// Delete an asset by ID
#[tauri::command]
pub async fn delete_asset(
    app_handle: tauri::AppHandle,
    asset_id: String,
) -> Result<bool, CommandError> {
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| CommandError {
            message: format!("Failed to get app data directory: {}", e),
        })?;

    assets::delete_asset(&app_data_dir, &asset_id).map_err(|e| e.into())
}

/// List all assets
#[tauri::command]
pub async fn list_assets(
    app_handle: tauri::AppHandle,
) -> Result<Vec<assets::AssetResult>, CommandError> {
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| CommandError {
            message: format!("Failed to get app data directory: {}", e),
        })?;

    assets::list_assets(&app_data_dir).map_err(|e| e.into())
}

/// Get the assets directory path (for debugging/info)
#[tauri::command]
pub async fn get_assets_path(app_handle: tauri::AppHandle) -> Result<String, CommandError> {
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| CommandError {
            message: format!("Failed to get app data directory: {}", e),
        })?;

    let assets_dir = assets::get_assets_dir(&app_data_dir);
    Ok(assets_dir.to_string_lossy().to_string())
}
