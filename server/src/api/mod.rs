use axum::{routing::{get, post}, Router};

use crate::AppState;

pub mod auth;
pub mod folders;
pub mod notes;
pub mod sync;
pub mod sync_crdt;
pub mod sync_folders;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/auth", post(auth::login))
        .route("/notes", get(notes::list_notes).post(notes::save_note))
        .route("/notes/:id", get(notes::get_note).delete(notes::delete_note))
        .route("/folders", get(folders::list_folders).post(folders::save_folder))
        .route("/folders/:id", get(folders::get_folder).delete(folders::delete_folder))
        .route("/sync", post(sync::sync_notes))
        .route("/sync/folders", post(sync_folders::sync_folders))
        // CRDT sync endpoints
        .route("/sync/crdt", post(sync_crdt::sync_crdt))
        .route("/crdt/:note_id", get(sync_crdt::get_crdt_state))
        .route("/ws", get(sync_crdt::ws_handler))
}
