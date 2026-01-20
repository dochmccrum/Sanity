use axum::{extract::State, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

use crate::{db::models::Folder, AppState};

#[derive(Debug, Deserialize)]
pub struct SyncFoldersRequest {
    pub since: Option<DateTime<Utc>>,
    pub folders: Vec<FolderUpsert>,
    /// Optional: IDs of all folders the client currently has (for discovering missing folders)
    #[serde(default)]
    pub known_folder_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct FolderUpsert {
    pub id: Uuid,
    pub name: String,
    pub parent_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub is_deleted: bool,
}

#[derive(Debug, Serialize)]
pub struct SyncFoldersResponse {
    pub pulled: Vec<Folder>,
    pub last_sync: DateTime<Utc>,
}

pub async fn sync_folders(
    State(state): State<AppState>,
    Json(payload): Json<SyncFoldersRequest>,
) -> Result<Json<SyncFoldersResponse>, axum::http::StatusCode> {
    tracing::info!(
        since = ?payload.since,
        pushed_count = payload.folders.len(),
        known_folder_ids_count = payload.known_folder_ids.len(),
        "sync_folders request received"
    );
    
    let mut tx = state.pool.begin().await.map_err(|err| {
        tracing::error!(?err, "failed to open transaction");
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Collect IDs of folders the client pushed – we'll exclude these from the pull
    // to avoid echoing back exactly what the client sent.
    let pushed_ids: HashSet<Uuid> = payload.folders.iter().map(|f| f.id).collect();

    // Apply incoming changes (upserts) with last-writer-wins semantics
    for folder in &payload.folders {
        let res = sqlx::query(
            "INSERT INTO folders (id, name, parent_id, created_at, updated_at, is_deleted)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (id) DO UPDATE SET
                name = EXCLUDED.name,
                parent_id = EXCLUDED.parent_id,
                updated_at = EXCLUDED.updated_at,
                is_deleted = EXCLUDED.is_deleted
             WHERE folders.updated_at < EXCLUDED.updated_at",
        )
        .bind(&folder.id)
        .bind(&folder.name)
        .bind(&folder.parent_id)
        .bind(folder.created_at)
        .bind(folder.updated_at)
        .bind(folder.is_deleted)
        .execute(&mut *tx)
        .await;

        if let Err(err) = res {
            tracing::error!(?err, "failed to upsert folder during sync");
            return Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
        }
    }

    // Pull newer changes from server (including deletions)
    // Also include folders the client doesn't have (based on known_folder_ids)
    let all_pulled = if let Some(since) = payload.since {
        // Get folders updated since last sync
        let updated_folders = sqlx::query_as::<_, Folder>(
            "SELECT id, name, parent_id, created_at, updated_at, is_deleted
             FROM folders
             WHERE updated_at > $1",
        )
        .bind(since)
        .fetch_all(&mut *tx)
        .await
        .map_err(|err| {
            tracing::error!(?err, "failed to pull folders");
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        })?;

        // Also get folders the client doesn't know about (if known_folder_ids provided)
        if !payload.known_folder_ids.is_empty() {
            let known_ids: HashSet<Uuid> = payload.known_folder_ids.iter().cloned().collect();
            let all_server_folders = sqlx::query_as::<_, Folder>(
                "SELECT id, name, parent_id, created_at, updated_at, is_deleted
                 FROM folders",
            )
            .fetch_all(&mut *tx)
            .await
            .map_err(|err| {
                tracing::error!(?err, "failed to fetch all folders");
                axum::http::StatusCode::INTERNAL_SERVER_ERROR
            })?;

            // Merge: include updated folders + folders client doesn't have
            let mut result_map: std::collections::HashMap<Uuid, Folder> = updated_folders
                .into_iter()
                .map(|f| (f.id, f))
                .collect();
            
            for folder in all_server_folders {
                if !known_ids.contains(&folder.id) && !result_map.contains_key(&folder.id) {
                    result_map.insert(folder.id, folder);
                }
            }
            
            result_map.into_values().collect()
        } else {
            updated_folders
        }
    } else {
        sqlx::query_as::<_, Folder>(
            "SELECT id, name, parent_id, created_at, updated_at, is_deleted
             FROM folders",
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(|err| {
            tracing::error!(?err, "failed to pull folders");
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        })?
    };

    // Filter out folders the client just pushed to avoid echoing them back
    let pulled: Vec<Folder> = all_pulled
        .into_iter()
        .filter(|f| !pushed_ids.contains(&f.id))
        .collect();

    tracing::info!(
        pulled_count = pulled.len(),
        "sync_folders returning folders"
    );
    for folder in &pulled {
        tracing::debug!(
            folder_id = %folder.id,
            folder_name = %folder.name,
            is_deleted = folder.is_deleted,
            "returning folder"
        );
    }

    tx.commit().await.map_err(|err| {
        tracing::error!(?err, "failed to commit folder sync");
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(SyncFoldersResponse {
        pulled,
        last_sync: Utc::now(),
    }))
}
