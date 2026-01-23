use rusqlite::{params, Connection, OptionalExtension, Result as SqliteResult};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use uuid::Uuid;

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

/// Represents a note in the database
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Note {
    pub id: String,
    pub title: String,
    pub content: String,
    pub folder_id: Option<String>,
    pub updated_at: String,
    pub is_deleted: bool,
    pub is_canvas: bool,
}

/// Represents a note summary (without content) for lists
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NoteSummary {
    pub id: String,
    pub title: String,
    pub folder_id: Option<String>,
    pub updated_at: String,
    pub is_deleted: bool,
    pub is_canvas: bool,
}

/// Represents a folder in the database
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Folder {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub is_deleted: bool,
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
    pub updated_at: Option<String>,
    pub is_deleted: bool,
    pub is_canvas: bool,
}

/// CRDT state for a note (Yjs document binary)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CrdtState {
    pub note_id: String,
    pub ydoc_state: Vec<u8>,   // Full Yjs document state
    pub state_vector: Vec<u8>, // State vector for sync
    pub updated_at: String,
}

/// Input for saving CRDT state
#[derive(Debug, Serialize, Deserialize)]
pub struct CrdtStateInput {
    pub note_id: String,
    pub ydoc_state: Vec<u8>,
    pub state_vector: Vec<u8>,
}

fn note_row_to_note(row: &rusqlite::Row) -> SqliteResult<Note> {
    Ok(Note {
        id: row.get(0)?,
        title: row.get(1)?,
        content: row.get(2)?,
        folder_id: row.get(3)?,
        updated_at: row.get(4)?,
        is_deleted: row.get::<_, i32>(5)? != 0,
        is_canvas: row.get::<_, i32>(6)? != 0,
    })
}

fn ensure_notes_schema(conn: &Connection) -> SqliteResult<()> {
    // Add `is_deleted` for existing installs.
    let mut stmt = conn.prepare("PRAGMA table_info(notes)")?;
    let mut rows = stmt.query([])?;
    let mut has_is_deleted = false;
    while let Some(row) = rows.next()? {
        let col_name: String = row.get(1)?;
        if col_name == "is_deleted" {
            has_is_deleted = true;
            break;
        }
    }

    if !has_is_deleted {
        conn.execute(
            "ALTER TABLE notes ADD COLUMN is_deleted INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_notes_is_deleted ON notes(is_deleted)",
            [],
        )?;
    }

    Ok(())
}

fn ensure_folders_schema(conn: &Connection) -> SqliteResult<()> {
    let mut stmt = conn.prepare("PRAGMA table_info(folders)")?;
    let mut rows = stmt.query([])?;
    let mut has_updated_at = false;
    let mut has_is_deleted = false;

    while let Some(row) = rows.next()? {
        let col_name: String = row.get(1)?;
        if col_name == "updated_at" {
            has_updated_at = true;
        }
        if col_name == "is_deleted" {
            has_is_deleted = true;
        }
    }

    if !has_updated_at {
        conn.execute(
            "ALTER TABLE folders ADD COLUMN updated_at TEXT NOT NULL DEFAULT ''",
            [],
        )?;

        // Backfill for existing rows.
        let now = now_rfc3339();
        conn.execute(
            "UPDATE folders SET updated_at = ?1 WHERE updated_at = ''",
            params![now],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_folders_updated_at ON folders(updated_at DESC)",
            [],
        )?;
    }

    if !has_is_deleted {
        conn.execute(
            "ALTER TABLE folders ADD COLUMN is_deleted INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_folders_is_deleted ON folders(is_deleted)",
            [],
        )?;
    }

    Ok(())
}

fn ensure_crdt_schema(conn: &Connection) -> SqliteResult<()> {
    // Create CRDT state table for Yjs document blobs
    conn.execute(
        "CREATE TABLE IF NOT EXISTS crdt_states (
            note_id TEXT PRIMARY KEY NOT NULL,
            ydoc_state BLOB NOT NULL,
            state_vector BLOB NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY (note_id) REFERENCES notes(id) ON DELETE CASCADE
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_crdt_states_updated_at ON crdt_states(updated_at DESC)",
        [],
    )?;

    Ok(())
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
             PRAGMA synchronous = NORMAL;",
        )?;

        // Create the folders table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS folders (
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                parent_id TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                is_deleted INTEGER NOT NULL DEFAULT 0,
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
                is_deleted INTEGER NOT NULL DEFAULT 0,
                is_canvas INTEGER NOT NULL DEFAULT 0,
                FOREIGN KEY (folder_id) REFERENCES folders(id) ON DELETE SET NULL
            )",
            [],
        )?;

        ensure_notes_schema(&conn)?;
        ensure_folders_schema(&conn)?;
        ensure_crdt_schema(&conn)?;

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
    pub fn get_all_notes(&self) -> SqliteResult<Vec<NoteSummary>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, title, folder_id, updated_at, is_deleted, is_canvas
             FROM notes
             WHERE is_deleted = 0
             ORDER BY updated_at DESC",
        )?;

        let notes_iter = stmt.query_map([], |row| {
            Ok(NoteSummary {
                id: row.get(0)?,
                title: row.get(1)?,
                folder_id: row.get(2)?,
                updated_at: row.get(3)?,
                is_deleted: row.get::<_, i32>(4)? != 0,
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
        let now = now_rfc3339();
        let updated_at = input.updated_at.unwrap_or_else(|| now.clone());

        let id = input.id.unwrap_or_else(|| Uuid::new_v4().to_string());

        conn.execute(
            "INSERT INTO notes (id, title, content, folder_id, updated_at, is_deleted, is_canvas)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                content = excluded.content,
                folder_id = excluded.folder_id,
                updated_at = excluded.updated_at,
                is_deleted = excluded.is_deleted,
                is_canvas = excluded.is_canvas",
            params![
                &id,
                &input.title,
                &input.content,
                &input.folder_id,
                &updated_at,
                input.is_deleted as i32,
                input.is_canvas as i32,
            ],
        )?;

        Ok(Note {
            id,
            title: input.title,
            content: input.content,
            folder_id: input.folder_id,
            updated_at,
            is_deleted: input.is_deleted,
            is_canvas: input.is_canvas,
        })
    }

    /// Delete a note by ID
    pub fn delete_note(&self, id: &str) -> SqliteResult<bool> {
        let conn = self.conn.lock().unwrap();
        let now = now_rfc3339();
        let rows_affected = conn.execute(
            "UPDATE notes SET is_deleted = 1, updated_at = ?2 WHERE id = ?1",
            params![id, now],
        )?;
        Ok(rows_affected > 0)
    }

    /// Move a note to a different folder
    pub fn move_note(&self, id: &str, folder_id: Option<&str>) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        let now = now_rfc3339();
        conn.execute(
            "UPDATE notes SET folder_id = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, folder_id, now],
        )?;
        Ok(())
    }

    /// Get a single note by ID
    pub fn get_note_by_id(&self, id: &str) -> SqliteResult<Option<Note>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, title, content, folder_id, updated_at, is_deleted, is_canvas
             FROM notes
             WHERE id = ?1 AND is_deleted = 0",
        )?;

        let mut rows = stmt.query(params![id])?;

        if let Some(row) = rows.next()? {
            Ok(Some(note_row_to_note(row)?))
        } else {
            Ok(None)
        }
    }

    /// Get notes by folder ID
    pub fn get_notes_by_folder(&self, folder_id: Option<&str>) -> SqliteResult<Vec<NoteSummary>> {
        let conn = self.conn.lock().unwrap();
        let mut notes = Vec::new();

        let row_to_note = |row: &rusqlite::Row| -> SqliteResult<NoteSummary> {
            Ok(NoteSummary {
                id: row.get(0)?,
                title: row.get(1)?,
                folder_id: row.get(2)?,
                updated_at: row.get(3)?,
                is_deleted: row.get::<_, i32>(4)? != 0,
                is_canvas: row.get::<_, i32>(5)? != 0,
            })
        };

        match folder_id {
            Some(fid) => {
                let mut stmt = conn.prepare(
                    "SELECT id, title, folder_id, updated_at, is_deleted, is_canvas
                     FROM notes
                     WHERE folder_id = ?1 AND is_deleted = 0
                     ORDER BY updated_at DESC",
                )?;
                let rows = stmt.query_map(params![fid], row_to_note)?;
                for row in rows {
                    notes.push(row?);
                }
            }
            None => {
                let mut stmt = conn.prepare(
                    "SELECT id, title, folder_id, updated_at, is_deleted, is_canvas
                     FROM notes
                     WHERE folder_id IS NULL AND is_deleted = 0
                     ORDER BY updated_at DESC",
                )?;
                let rows = stmt.query_map([], row_to_note)?;
                for row in rows {
                    notes.push(row?);
                }
            }
        };

        Ok(notes)
    }

    /// Get notes updated since a given timestamp (RFC3339 string). Includes deleted notes.
    pub fn get_notes_updated_since(&self, since: Option<&str>) -> SqliteResult<Vec<Note>> {
        let conn = self.conn.lock().unwrap();
        let mut notes = Vec::new();

        match since {
            Some(since_ts) => {
                let mut stmt = conn.prepare(
                    "SELECT id, title, content, folder_id, updated_at, is_deleted, is_canvas
                     FROM notes
                     WHERE updated_at > ?1
                     ORDER BY updated_at ASC",
                )?;
                let rows = stmt.query_map(params![since_ts], note_row_to_note)?;
                for row in rows {
                    notes.push(row?);
                }
            }
            None => {
                let mut stmt = conn.prepare(
                    "SELECT id, title, content, folder_id, updated_at, is_deleted, is_canvas
                     FROM notes
                     ORDER BY updated_at ASC",
                )?;
                let rows = stmt.query_map([], note_row_to_note)?;
                for row in rows {
                    notes.push(row?);
                }
            }
        }

        Ok(notes)
    }

    /// Apply notes from a remote sync. Uses last-writer-wins based on updated_at.
    pub fn apply_sync_notes(&self, notes: Vec<Note>) -> SqliteResult<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        for note in notes {
            let mut folder_id = note.folder_id;
            if let Some(ref fid) = folder_id {
                let exists: Option<i32> = tx
                    .query_row(
                        "SELECT 1 FROM folders WHERE id = ?1 AND is_deleted = 0 LIMIT 1",
                        params![fid],
                        |row| row.get(0),
                    )
                    .optional()?;
                if exists.is_none() {
                    folder_id = None;
                }
            }

            tx.execute(
                "INSERT INTO notes (id, title, content, folder_id, updated_at, is_deleted, is_canvas)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(id) DO UPDATE SET
                    title = excluded.title,
                    content = excluded.content,
                    folder_id = excluded.folder_id,
                    updated_at = excluded.updated_at,
                    is_deleted = excluded.is_deleted,
                    is_canvas = excluded.is_canvas
                 WHERE excluded.updated_at > notes.updated_at",
                params![
                    note.id,
                    note.title,
                    note.content,
                    folder_id,
                    note.updated_at,
                    note.is_deleted as i32,
                    note.is_canvas as i32,
                ],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    /// Get all folders
    pub fn get_all_folders(&self) -> SqliteResult<Vec<Folder>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, parent_id, created_at, updated_at, is_deleted
             FROM folders
             WHERE is_deleted = 0
             ORDER BY name",
        )?;

        let folders = stmt
            .query_map([], |row| {
                Ok(Folder {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    parent_id: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                    is_deleted: row.get::<_, i32>(5)? != 0,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(folders)
    }

    /// Get a single folder by ID
    pub fn get_folder_by_id(&self, folder_id: &str) -> SqliteResult<Option<Folder>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, parent_id, created_at, updated_at, is_deleted
             FROM folders
             WHERE id = ?",
        )?;

        let mut rows = stmt.query(params![folder_id])?;
        if let Some(row) = rows.next()? {
            let folder = Folder {
                id: row.get(0)?,
                name: row.get(1)?,
                parent_id: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
                is_deleted: row.get::<_, i32>(5)? != 0,
            };
            if folder.is_deleted {
                Ok(None)
            } else {
                Ok(Some(folder))
            }
        } else {
            Ok(None)
        }
    }

    /// Save or update a folder
    pub fn save_folder(&self, input: FolderInput) -> SqliteResult<Folder> {
        let conn = self.conn.lock().unwrap();
        let id = input.id.unwrap_or_else(|| Uuid::new_v4().to_string());
        let now = now_rfc3339();

        conn.execute(
            "INSERT INTO folders (id, name, parent_id, created_at, updated_at, is_deleted)
             VALUES (?1, ?2, ?3, ?4, ?5, 0)
             ON CONFLICT(id) DO UPDATE SET 
                name = excluded.name,
                parent_id = excluded.parent_id,
                updated_at = excluded.updated_at,
                is_deleted = 0",
            params![id, input.name, input.parent_id, now, now],
        )?;

        // Return the canonical row (preserves existing created_at).
        let mut stmt = conn.prepare(
            "SELECT id, name, parent_id, created_at, updated_at, is_deleted
             FROM folders
             WHERE id = ?1",
        )?;
        let folder = stmt.query_row(params![id], |row| {
            Ok(Folder {
                id: row.get(0)?,
                name: row.get(1)?,
                parent_id: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
                is_deleted: row.get::<_, i32>(5)? != 0,
            })
        })?;

        Ok(folder)
    }

    /// Delete a folder by ID
    pub fn delete_folder(&self, folder_id: &str) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();

        // Soft-delete folder and descendants.
        let now = now_rfc3339();
        conn.execute(
            "WITH RECURSIVE descendants(id) AS (
                SELECT id FROM folders WHERE id = ?1
                UNION ALL
                SELECT f.id FROM folders f
                JOIN descendants d ON f.parent_id = d.id
            )
            UPDATE folders
            SET is_deleted = 1, updated_at = ?2
            WHERE id IN (SELECT id FROM descendants)",
            params![folder_id, &now],
        )?;

        // ALSO Soft-delete all notes in these folders
        conn.execute(
            "WITH RECURSIVE descendants(id) AS (
                SELECT id FROM folders WHERE id = ?1
                UNION ALL
                SELECT f.id FROM folders f
                JOIN descendants d ON f.parent_id = d.id
            )
            UPDATE notes
            SET is_deleted = 1, updated_at = ?2
            WHERE folder_id IN (SELECT id FROM descendants)",
            params![folder_id, &now],
        )?;

        Ok(())
    }

    /// Get all child folders of a parent folder
    pub fn get_folders_by_parent(&self, parent_id: Option<&str>) -> SqliteResult<Vec<Folder>> {
        let conn = self.conn.lock().unwrap();

        let mut folders = Vec::new();

        match parent_id {
            Some(pid) => {
                let mut stmt = conn.prepare(
                    "SELECT id, name, parent_id, created_at, updated_at, is_deleted
                     FROM folders
                     WHERE parent_id = ? AND is_deleted = 0
                     ORDER BY name",
                )?;
                let rows = stmt.query_map(params![pid], |row| {
                    Ok(Folder {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        parent_id: row.get(2)?,
                        created_at: row.get(3)?,
                        updated_at: row.get(4)?,
                        is_deleted: row.get::<_, i32>(5)? != 0,
                    })
                })?;
                for row in rows {
                    folders.push(row?);
                }
            }
            None => {
                let mut stmt = conn.prepare(
                    "SELECT id, name, parent_id, created_at, updated_at, is_deleted
                     FROM folders
                     WHERE parent_id IS NULL AND is_deleted = 0
                     ORDER BY name",
                )?;
                let rows = stmt.query_map([], |row| {
                    Ok(Folder {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        parent_id: row.get(2)?,
                        created_at: row.get(3)?,
                        updated_at: row.get(4)?,
                        is_deleted: row.get::<_, i32>(5)? != 0,
                    })
                })?;
                for row in rows {
                    folders.push(row?);
                }
            }
        };

        Ok(folders)
    }

    /// Get folders updated since a given timestamp (RFC3339 string). Includes deleted folders.
    pub fn get_folders_updated_since(&self, since: Option<&str>) -> SqliteResult<Vec<Folder>> {
        let conn = self.conn.lock().unwrap();
        let mut folders = Vec::new();

        match since {
            Some(since_ts) => {
                let mut stmt = conn.prepare(
                    "SELECT id, name, parent_id, created_at, updated_at, is_deleted
                     FROM folders
                     WHERE updated_at > ?1
                     ORDER BY updated_at ASC",
                )?;
                let rows = stmt.query_map(params![since_ts], |row| {
                    Ok(Folder {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        parent_id: row.get(2)?,
                        created_at: row.get(3)?,
                        updated_at: row.get(4)?,
                        is_deleted: row.get::<_, i32>(5)? != 0,
                    })
                })?;
                for row in rows {
                    folders.push(row?);
                }
            }
            None => {
                let mut stmt = conn.prepare(
                    "SELECT id, name, parent_id, created_at, updated_at, is_deleted
                     FROM folders
                     ORDER BY updated_at ASC",
                )?;
                let rows = stmt.query_map([], |row| {
                    Ok(Folder {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        parent_id: row.get(2)?,
                        created_at: row.get(3)?,
                        updated_at: row.get(4)?,
                        is_deleted: row.get::<_, i32>(5)? != 0,
                    })
                })?;
                for row in rows {
                    folders.push(row?);
                }
            }
        }

        Ok(folders)
    }

    /// Apply folders pulled from a remote sync. Uses last-writer-wins based on updated_at.
    pub fn apply_sync_folders(&self, folders: Vec<Folder>) -> SqliteResult<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        for folder in folders {
            tx.execute(
                "INSERT INTO folders (id, name, parent_id, created_at, updated_at, is_deleted)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    parent_id = excluded.parent_id,
                    updated_at = excluded.updated_at,
                    is_deleted = excluded.is_deleted
                 WHERE excluded.updated_at > folders.updated_at",
                params![
                    folder.id,
                    folder.name,
                    folder.parent_id,
                    folder.created_at,
                    folder.updated_at,
                    folder.is_deleted as i32,
                ],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    // ========================================================================
    // CRDT State Methods for Yjs Sync
    // ========================================================================

    /// Save CRDT state for a note
    pub fn save_crdt_state(&self, input: CrdtStateInput) -> SqliteResult<CrdtState> {
        let conn = self.conn.lock().unwrap();
        let now = now_rfc3339();

        conn.execute(
            "INSERT INTO crdt_states (note_id, ydoc_state, state_vector, updated_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(note_id) DO UPDATE SET
                ydoc_state = excluded.ydoc_state,
                state_vector = excluded.state_vector,
                updated_at = excluded.updated_at",
            params![&input.note_id, &input.ydoc_state, &input.state_vector, &now,],
        )?;

        Ok(CrdtState {
            note_id: input.note_id,
            ydoc_state: input.ydoc_state,
            state_vector: input.state_vector,
            updated_at: now,
        })
    }

    /// Get CRDT state for a note
    pub fn get_crdt_state(&self, note_id: &str) -> SqliteResult<Option<CrdtState>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT note_id, ydoc_state, state_vector, updated_at
             FROM crdt_states
             WHERE note_id = ?1",
        )?;

        let mut rows = stmt.query(params![note_id])?;

        if let Some(row) = rows.next()? {
            Ok(Some(CrdtState {
                note_id: row.get(0)?,
                ydoc_state: row.get(1)?,
                state_vector: row.get(2)?,
                updated_at: row.get(3)?,
            }))
        } else {
            Ok(None)
        }
    }

    /// Get all CRDT states (for full sync)
    pub fn get_all_crdt_states(&self) -> SqliteResult<Vec<CrdtState>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT note_id, ydoc_state, state_vector, updated_at
             FROM crdt_states
             ORDER BY updated_at DESC",
        )?;

        let states = stmt
            .query_map([], |row| {
                Ok(CrdtState {
                    note_id: row.get(0)?,
                    ydoc_state: row.get(1)?,
                    state_vector: row.get(2)?,
                    updated_at: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(states)
    }

    /// Get CRDT states for multiple notes
    pub fn get_crdt_states_for_notes(&self, note_ids: &[String]) -> SqliteResult<Vec<CrdtState>> {
        if note_ids.is_empty() {
            return Ok(Vec::new());
        }

        let conn = self.conn.lock().unwrap();
        let placeholders: Vec<String> = note_ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect();
        let query = format!(
            "SELECT note_id, ydoc_state, state_vector, updated_at
             FROM crdt_states
             WHERE note_id IN ({})",
            placeholders.join(", ")
        );

        let mut stmt = conn.prepare(&query)?;

        // Bind all parameters
        let params: Vec<&dyn rusqlite::ToSql> =
            note_ids.iter().map(|s| s as &dyn rusqlite::ToSql).collect();

        let states = stmt
            .query_map(params.as_slice(), |row| {
                Ok(CrdtState {
                    note_id: row.get(0)?,
                    ydoc_state: row.get(1)?,
                    state_vector: row.get(2)?,
                    updated_at: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(states)
    }

    /// Delete CRDT state for a note
    pub fn delete_crdt_state(&self, note_id: &str) -> SqliteResult<bool> {
        let conn = self.conn.lock().unwrap();
        let rows_affected = conn.execute(
            "DELETE FROM crdt_states WHERE note_id = ?1",
            params![note_id],
        )?;
        Ok(rows_affected > 0)
    }

    /// Get CRDT states updated since a timestamp
    pub fn get_crdt_states_updated_since(
        &self,
        since: Option<&str>,
    ) -> SqliteResult<Vec<CrdtState>> {
        let conn = self.conn.lock().unwrap();
        let mut states = Vec::new();

        match since {
            Some(since_ts) => {
                let mut stmt = conn.prepare(
                    "SELECT note_id, ydoc_state, state_vector, updated_at
                     FROM crdt_states
                     WHERE updated_at > ?1
                     ORDER BY updated_at ASC",
                )?;
                let rows = stmt.query_map(params![since_ts], |row| {
                    Ok(CrdtState {
                        note_id: row.get(0)?,
                        ydoc_state: row.get(1)?,
                        state_vector: row.get(2)?,
                        updated_at: row.get(3)?,
                    })
                })?;
                for row in rows {
                    states.push(row?);
                }
            }
            None => {
                let mut stmt = conn.prepare(
                    "SELECT note_id, ydoc_state, state_vector, updated_at
                     FROM crdt_states
                     ORDER BY updated_at ASC",
                )?;
                let rows = stmt.query_map([], |row| {
                    Ok(CrdtState {
                        note_id: row.get(0)?,
                        ydoc_state: row.get(1)?,
                        state_vector: row.get(2)?,
                        updated_at: row.get(3)?,
                    })
                })?;
                for row in rows {
                    states.push(row?);
                }
            }
        }

        Ok(states)
    }

    /// Apply CRDT update - merge incoming binary update with existing state
    /// This is called when receiving updates from the server
    pub fn apply_crdt_update(&self, note_id: &str, update: &[u8]) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        let now = now_rfc3339();

        // Check if we have existing state
        let existing: Option<Vec<u8>> = conn
            .query_row(
                "SELECT ydoc_state FROM crdt_states WHERE note_id = ?1",
                params![note_id],
                |row| row.get(0),
            )
            .optional()?;

        if existing.is_some() {
            // Just store the update - actual merging happens in the frontend
            // The frontend will load the state, apply the update, and save back
            conn.execute(
                "UPDATE crdt_states SET ydoc_state = ?2, updated_at = ?3 WHERE note_id = ?1",
                params![note_id, update, now],
            )?;
        } else {
            // No existing state, store as new
            conn.execute(
                "INSERT INTO crdt_states (note_id, ydoc_state, state_vector, updated_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![note_id, update, update, now],
            )?;
        }

        Ok(())
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
        let uri = format!(
            "asset://localhost/{}",
            file_path.to_string_lossy().replace('\\', "/")
        );

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

        fs::write(&file_path, data).map_err(|e| format!("Failed to write asset file: {}", e))?;

        let uri = format!(
            "asset://localhost/{}",
            file_path.to_string_lossy().replace('\\', "/")
        );

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
                let filename = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or_default()
                    .to_string();

                let uri = format!(
                    "asset://localhost/{}",
                    path.to_string_lossy().replace('\\', "/")
                );

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
