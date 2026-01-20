use axum::{extract::{Path, Query, State}, Json};
use serde::Deserialize;
use uuid::Uuid;

use crate::{db::models::Folder, AppState};

#[derive(Debug, Deserialize)]
pub struct FolderInput {
    pub id: Option<Uuid>,
    pub name: String,
    pub parent_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct FolderQuery {
    pub parent_id: Option<String>,
}

pub async fn list_folders(
    State(state): State<AppState>,
    Query(query): Query<FolderQuery>,
) -> Result<Json<Vec<Folder>>, axum::http::StatusCode> {
    let parent_uuid = match query.parent_id.as_deref() {
        Some("") | Some("null") => None,
        Some(value) => match Uuid::parse_str(value) {
            Ok(parsed) => Some(parsed),
            Err(_) => return Err(axum::http::StatusCode::BAD_REQUEST),
        },
        None => None,
    };

    let records = match (query.parent_id.is_some(), parent_uuid) {
        (true, None) => {
            sqlx::query_as::<_, Folder>(
                "SELECT id, name, parent_id, created_at, updated_at, is_deleted FROM folders WHERE parent_id IS NULL AND is_deleted = false ORDER BY created_at ASC",
            )
            .fetch_all(&state.pool)
            .await
        }
        (true, Some(parent_id)) => {
            sqlx::query_as::<_, Folder>(
                "SELECT id, name, parent_id, created_at, updated_at, is_deleted FROM folders WHERE parent_id = $1 AND is_deleted = false ORDER BY created_at ASC",
            )
            .bind(parent_id)
            .fetch_all(&state.pool)
            .await
        }
        (false, _) => {
            sqlx::query_as::<_, Folder>(
                "SELECT id, name, parent_id, created_at, updated_at, is_deleted FROM folders WHERE is_deleted = false ORDER BY created_at ASC",
            )
            .fetch_all(&state.pool)
            .await
        }
    }
    .map_err(|err| {
        tracing::error!(?err, "failed to list folders");
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(records))
}

pub async fn get_folder(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Folder>, axum::http::StatusCode> {
    let folder_id = Uuid::parse_str(&id).map_err(|_| axum::http::StatusCode::BAD_REQUEST)?;
    let record = sqlx::query_as::<_, Folder>(
        "SELECT id, name, parent_id, created_at, updated_at, is_deleted FROM folders WHERE id = $1",
    )
    .bind(folder_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|err| {
        tracing::error!(?err, "failed to fetch folder");
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    })?;

    match record {
        Some(folder) if !folder.is_deleted => Ok(Json(folder)),
        Some(_) => Err(axum::http::StatusCode::NOT_FOUND),
        None => Err(axum::http::StatusCode::NOT_FOUND),
    }
}

pub async fn save_folder(
    State(state): State<AppState>,
    Json(folder): Json<FolderInput>,
) -> Result<Json<Folder>, axum::http::StatusCode> {
    let id = folder.id.unwrap_or_else(Uuid::new_v4);

    let record = sqlx::query_as::<_, Folder>(
           "INSERT INTO folders (id, name, parent_id, created_at, updated_at, is_deleted)
            VALUES ($1, $2, $3, now(), now(), false)
            ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name, parent_id = EXCLUDED.parent_id, updated_at = now(), is_deleted = false
            RETURNING id, name, parent_id, created_at, updated_at, is_deleted",
    )
    .bind(id)
    .bind(&folder.name)
    .bind(folder.parent_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|err| {
        tracing::error!(?err, "failed to save folder");
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(record))
}

pub async fn delete_folder(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    let folder_id = Uuid::parse_str(&id).map_err(|_| axum::http::StatusCode::BAD_REQUEST)?;

    // Soft-delete folder and all descendants (so tree stays consistent across sync)
    let result = sqlx::query(
        "WITH RECURSIVE descendants AS (
            SELECT id FROM folders WHERE id = $1
            UNION ALL
            SELECT f.id FROM folders f
            JOIN descendants d ON f.parent_id = d.id
        )
        UPDATE folders
        SET is_deleted = true, updated_at = now()
        WHERE id IN (SELECT id FROM descendants)",
    )
        .bind(folder_id)
        .execute(&state.pool)
        .await
        .map_err(|err| {
            tracing::error!(?err, "failed to delete folder");
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if result.rows_affected() == 0 {
        return Err(axum::http::StatusCode::NOT_FOUND);
    }

    Ok(Json(serde_json::json!({ "deleted": true })))
}
