use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};

use crate::{auth::jwt, AppState};

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
}

pub async fn login(State(state): State<AppState>, Json(payload): Json<LoginRequest>) -> Result<Json<LoginResponse>, axum::http::StatusCode> {
    // TODO: replace with real credential check
    if payload.username.is_empty() {
        return Err(axum::http::StatusCode::UNAUTHORIZED);
    }

    let token = jwt::encode_token(&state.jwt_secret, &payload.username)
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(LoginResponse { token }))
}
