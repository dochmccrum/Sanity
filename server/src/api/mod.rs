use axum::{routing::{get, post}, Router};

use crate::AppState;

pub mod auth;
pub mod folders;
pub mod notes;
pub mod sync;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/auth", post(auth::login))
    .route("/notes", get(notes::list_notes).post(notes::save_note))
    .route("/notes/:id", get(notes::get_note).delete(notes::delete_note))
        .route("/folders", get(folders::list_folders).post(folders::save_folder))
        .route("/folders/:id", get(folders::get_folder).delete(folders::delete_folder))
        .route("/sync", post(sync::sync_notes))
}
