use rusqlite::{Connection, Result as SqliteResult, params};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use uuid::Uuid;

/// Represents a note in the database
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Note {
    pub id: String,
    pub title: String,
    pub content: String,
    pub folder_id: Option<String>,
    pub updated_at: String,
    pub is_canvas: bool,
}

/// Represents a folder in the database
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Folder {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub created_at: String,
}

/// Input structure for creating/updating folders
#[derive(Debug, Serialize, Deserialize)]
pub struct FolderInput {
    pub id: Option<String>,
    pub name: String,
    pub parent_id: Option<String>,
}

/// Input structure for creating/updating notes
#[derive(Debug, Serialize, Deserialize)]
pub struct NoteInput {
    pub id: Option<String>,
    pub title: String,
    pub content: String,
    pub folder_id: Option<String>,
    pub is_canvas: bool,
}

/// Database wrapper for thread-safe access
pub struct Database {
    pub conn: Mutex<Connection>,
}

impl Database {
    /// Initialize the database connection and create tables
    pub fn new(app_data_dir: &PathBuf) -> SqliteResult<Self> {
        // Ensure the app data directory exists
        fs::create_dir_all(app_data_dir).expect("Failed to create app data directory");

        // Create the database file path
        let db_path = app_data_dir.join("notes.db");

        // Open or create the database
        let conn = Connection::open(&db_path)?;

        // Enable foreign keys and WAL mode for better performance
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;"
        )?;

        // Create the folders table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS folders (
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                parent_id TEXT,
                created_at TEXT NOT NULL,
                FOREIGN KEY (parent_id) REFERENCES folders(id) ON DELETE CASCADE
            )",
            [],
        )?;

        // Create the notes table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS notes (
                id TEXT PRIMARY KEY NOT NULL,
                title TEXT NOT NULL DEFAULT '',
                content TEXT NOT NULL DEFAULT '',
                folder_id TEXT,
                updated_at TEXT NOT NULL,
                is_canvas INTEGER NOT NULL DEFAULT 0,
                FOREIGN KEY (folder_id) REFERENCES folders(id) ON DELETE SET NULL
            )",
            [],
        )?;

        // Create indexes for common queries
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_notes_folder_id ON notes(folder_id)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_notes_updated_at ON notes(updated_at DESC)",
            [],
        )?;

        Ok(Database {
            conn: Mutex::new(conn),
        })
    }

    /// Get all notes from the database
    pub fn get_all_notes(&self) -> SqliteResult<Vec<Note>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, title, content, folder_id, updated_at, is_canvas 
             FROM notes 
             ORDER BY updated_at DESC"
        )?;

        let notes_iter = stmt.query_map([], |row| {
            Ok(Note {
                id: row.get(0)?,
                title: row.get(1)?,
                content: row.get(2)?,
                folder_id: row.get(3)?,
                updated_at: row.get(4)?,
                is_canvas: row.get::<_, i32>(5)? != 0,
            })
        })?;

        let mut notes = Vec::new();
        for note in notes_iter {
            notes.push(note?);
        }

        Ok(notes)
    }

    /// Save a note (insert or update)
    pub fn save_note(&self, input: NoteInput) -> SqliteResult<Note> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();

        let id = input.id.unwrap_or_else(|| Uuid::new_v4().to_string());

        conn.execute(
            "INSERT INTO notes (id, title, content, folder_id, updated_at, is_canvas)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                content = excluded.content,
                folder_id = excluded.folder_id,
                updated_at = excluded.updated_at,
                is_canvas = excluded.is_canvas",
            params![
                &id,
                &input.title,
                &input.content,
                &input.folder_id,
                &now,
                input.is_canvas as i32,
            ],
        )?;

        Ok(Note {
            id,
            title: input.title,
            content: input.content,
            folder_id: input.folder_id,
            updated_at: now,
            is_canvas: input.is_canvas,
        })
    }

    /// Delete a note by ID
    pub fn delete_note(&self, id: &str) -> SqliteResult<bool> {
        let conn = self.conn.lock().unwrap();
        let rows_affected = conn.execute("DELETE FROM notes WHERE id = ?1", params![id])?;
        Ok(rows_affected > 0)
    }

    /// Get a single note by ID
    pub fn get_note_by_id(&self, id: &str) -> SqliteResult<Option<Note>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, title, content, folder_id, updated_at, is_canvas 
             FROM notes 
             WHERE id = ?1"
        )?;

        let mut rows = stmt.query(params![id])?;

        if let Some(row) = rows.next()? {
            Ok(Some(Note {
                id: row.get(0)?,
                title: row.get(1)?,
                content: row.get(2)?,
                folder_id: row.get(3)?,
                updated_at: row.get(4)?,
                is_canvas: row.get::<_, i32>(5)? != 0,
            }))
        } else {
            Ok(None)
        }
    }

    /// Get notes by folder ID
    pub fn get_notes_by_folder(&self, folder_id: Option<&str>) -> SqliteResult<Vec<Note>> {
        let conn = self.conn.lock().unwrap();
        let mut notes = Vec::new();

        let row_to_note = |row: &rusqlite::Row| -> SqliteResult<Note> {
            Ok(Note {
                id: row.get(0)?,
                title: row.get(1)?,
                content: row.get(2)?,
                folder_id: row.get(3)?,
                updated_at: row.get(4)?,
                is_canvas: row.get::<_, i32>(5)? != 0,
            })
        };

        match folder_id {
            Some(fid) => {
                let mut stmt = conn.prepare(
                    "SELECT id, title, content, folder_id, updated_at, is_canvas 
                     FROM notes 
                     WHERE folder_id = ?1
                     ORDER BY updated_at DESC"
                )?;
                let rows = stmt.query_map(params![fid], row_to_note)?;
                for row in rows {
                    notes.push(row?);
                }
            }
            None => {
                let mut stmt = conn.prepare(
                    "SELECT id, title, content, folder_id, updated_at, is_canvas 
                     FROM notes 
                     WHERE folder_id IS NULL
                     ORDER BY updated_at DESC"
                )?;
                let rows = stmt.query_map([], row_to_note)?;
                for row in rows {
                    notes.push(row?);
                }
            }
        };

        Ok(notes)
    }

    /// Get all folders
    pub fn get_all_folders(&self) -> SqliteResult<Vec<Folder>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, parent_id, created_at 
             FROM folders 
             ORDER BY name"
        )?;

        let folders = stmt.query_map([], |row| {
            Ok(Folder {
                id: row.get(0)?,
                name: row.get(1)?,
                parent_id: row.get(2)?,
                created_at: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

        Ok(folders)
    }

    /// Get a single folder by ID
    pub fn get_folder_by_id(&self, folder_id: &str) -> SqliteResult<Option<Folder>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, parent_id, created_at 
             FROM folders 
             WHERE id = ?"
        )?;

        let mut rows = stmt.query(params![folder_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Folder {
                id: row.get(0)?,
                name: row.get(1)?,
                parent_id: row.get(2)?,
                created_at: row.get(3)?,
            }))
        } else {
            Ok(None)
        }
    }

    /// Save or update a folder
    pub fn save_folder(&self, input: FolderInput) -> SqliteResult<Folder> {
        let conn = self.conn.lock().unwrap();
        let id = input.id.unwrap_or_else(|| Uuid::new_v4().to_string());
        let created_at = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO folders (id, name, parent_id, created_at) 
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET 
                name = excluded.name,
                parent_id = excluded.parent_id",
            params![id, input.name, input.parent_id, created_at],
        )?;

        Ok(Folder {
            id: id.clone(),
            name: input.name,
            parent_id: input.parent_id,
            created_at,
        })
    }

    /// Delete a folder by ID
    pub fn delete_folder(&self, folder_id: &str) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM folders WHERE id = ?", params![folder_id])?;
        Ok(())
    }

    /// Get all child folders of a parent folder
    pub fn get_folders_by_parent(&self, parent_id: Option<&str>) -> SqliteResult<Vec<Folder>> {
        let conn = self.conn.lock().unwrap();
        
        let mut folders = Vec::new();
        
        match parent_id {
            Some(pid) => {
                let mut stmt = conn.prepare(
                    "SELECT id, name, parent_id, created_at 
                     FROM folders 
                     WHERE parent_id = ?
                     ORDER BY name"
                )?;
                let rows = stmt.query_map(params![pid], |row| {
                    Ok(Folder {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        parent_id: row.get(2)?,
                        created_at: row.get(3)?,
                    })
                })?;
                for row in rows {
                    folders.push(row?);
                }
            }
            None => {
                let mut stmt = conn.prepare(
                    "SELECT id, name, parent_id, created_at 
                     FROM folders 
                     WHERE parent_id IS NULL
                     ORDER BY name"
                )?;
                let rows = stmt.query_map([], |row| {
                    Ok(Folder {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        parent_id: row.get(2)?,
                        created_at: row.get(3)?,
                    })
                })?;
                for row in rows {
                    folders.push(row?);
                }
            }
        };

        Ok(folders)
    }
}

/// Asset management for saving images and files
pub mod assets {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use std::fs;
    use std::path::PathBuf;
    use uuid::Uuid;

    /// Result of saving an asset
    #[derive(Debug, serde::Serialize)]
    pub struct AssetResult {
        pub id: String,
        pub uri: String,
        pub path: String,
    }

    /// Get the assets directory path
    pub fn get_assets_dir(app_data_dir: &PathBuf) -> PathBuf {
        app_data_dir.join(".assets")
    }

    /// Ensure the assets directory exists
    pub fn ensure_assets_dir(app_data_dir: &PathBuf) -> std::io::Result<PathBuf> {
        let assets_dir = get_assets_dir(app_data_dir);
        fs::create_dir_all(&assets_dir)?;
        Ok(assets_dir)
    }

    /// Save a base64-encoded image to the .assets folder
    /// Returns the asset ID and a local URI for the frontend
    pub fn save_image_asset(
        app_data_dir: &PathBuf,
        base64_data: &str,
        file_extension: &str,
    ) -> Result<AssetResult, String> {
        // Ensure assets directory exists
        let assets_dir = ensure_assets_dir(app_data_dir)
            .map_err(|e| format!("Failed to create assets directory: {}", e))?;

        // Generate unique filename
        let asset_id = Uuid::new_v4().to_string();
        let filename = format!("{}.{}", asset_id, file_extension.trim_start_matches('.'));
        let file_path = assets_dir.join(&filename);

        // Decode base64 data (handle data URL prefix if present)
        let clean_base64 = if base64_data.contains(',') {
            base64_data.split(',').nth(1).unwrap_or(base64_data)
        } else {
            base64_data
        };

        let decoded = STANDARD
            .decode(clean_base64)
            .map_err(|e| format!("Failed to decode base64: {}", e))?;

        // Write file to disk
        fs::write(&file_path, &decoded)
            .map_err(|e| format!("Failed to write asset file: {}", e))?;

        // Return the local URI that Tauri can serve
        // Using asset: protocol for Tauri 2.0 compatibility
        let uri = format!("asset://localhost/{}", file_path.to_string_lossy().replace('\\', "/"));

        Ok(AssetResult {
            id: asset_id,
            uri,
            path: file_path.to_string_lossy().to_string(),
        })
    }

    /// Save raw bytes as an image asset
    pub fn save_image_bytes(
        app_data_dir: &PathBuf,
        data: &[u8],
        file_extension: &str,
    ) -> Result<AssetResult, String> {
        let assets_dir = ensure_assets_dir(app_data_dir)
            .map_err(|e| format!("Failed to create assets directory: {}", e))?;

        let asset_id = Uuid::new_v4().to_string();
        let filename = format!("{}.{}", asset_id, file_extension.trim_start_matches('.'));
        let file_path = assets_dir.join(&filename);

        fs::write(&file_path, data)
            .map_err(|e| format!("Failed to write asset file: {}", e))?;

        let uri = format!("asset://localhost/{}", file_path.to_string_lossy().replace('\\', "/"));

        Ok(AssetResult {
            id: asset_id,
            uri,
            path: file_path.to_string_lossy().to_string(),
        })
    }

    /// Delete an asset by its ID
    pub fn delete_asset(app_data_dir: &PathBuf, asset_id: &str) -> Result<bool, String> {
        let assets_dir = get_assets_dir(app_data_dir);

        // Find and delete the asset file (checking common extensions)
        let extensions = ["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp"];

        for ext in &extensions {
            let file_path = assets_dir.join(format!("{}.{}", asset_id, ext));
            if file_path.exists() {
                fs::remove_file(&file_path)
                    .map_err(|e| format!("Failed to delete asset: {}", e))?;
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// List all assets in the .assets folder
    pub fn list_assets(app_data_dir: &PathBuf) -> Result<Vec<AssetResult>, String> {
        let assets_dir = get_assets_dir(app_data_dir);

        if !assets_dir.exists() {
            return Ok(Vec::new());
        }

        let entries = fs::read_dir(&assets_dir)
            .map_err(|e| format!("Failed to read assets directory: {}", e))?;

        let mut assets = Vec::new();

        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
            let path = entry.path();

            if path.is_file() {
                let filename = path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or_default()
                    .to_string();

                let uri = format!("asset://localhost/{}", path.to_string_lossy().replace('\\', "/"));

                assets.push(AssetResult {
                    id: filename,
                    uri,
                    path: path.to_string_lossy().to_string(),
                });
            }
        }

        Ok(assets)
    }
}
