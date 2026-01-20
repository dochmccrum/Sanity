use axum::{extract::State, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

use crate::{db::models::Note, AppState};

#[derive(Debug, Deserialize)]
pub struct SyncRequest {
    pub since: Option<DateTime<Utc>>,
    pub notes: Vec<NoteUpsert>,
}

#[derive(Debug, Deserialize)]
pub struct NoteUpsert {
    pub id: Uuid,
    pub title: String,
    pub content: String,
    pub folder_id: Option<Uuid>,
    pub updated_at: DateTime<Utc>,
    pub is_deleted: bool,
    pub is_canvas: bool,
}

#[derive(Debug, Serialize)]
pub struct SyncResponse {
    pub pulled: Vec<Note>,
    pub last_sync: DateTime<Utc>,
}

pub async fn sync_notes(State(state): State<AppState>, Json(payload): Json<SyncRequest>) -> Result<Json<SyncResponse>, axum::http::StatusCode> {
    let mut tx = state.pool.begin().await.map_err(|err| {
        tracing::error!(?err, "failed to open transaction");
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Collect IDs of notes the client pushed – we'll exclude these from the pull
    // to avoid echoing back exactly what the client sent.
    let pushed_ids: HashSet<Uuid> = payload.notes.iter().map(|n| n.id).collect();

    // Apply incoming changes (upserts) with last-writer-wins semantics
    for note in &payload.notes {
        let res = sqlx::query(
            "INSERT INTO notes (id, title, content, folder_id, updated_at, is_deleted, is_canvas)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT (id) DO UPDATE SET
                title = EXCLUDED.title,
                content = EXCLUDED.content,
                folder_id = EXCLUDED.folder_id,
                updated_at = EXCLUDED.updated_at,
                is_deleted = EXCLUDED.is_deleted,
                is_canvas = EXCLUDED.is_canvas
             WHERE notes.updated_at < EXCLUDED.updated_at",
        )
        .bind(&note.id)
        .bind(&note.title)
        .bind(&note.content)
        .bind(&note.folder_id)
        .bind(note.updated_at)
        .bind(note.is_deleted)
        .bind(note.is_canvas)
        .execute(&mut *tx)
        .await;

        if let Err(err) = res {
            tracing::error!(?err, "failed to upsert note during sync");
            return Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
        }
    }

    // Pull newer changes from server
    let all_pulled = if let Some(since) = payload.since {
        sqlx::query_as::<_, Note>(
            "SELECT id, title, content, folder_id, updated_at, is_deleted, is_canvas FROM notes WHERE updated_at > $1",
        )
        .bind(since)
        .fetch_all(&mut *tx)
        .await
        .map_err(|err| {
            tracing::error!(?err, "failed to pull notes");
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        })?
    } else {
        sqlx::query_as::<_, Note>(
            "SELECT id, title, content, folder_id, updated_at, is_deleted, is_canvas FROM notes",
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(|err| {
            tracing::error!(?err, "failed to pull notes");
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        })?
    };

    // Filter out notes the client just pushed to avoid echoing them back
    let pulled: Vec<Note> = all_pulled
        .into_iter()
        .filter(|n| !pushed_ids.contains(&n.id))
        .collect();

    tx.commit().await.map_err(|err| {
        tracing::error!(?err, "failed to commit sync");
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(SyncResponse {
        pulled,
        last_sync: Utc::now(),
    }))
}
