use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State, Query,
    },
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Utc};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;
use yrs::{Doc, ReadTxn, Transact, Update, StateVector};
use yrs::updates::decoder::Decode;
use yrs::updates::encoder::Encode;

use crate::AppState;

// ============================================================================
// Types for CRDT Sync
// ============================================================================

/// CRDT state stored in the database
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CrdtState {
    pub note_id: Uuid,
    pub ydoc_state: Vec<u8>,
    pub state_vector: Vec<u8>,
    pub updated_at: DateTime<Utc>,
}

/// Note metadata (non-CRDT fields)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteMetadata {
    pub id: Uuid,
    pub title: String,
    pub content: String,
    pub folder_id: Option<Uuid>,
    pub is_deleted: bool,
    pub is_canvas: bool,
    pub updated_at: DateTime<Utc>,
}

/// CRDT sync request from client
#[derive(Debug, Deserialize)]
pub struct CrdtSyncRequest {
    /// Map of note_id -> base64-encoded state vector
    pub state_vectors: HashMap<String, String>,
    /// Updates to push to server: note_id -> base64-encoded update
    pub updates: HashMap<String, String>,
    /// Note metadata updates
    pub metadata: Vec<NoteMetadata>,
}

/// CRDT sync response to client
#[derive(Debug, Serialize)]
pub struct CrdtSyncResponse {
    /// Updates for each note: note_id -> base64-encoded update diff
    pub updates: HashMap<String, String>,
    /// Metadata for notes that changed
    pub metadata: Vec<NoteMetadata>,
    /// Server timestamp
    pub server_time: DateTime<Utc>,
}

/// WebSocket message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsMessage {
    /// Subscribe to a note's updates
    Subscribe { note_id: String },
    /// Unsubscribe from a note
    Unsubscribe { note_id: String },
    /// Push an update for a note
    Update { note_id: String, payload: String },
    /// Request full sync
    SyncRequest { payload: String },
    /// Sync response from server
    SyncResponse { payload: String },
    /// Note metadata update
    NoteMetadata { payload: String },
    /// Error message
    Error { message: String },
}

/// Query params for WebSocket connection
#[derive(Debug, Deserialize)]
pub struct WsQuery {
    pub token: Option<String>,
}

/// Response for single CRDT state fetch
#[derive(Debug, Serialize)]
pub struct CrdtStateResponse {
    pub note_id: String,
    pub ydoc_state: String,  // base64 encoded
    pub state_vector: String, // base64 encoded
    pub updated_at: DateTime<Utc>,
}

// ============================================================================
// HTTP Endpoint to Get CRDT State for a Single Note
// ============================================================================

pub async fn get_crdt_state(
    State(state): State<AppState>,
    axum::extract::Path(note_id): axum::extract::Path<String>,
) -> Result<Json<Option<CrdtStateResponse>>, axum::http::StatusCode> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    
    let note_uuid: Uuid = note_id.parse().map_err(|_| {
        tracing::error!("invalid note_id: {}", note_id);
        axum::http::StatusCode::BAD_REQUEST
    })?;

    let crdt_state: Option<CrdtState> = sqlx::query_as(
        "SELECT note_id, ydoc_state, state_vector, updated_at FROM crdt_states WHERE note_id = $1"
    )
    .bind(note_uuid)
    .fetch_optional(&state.pool)
    .await
    .map_err(|err| {
        tracing::error!(?err, "failed to fetch crdt state");
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(crdt_state.map(|s| CrdtStateResponse {
        note_id: s.note_id.to_string(),
        ydoc_state: STANDARD.encode(&s.ydoc_state),
        state_vector: STANDARD.encode(&s.state_vector),
        updated_at: s.updated_at,
    })))
}

// ============================================================================
// HTTP Endpoint for CRDT Sync (Fallback/Initial Sync)
// ============================================================================

pub async fn sync_crdt(
    State(state): State<AppState>,
    Json(payload): Json<CrdtSyncRequest>,
) -> Result<Json<CrdtSyncResponse>, axum::http::StatusCode> {
    use base64::{engine::general_purpose::STANDARD, Engine};

    let mut tx = state.pool.begin().await.map_err(|err| {
        tracing::error!(?err, "failed to open transaction");
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let mut response_updates: HashMap<String, String> = HashMap::new();
    let mut response_metadata: Vec<NoteMetadata> = Vec::new();

    // Process incoming updates from the client
    for (note_id_str, base64_update) in &payload.updates {
        let note_id: Uuid = note_id_str.parse().map_err(|_| {
            tracing::error!("invalid note_id: {}", note_id_str);
            axum::http::StatusCode::BAD_REQUEST
        })?;

        let update = STANDARD.decode(base64_update).map_err(|err| {
            tracing::error!(?err, "failed to decode base64 update");
            axum::http::StatusCode::BAD_REQUEST
        })?;

        // Get existing state if any
        let existing: Option<CrdtState> = sqlx::query_as(
            "SELECT note_id, ydoc_state, state_vector, updated_at FROM crdt_states WHERE note_id = $1"
        )
        .bind(note_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|err| {
            tracing::error!(?err, "failed to fetch existing crdt state");
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        })?;

        // Merge or insert the update using yrs
        let doc = Doc::new();
        {
            let mut txn = doc.transact_mut();
            
            // Apply existing state if present
            if let Some(existing_state) = existing {
                if let Ok(update) = Update::decode_v1(&existing_state.ydoc_state) {
                     txn.apply_update(update);
                }
            }
            
            // Apply incoming update
            if let Ok(update) = Update::decode_v1(&update) {
                txn.apply_update(update);
            }
        }

        let new_state = doc.transact().encode_state_as_update_v1(&StateVector::default());
        let state_vector = doc.transact().state_vector().encode_v1();

        sqlx::query(
            "INSERT INTO crdt_states (note_id, ydoc_state, state_vector, updated_at)
             VALUES ($1, $2, $3, now())
             ON CONFLICT (note_id) DO UPDATE SET
                ydoc_state = EXCLUDED.ydoc_state,
                state_vector = EXCLUDED.state_vector,
                updated_at = EXCLUDED.updated_at"
        )
        .bind(note_id)
        .bind(&new_state)
        .bind(&state_vector)
        .execute(&mut *tx)
        .await
        .map_err(|err| {
            tracing::error!(?err, "failed to upsert crdt state");
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        })?;

        // Broadcast update to other connected clients
        if let Some(hub) = &state.sync_hub {
            let _ = hub.broadcast_update(note_id, &update).await;
        }
    }

    // Apply metadata updates
    for meta in &payload.metadata {
          sqlx::query(
                "INSERT INTO notes (id, title, content, folder_id, updated_at, is_deleted, is_canvas)
                 VALUES ($1, $2, $3, $4, $5, $6, $7)
                 ON CONFLICT (id) DO UPDATE SET
                     title = EXCLUDED.title,
                     content = EXCLUDED.content,
                     folder_id = EXCLUDED.folder_id,
                     is_deleted = EXCLUDED.is_deleted,
                     is_canvas = EXCLUDED.is_canvas,
                     updated_at = EXCLUDED.updated_at
                 WHERE notes.updated_at < EXCLUDED.updated_at"
          )
          .bind(meta.id)
          .bind(&meta.title)
          .bind(&meta.content)
          .bind(meta.folder_id)
          .bind(meta.updated_at)
          .bind(meta.is_deleted)
          .bind(meta.is_canvas)
        .execute(&mut *tx)
        .await
        .map_err(|err| {
            tracing::error!(?err, "failed to upsert note metadata");
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        })?;
    }

    // Calculate diffs for each note the client knows about
    for (note_id_str, client_sv_base64) in &payload.state_vectors {
        let note_id: Uuid = match note_id_str.parse() {
            Ok(id) => id,
            Err(_) => continue,
        };

        let client_sv_bytes = match STANDARD.decode(client_sv_base64) {
            Ok(sv) => sv,
            Err(_) => continue,
        };

        // Get server's state for this note
        let server_state: Option<CrdtState> = sqlx::query_as(
            "SELECT note_id, ydoc_state, state_vector, updated_at FROM crdt_states WHERE note_id = $1"
        )
        .bind(note_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|err| {
            tracing::error!(?err, "failed to fetch server crdt state");
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        })?;

        if let Some(state) = server_state {
            // Calculate diff using Yjs
            let doc = Doc::new();
            let mut txn = doc.transact_mut();
            
            if let Ok(update) = Update::decode_v1(&state.ydoc_state) {
                txn.apply_update(update);
                
                if let Ok(remote_sv) = StateVector::decode_v1(&client_sv_bytes) {
                    let diff = txn.encode_diff_v1(&remote_sv);
                    let diff_base64 = STANDARD.encode(&diff);
                    response_updates.insert(note_id_str.clone(), diff_base64);
                }
            }
        }
    }

    // Fetch any new notes the client doesn't have
    let client_note_ids: Vec<Uuid> = payload.state_vectors.keys()
        .filter_map(|s| s.parse().ok())
        .collect();

    let new_notes: Vec<(Uuid, Vec<u8>)> = if client_note_ids.is_empty() {
        // Client has nothing, send all
        sqlx::query_as::<_, (Uuid, Vec<u8>)>(
            "SELECT note_id, ydoc_state FROM crdt_states"
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(|err| {
            tracing::error!(?err, "failed to fetch all crdt states");
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        })?
    } else {
        // Send notes client doesn't have
        sqlx::query_as::<_, (Uuid, Vec<u8>)>(
            "SELECT note_id, ydoc_state FROM crdt_states WHERE note_id != ALL($1)"
        )
        .bind(&client_note_ids)
        .fetch_all(&mut *tx)
        .await
        .map_err(|err| {
            tracing::error!(?err, "failed to fetch new crdt states");
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        })?
    };

    for (note_id, ydoc_state) in new_notes {
        if !response_updates.contains_key(&note_id.to_string()) {
            response_updates.insert(note_id.to_string(), STANDARD.encode(&ydoc_state));
        }
    }

    // Collect all note IDs the client knows about from the incoming metadata
    let client_metadata_ids: Vec<Uuid> = payload.metadata.iter()
        .map(|m| m.id)
        .collect();

    // Fetch metadata for ALL notes the client doesn't have (including those without CRDT states)
    // This ensures new notes created on the server are sent to the client
    let all_server_notes: Vec<NoteMetadata> = if client_metadata_ids.is_empty() {
        // Client has nothing, send all notes (including deletions)
        sqlx::query_as::<_, (Uuid, String, String, Option<Uuid>, bool, bool, DateTime<Utc>)>(
            "SELECT id, title, content, folder_id, is_deleted, is_canvas, updated_at FROM notes"
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(|err| {
            tracing::error!(?err, "failed to fetch all note metadata");
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        })?
        .into_iter()
        .map(|(id, title, content, folder_id, is_deleted, is_canvas, updated_at)| NoteMetadata {
            id, title, content, folder_id, is_deleted, is_canvas, updated_at,
        })
        .collect()
    } else {
        // Send notes the client doesn't have, plus notes with newer metadata
        sqlx::query_as::<_, (Uuid, String, String, Option<Uuid>, bool, bool, DateTime<Utc>)>(
            "SELECT id, title, content, folder_id, is_deleted, is_canvas, updated_at FROM notes"
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(|err| {
            tracing::error!(?err, "failed to fetch note metadata");
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        })?
        .into_iter()
        .map(|(id, title, content, folder_id, is_deleted, is_canvas, updated_at)| NoteMetadata {
            id, title, content, folder_id, is_deleted, is_canvas, updated_at,
        })
        .collect()
    };

    // Filter to notes the client needs:
    // 1. Notes the client doesn't have at all
    // 2. Notes where server has newer metadata
    let client_metadata_map: std::collections::HashMap<Uuid, DateTime<Utc>> = payload.metadata.iter()
        .map(|m| (m.id, m.updated_at))
        .collect();

    for note in all_server_notes {
        let should_include = match client_metadata_map.get(&note.id) {
            None => true, // Client doesn't have this note
            Some(client_updated) => note.updated_at > *client_updated, // Server has newer version
        };
        
        if should_include {
            // If this note has CRDT state but isn't in response_updates yet, add it
            if !response_updates.contains_key(&note.id.to_string()) {
                let crdt_state: Option<Vec<u8>> = sqlx::query_scalar(
                    "SELECT ydoc_state FROM crdt_states WHERE note_id = $1"
                )
                .bind(note.id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|err| {
                    tracing::error!(?err, "failed to fetch crdt state for note");
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR
                })?;
                
                if let Some(state) = crdt_state {
                    response_updates.insert(note.id.to_string(), STANDARD.encode(&state));
                }
            }
            
            response_metadata.push(note);
        }
    }

    tx.commit().await.map_err(|err| {
        tracing::error!(?err, "failed to commit sync transaction");
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(CrdtSyncResponse {
        updates: response_updates,
        metadata: response_metadata,
        server_time: Utc::now(),
    }))
}

// ============================================================================
// WebSocket Handler for Real-time Sync
// ============================================================================

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(query): Query<WsQuery>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    // TODO: Validate JWT token from query.token
    // For now, accept all connections

    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    tracing::info!("ws connection opened");
    let (mut sender, mut receiver) = socket.split();

    // Get or create sync hub
    let hub = match &state.sync_hub {
        Some(h) => h.clone(),
        None => {
            tracing::warn!("sync hub not initialized");
            return;
        }
    };

    // Subscribe to broadcast channel
    let mut broadcast_rx = hub.subscribe();

    // Subscribed notes for this connection
    let subscribed_notes: Arc<RwLock<std::collections::HashSet<Uuid>>> = 
        Arc::new(RwLock::new(std::collections::HashSet::new()));

    // Channel for sending responses from the receiver task to the sender task
    let (response_tx, mut response_rx) = tokio::sync::mpsc::channel::<String>(32);

    let subscribed_notes_clone = subscribed_notes.clone();

    // Spawn task to handle sending (broadcasts + responses)
    let send_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                // Handle broadcast messages
                Ok(msg) = broadcast_rx.recv() => {
                    let should_send = match &msg {
                        WsMessage::Update { note_id, .. } => {
                            if let Ok(uuid) = note_id.parse::<Uuid>() {
                                subscribed_notes_clone.read().await.contains(&uuid)
                            } else {
                                false
                            }
                        },
                        WsMessage::NoteMetadata { .. } => {
                            // Broadcast metadata to everyone so they see new notes or title changes
                            true
                        },
                        _ => false,
                    };

                    if should_send {
                        if let Ok(json) = serde_json::to_string(&msg) {
                            tracing::info!(?json, "sending ws message");
                            if sender.send(Message::Text(json.into())).await.is_err() {
                                break;
                            }
                        }
                    }
                }
                // Handle response messages from the receiver task
                Some(json) = response_rx.recv() => {
                    tracing::info!(?json, "sending ws message");
                    if sender.send(Message::Text(json.into())).await.is_err() {
                        break;
                    }
                }
                else => break,
            }
        }
    });

    // Handle incoming messages
    while let Some(msg) = receiver.next().await {
        let msg = match msg {
            Ok(Message::Text(text)) => text,
            Ok(Message::Close(_)) => break,
            Err(_) => break,
            _ => continue,
        };

        tracing::info!(?msg, "received ws message");

        let ws_msg: WsMessage = match serde_json::from_str(&msg) {
            Ok(m) => m,
            Err(err) => {
                tracing::warn!(?err, "invalid ws message");
                continue;
            }
        };

        match ws_msg {
            WsMessage::Subscribe { note_id } => {
                if let Ok(uuid) = note_id.parse::<Uuid>() {
                    tracing::info!(?uuid, "subscribing to note");
                    subscribed_notes.write().await.insert(uuid);
                }
            }
            WsMessage::Unsubscribe { note_id } => {
                if let Ok(uuid) = note_id.parse::<Uuid>() {
                    tracing::info!(?uuid, "unsubscribing from note");
                    subscribed_notes.write().await.remove(&uuid);
                }
            }
            WsMessage::Update { note_id, payload } => {
                use base64::{engine::general_purpose::STANDARD, Engine};
                if let (Ok(uuid), Ok(update)) = (note_id.parse::<Uuid>(), STANDARD.decode(&payload)) {
                    tracing::info!(?uuid, "received update for note");
                    
                    // Store update in database with a transaction to prevent race conditions
                    let mut tx = match state.pool.begin().await {
                        Ok(t) => t,
                        Err(err) => {
                            tracing::error!(?err, "failed to start transaction for update");
                            continue;
                        }
                    };

                    // Read existing state with FOR UPDATE lock
                    let existing: Option<Vec<u8>> = sqlx::query_scalar(
                        "SELECT ydoc_state FROM crdt_states WHERE note_id = $1 FOR UPDATE"
                    )
                    .bind(uuid)
                    .fetch_optional(&mut *tx)
                    .await
                    .unwrap_or(None);

                    // Merge using yrs
                    let doc = Doc::new();
                    {
                        let mut txn = doc.transact_mut();
                        if let Some(existing_state) = existing {
                             if let Ok(u) = Update::decode_v1(&existing_state) {
                                 txn.apply_update(u);
                             }
                        }
                        if let Ok(u) = Update::decode_v1(&update) {
                            txn.apply_update(u);
                        }
                    }

                    let new_state = doc.transact().encode_state_as_update_v1(&StateVector::default());
                    let state_vector = doc.transact().state_vector().encode_v1();

                    let _ = sqlx::query(
                        "INSERT INTO crdt_states (note_id, ydoc_state, state_vector, updated_at)
                         VALUES ($1, $2, $3, now())
                         ON CONFLICT (note_id) DO UPDATE SET
                            ydoc_state = EXCLUDED.ydoc_state,
                            state_vector = EXCLUDED.state_vector,
                            updated_at = EXCLUDED.updated_at"
                    )
                    .bind(uuid)
                    .bind(&new_state)
                    .bind(&state_vector)
                    .execute(&mut *tx)
                    .await;

                    if let Err(err) = tx.commit().await {
                        tracing::error!(?err, "failed to commit transaction for update");
                        continue;
                    }

                    // Broadcast to other clients
                    tracing::info!(?uuid, "broadcasting update for note");
                    let _ = hub.broadcast(WsMessage::Update { note_id, payload }).await;
                }
            }
            WsMessage::NoteMetadata { payload } => {
                if let Ok(meta) = serde_json::from_str::<NoteMetadata>(&payload) {
                    tracing::info!(?meta.id, "received metadata update");
                    
                    let _ = sqlx::query(
                        "INSERT INTO notes (id, title, content, folder_id, updated_at, is_deleted, is_canvas)
                         VALUES ($1, $2, $3, $4, $5, $6, $7)
                         ON CONFLICT (id) DO UPDATE SET
                             title = EXCLUDED.title,
                             content = EXCLUDED.content,
                             folder_id = EXCLUDED.folder_id,
                             is_deleted = EXCLUDED.is_deleted,
                             is_canvas = EXCLUDED.is_canvas,
                             updated_at = EXCLUDED.updated_at"
                    )
                    .bind(meta.id)
                    .bind(&meta.title)
                    .bind(&meta.content)
                    .bind(meta.folder_id)
                    .bind(meta.updated_at)
                    .bind(meta.is_deleted)
                    .bind(meta.is_canvas)
                    .execute(&state.pool)
                    .await;

                    // Broadcast metadata to other clients
                    let _ = hub.broadcast(WsMessage::NoteMetadata { payload: payload.to_string() }).await;
                }
            }
            WsMessage::SyncRequest { payload } => {
                // Handle full sync request via WebSocket
                if let Ok(request) = serde_json::from_str::<CrdtSyncRequest>(&payload) {
                    tracing::info!(?request, "received sync request");
                    use base64::{engine::general_purpose::STANDARD, Engine};
                    
                    let mut response_updates: HashMap<String, String> = HashMap::new();
                    let mut response_metadata: Vec<NoteMetadata> = Vec::new();
                    
                    // Process incoming updates from the client with a transaction
                    for (note_id_str, base64_update) in &request.updates {
                        if let (Ok(note_id), Ok(update)) = (
                            note_id_str.parse::<Uuid>(),
                            STANDARD.decode(base64_update)
                        ) {
                            let mut tx = match state.pool.begin().await {
                                Ok(t) => t,
                                Err(err) => {
                                    tracing::error!(?err, "failed to start transaction for sync update");
                                    continue;
                                }
                            };

                            // Get existing state with lock
                            let existing: Option<Vec<u8>> = sqlx::query_scalar(
                                "SELECT ydoc_state FROM crdt_states WHERE note_id = $1 FOR UPDATE"
                            )
                            .bind(note_id)
                            .fetch_optional(&mut *tx)
                            .await
                            .unwrap_or(None);

                            // Merge using yrs
                            let doc = Doc::new();
                            {
                                let mut txn = doc.transact_mut();
                                if let Some(existing_state) = existing {
                                    if let Ok(u) = Update::decode_v1(&existing_state) {
                                        txn.apply_update(u);
                                    }
                                }
                                if let Ok(u) = Update::decode_v1(&update) {
                                    txn.apply_update(u);
                                }
                            }

                            let new_state = doc.transact().encode_state_as_update_v1(&StateVector::default());
                            let state_vector = doc.transact().state_vector().encode_v1();

                            let _ = sqlx::query(
                                "INSERT INTO crdt_states (note_id, ydoc_state, state_vector, updated_at)
                                 VALUES ($1, $2, $3, now())
                                 ON CONFLICT (note_id) DO UPDATE SET
                                    ydoc_state = EXCLUDED.ydoc_state,
                                    state_vector = EXCLUDED.state_vector,
                                    updated_at = EXCLUDED.updated_at"
                            )
                            .bind(note_id)
                            .bind(&new_state)
                            .bind(&state_vector)
                            .execute(&mut *tx)
                            .await;

                            if let Err(err) = tx.commit().await {
                                tracing::error!(?err, "failed to commit transaction for sync update");
                                continue;
                            }

                            // Broadcast to other clients
                            let _ = hub.broadcast_update(note_id, &update).await;
                        }
                    }

                    // Process incoming metadata from the client
                    for meta in &request.metadata {
                        let _ = sqlx::query(
                            "INSERT INTO notes (id, title, content, folder_id, updated_at, is_deleted, is_canvas)
                             VALUES ($1, $2, $3, $4, $5, $6, $7)
                             ON CONFLICT (id) DO UPDATE SET
                                 title = EXCLUDED.title,
                                 content = EXCLUDED.content,
                                 folder_id = EXCLUDED.folder_id,
                                 is_deleted = EXCLUDED.is_deleted,
                                 is_canvas = EXCLUDED.is_canvas,
                                 updated_at = EXCLUDED.updated_at
                             WHERE notes.updated_at < EXCLUDED.updated_at"
                        )
                        .bind(meta.id)
                        .bind(&meta.title)
                        .bind(&meta.content)
                        .bind(meta.folder_id)
                        .bind(meta.updated_at)
                        .bind(meta.is_deleted)
                        .bind(meta.is_canvas)
                        .execute(&state.pool)
                        .await;
                    }

                    // Calculate diffs for notes client knows about
                    for (note_id_str, client_sv_base64) in &request.state_vectors {
                        if let (Ok(note_id), Ok(client_sv_bytes)) = (
                            note_id_str.parse::<Uuid>(),
                            STANDARD.decode(client_sv_base64)
                        ) {
                            let server_state: Option<Vec<u8>> = sqlx::query_scalar(
                                "SELECT ydoc_state FROM crdt_states WHERE note_id = $1"
                            )
                            .bind(note_id)
                            .fetch_optional(&state.pool)
                            .await
                            .unwrap_or(None);

                            if let Some(state_bytes) = server_state {
                                let doc = Doc::new();
                                let mut txn = doc.transact_mut();
                                if let Ok(update) = Update::decode_v1(&state_bytes) {
                                    txn.apply_update(update);
                                    if let Ok(remote_sv) = StateVector::decode_v1(&client_sv_bytes) {
                                        let diff = txn.encode_diff_v1(&remote_sv);
                                        response_updates.insert(note_id_str.clone(), STANDARD.encode(&diff));
                                    }
                                 }
                            }
                        }
                    }

                    // Get notes client doesn't have
                    let client_note_ids: Vec<Uuid> = request.state_vectors.keys()
                        .filter_map(|s| s.parse().ok())
                        .collect();

                    let new_notes: Vec<(Uuid, Vec<u8>)> = if client_note_ids.is_empty() {
                        sqlx::query_as::<_, (Uuid, Vec<u8>)>(
                            "SELECT note_id, ydoc_state FROM crdt_states"
                        )
                        .fetch_all(&state.pool)
                        .await
                        .unwrap_or_default()
                    } else {
                        sqlx::query_as::<_, (Uuid, Vec<u8>)>(
                            "SELECT note_id, ydoc_state FROM crdt_states WHERE note_id != ALL($1)"
                        )
                        .bind(&client_note_ids)
                        .fetch_all(&state.pool)
                        .await
                        .unwrap_or_default()
                    };

                    for (note_id, ydoc_state) in new_notes {
                        if !response_updates.contains_key(&note_id.to_string()) {
                            response_updates.insert(note_id.to_string(), STANDARD.encode(&ydoc_state));
                        }
                    }

                    // Fetch metadata
                    let all_notes: Vec<(Uuid, String, String, Option<Uuid>, bool, bool, DateTime<Utc>)> = 
                        sqlx::query_as(
                            "SELECT id, title, content, folder_id, is_deleted, is_canvas, updated_at FROM notes"
                        )
                        .fetch_all(&state.pool)
                        .await
                        .unwrap_or_default();

                    let client_metadata_map: std::collections::HashMap<Uuid, DateTime<Utc>> = request.metadata.iter()
                        .map(|m| (m.id, m.updated_at))
                        .collect();

                    for (id, title, content, folder_id, is_deleted, is_canvas, updated_at) in all_notes {
                        let should_include = match client_metadata_map.get(&id) {
                            None => true,
                            Some(client_updated) => updated_at > *client_updated,
                        };
                        
                        if should_include {
                            response_metadata.push(NoteMetadata {
                                id, title, content, folder_id, is_deleted, is_canvas, updated_at,
                            });
                        }
                    }

                    // Send sync response via the response channel
                    let response = CrdtSyncResponse {
                        updates: response_updates,
                        metadata: response_metadata,
                        server_time: Utc::now(),
                    };

                    if let Ok(json) = serde_json::to_string(&WsMessage::SyncResponse {
                        payload: serde_json::to_string(&response).unwrap_or_default(),
                    }) {
                        let _ = response_tx.send(json).await;
                        tracing::info!("ws sync request processed with {} updates", response.updates.len());
                    }
                }
            }
            _ => {}
        }
    }

    // Cleanup
    tracing::info!("ws connection closed");
    send_task.abort();
}

// ============================================================================
// Sync Hub for Managing WebSocket Connections
// ============================================================================

/// Hub for broadcasting CRDT updates to connected clients
#[derive(Clone)]
pub struct SyncHub {
    /// Broadcast channel for updates
    tx: broadcast::Sender<WsMessage>,
}

impl SyncHub {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1024);
        Self { tx }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<WsMessage> {
        self.tx.subscribe()
    }

    pub async fn broadcast(&self, msg: WsMessage) -> Result<(), broadcast::error::SendError<WsMessage>> {
        self.tx.send(msg)?;
        Ok(())
    }

    pub async fn broadcast_update(&self, note_id: Uuid, update: &[u8]) -> Result<(), broadcast::error::SendError<WsMessage>> {
        use base64::{engine::general_purpose::STANDARD, Engine};
        let msg = WsMessage::Update {
            note_id: note_id.to_string(),
            payload: STANDARD.encode(update),
        };
        self.tx.send(msg)?;
        Ok(())
    }
}

impl Default for SyncHub {
    fn default() -> Self {
        Self::new()
    }
}
