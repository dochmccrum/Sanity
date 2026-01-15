use axum::{extract::{Path, State}, Json};
use serde::Deserialize;
use uuid::Uuid;

use crate::{db::models::Note, AppState};

#[derive(Debug, Deserialize)]
pub struct NoteInput {
    pub id: Option<Uuid>,
    pub title: String,
    pub content: String,
    pub folder_id: Option<Uuid>,
    pub is_deleted: Option<bool>,
    pub is_canvas: Option<bool>,
    pub updated_at: Option<String>,
}

pub async fn list_notes(State(state): State<AppState>) -> Result<Json<Vec<Note>>, axum::http::StatusCode> {
    let records = sqlx::query_as::<_, Note>(
        "SELECT id, title, content, folder_id, updated_at, is_deleted, is_canvas FROM notes ORDER BY updated_at DESC",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|err| {
        tracing::error!(?err, "failed to list notes");
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(records))
}

pub async fn get_note(State(state): State<AppState>, Path(id): Path<String>) -> Result<Json<Note>, axum::http::StatusCode> {
    let note_id = Uuid::parse_str(&id).map_err(|_| axum::http::StatusCode::BAD_REQUEST)?;
    let record = sqlx::query_as::<_, Note>(
        "SELECT id, title, content, folder_id, updated_at, is_deleted, is_canvas FROM notes WHERE id = $1",
    )
    .bind(note_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|err| {
        tracing::error!(?err, "failed to fetch note");
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    })?;

    match record {
        Some(note) => Ok(Json(note)),
        None => Err(axum::http::StatusCode::NOT_FOUND),
    }
}

pub async fn save_note(State(state): State<AppState>, Json(note): Json<NoteInput>) -> Result<Json<Note>, axum::http::StatusCode> {
    let id = note.id.unwrap_or_else(Uuid::new_v4);
    let is_deleted = note.is_deleted.unwrap_or(false);
    let is_canvas = note.is_canvas.unwrap_or(false);

    let record = sqlx::query_as::<_, Note>(
        "INSERT INTO notes (id, title, content, folder_id, updated_at, is_deleted, is_canvas) VALUES ($1, $2, $3, $4, now(), $5, $6)
         ON CONFLICT (id) DO UPDATE SET title = EXCLUDED.title, content = EXCLUDED.content, folder_id = EXCLUDED.folder_id, updated_at = now(), is_deleted = EXCLUDED.is_deleted, is_canvas = EXCLUDED.is_canvas
         RETURNING id, title, content, folder_id, updated_at, is_deleted, is_canvas",
    )
    .bind(id)
    .bind(&note.title)
    .bind(&note.content)
    .bind(note.folder_id)
    .bind(is_deleted)
    .bind(is_canvas)
    .fetch_one(&state.pool)
    .await
    .map_err(|err| {
        tracing::error!(?err, "failed to save note");
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(record))
}

pub async fn delete_note(State(state): State<AppState>, Path(id): Path<String>) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    let note_id = Uuid::parse_str(&id).map_err(|_| axum::http::StatusCode::BAD_REQUEST)?;
    let result = sqlx::query("UPDATE notes SET is_deleted = true, updated_at = now() WHERE id = $1")
        .bind(note_id)
        .execute(&state.pool)
        .await
        .map_err(|err| {
            tracing::error!(?err, "failed to soft-delete note");
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if result.rows_affected() == 0 {
        return Err(axum::http::StatusCode::NOT_FOUND);
    }

    Ok(Json(serde_json::json!({ "deleted": true })))
}
