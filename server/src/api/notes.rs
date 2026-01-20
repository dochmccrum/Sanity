use axum::{extract::{Path, Query, State}, Json};
use serde::Deserialize;
use uuid::Uuid;
use yrs::{Doc, ReadTxn, Transact, StateVector, XmlFragment as XmlFragmentTrait, XmlFragmentRef, XmlTextPrelim, XmlElementPrelim};
use yrs::types::xml::XmlIn;
use yrs::updates::encoder::Encode;

use crate::{db::models::Note, AppState, api::sync_crdt::{WsMessage, NoteMetadata}};

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

#[derive(Debug, Deserialize)]
pub struct FolderQuery {
    pub folder_id: Option<String>,
}

pub async fn list_notes(
    State(state): State<AppState>,
    Query(query): Query<FolderQuery>,
) -> Result<Json<Vec<Note>>, axum::http::StatusCode> {
    let folder_uuid = match query.folder_id.as_deref() {
        Some("") | Some("null") => None,
        Some(value) => match Uuid::parse_str(value) {
            Ok(parsed) => Some(parsed),
            Err(_) => return Err(axum::http::StatusCode::BAD_REQUEST),
        },
        None => None,
    };

    let records = match (query.folder_id.is_some(), folder_uuid) {
        (true, None) => {
            sqlx::query_as::<_, Note>(
                "SELECT id, title, content, folder_id, updated_at, is_deleted, is_canvas FROM notes WHERE folder_id IS NULL AND is_deleted = false ORDER BY updated_at DESC",
            )
            .fetch_all(&state.pool)
            .await
        }
        (true, Some(folder_id)) => {
            sqlx::query_as::<_, Note>(
                "SELECT id, title, content, folder_id, updated_at, is_deleted, is_canvas FROM notes WHERE folder_id = $1 AND is_deleted = false ORDER BY updated_at DESC",
            )
            .bind(folder_id)
            .fetch_all(&state.pool)
            .await
        }
        (false, _) => {
            sqlx::query_as::<_, Note>(
                "SELECT id, title, content, folder_id, updated_at, is_deleted, is_canvas FROM notes WHERE is_deleted = false ORDER BY updated_at DESC",
            )
            .fetch_all(&state.pool)
            .await
        }
    }
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

    // Broadcast metadata update via WebSocket
    if let Some(hub) = &state.sync_hub {
        let meta = NoteMetadata {
            id: record.id,
            title: record.title.clone(),
            content: record.content.clone(),
            folder_id: record.folder_id,
            is_deleted: record.is_deleted,
            is_canvas: record.is_canvas,
            updated_at: record.updated_at,
        };
        if let Ok(payload) = serde_json::to_string(&meta) {
            let _ = hub.broadcast(WsMessage::NoteMetadata { payload }).await;
        }
    }

    // Also create/update CRDT state if content is provided
    // This ensures notes created via the REST API have CRDT states for sync
    if !note.content.is_empty() && !is_canvas {
        // Check if CRDT state already exists
        let existing_crdt: Option<Vec<u8>> = sqlx::query_scalar(
            "SELECT ydoc_state FROM crdt_states WHERE note_id = $1"
        )
        .bind(id)
        .fetch_optional(&state.pool)
        .await
        .unwrap_or(None);

        if existing_crdt.is_none() {
            // Create initial CRDT state from content using XmlFragment
            // This matches the client's Yjs structure (TipTap uses XmlFragment)
            let doc = Doc::new();
            {
                let fragment: XmlFragmentRef = doc.get_or_insert_xml_fragment("content");
                let mut txn = doc.transact_mut();
                // Create a paragraph element with the text content
                // This is a simplified structure - full HTML parsing would be better
                // but the client will sync proper rich text structure on first edit
                let plain_text = html_to_text(&note.content);
                if !plain_text.is_empty() {
                    // Insert a paragraph with text content using the correct API
                    let text_prelim = XmlTextPrelim::new(&plain_text);
                    let p_prelim = XmlElementPrelim::new("paragraph", vec![XmlIn::Text(text_prelim.into())]);
                    fragment.insert(&mut txn, 0, p_prelim);
                }
            }
            let ydoc_state = doc.transact().encode_state_as_update_v1(&StateVector::default());
            let state_vector = doc.transact().state_vector().encode_v1();

            let _ = sqlx::query(
                "INSERT INTO crdt_states (note_id, ydoc_state, state_vector, updated_at)
                 VALUES ($1, $2, $3, now())
                 ON CONFLICT (note_id) DO NOTHING"
            )
            .bind(id)
            .bind(&ydoc_state)
            .bind(&state_vector)
            .execute(&state.pool)
            .await;
        }
    }

    Ok(Json(record))
}

/// Simple HTML to text conversion for initial CRDT seeding
fn html_to_text(html: &str) -> String {
    // Basic HTML tag stripping - a proper implementation would use an HTML parser
    let mut result = html.to_string();
    // Replace common block elements with newlines
    for tag in &["</p>", "</div>", "</h1>", "</h2>", "</h3>", "</h4>", "</h5>", "</h6>", "<br>", "<br/>", "<br />"] {
        result = result.replace(tag, "\n");
    }
    // Remove all remaining HTML tags
    let re = regex::Regex::new(r"<[^>]+>").unwrap();
    result = re.replace_all(&result, "").to_string();
    // Decode common HTML entities
    result = result.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"");
    // Trim excess whitespace
    result.trim().to_string()
}

pub async fn delete_note(State(state): State<AppState>, Path(id): Path<String>) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    let note_id = Uuid::parse_str(&id).map_err(|_| axum::http::StatusCode::BAD_REQUEST)?;
    
    let record = sqlx::query_as::<_, Note>(
        "UPDATE notes SET is_deleted = true, updated_at = now() WHERE id = $1 RETURNING *"
    )
    .bind(note_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|err| {
        tracing::error!(?err, "failed to soft-delete note");
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let note = match record {
        Some(n) => n,
        None => return Err(axum::http::StatusCode::NOT_FOUND),
    };

    // Broadcast deletion via WebSocket
    if let Some(hub) = &state.sync_hub {
        let meta = NoteMetadata {
            id: note.id,
            title: note.title.clone(),
            content: note.content.clone(),
            folder_id: note.folder_id,
            is_deleted: note.is_deleted,
            is_canvas: note.is_canvas,
            updated_at: note.updated_at,
        };
        if let Ok(payload) = serde_json::to_string(&meta) {
            let _ = hub.broadcast(WsMessage::NoteMetadata { payload }).await;
        }
    }

    Ok(Json(serde_json::json!({ "deleted": true })))
}
