use std::{env, net::SocketAddr, path::PathBuf, sync::Arc};

use axum::{
    routing::{get, post},
    Router,
};
use tower_http::{
    cors::CorsLayer,
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};

mod api;
mod auth;
mod db;

use api::sync_crdt::SyncHub;

#[derive(Clone)]
pub struct AppState {
    pub pool: sqlx::PgPool,
    pub jwt_secret: Arc<String>,
    pub static_dir: Arc<PathBuf>,
    pub index_html: Arc<PathBuf>,
    pub sync_hub: Option<Arc<SyncHub>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL is required");
    let jwt_secret = env::var("JWT_SECRET").unwrap_or_else(|_| "dev-secret-change-me".into());
    let static_dir = env::var("STATIC_DIR").unwrap_or_else(|_| "./static".into());
    let static_dir_path = PathBuf::from(&static_dir);
    let index_html_path = static_dir_path.join("index.html");

    let pool = db::connect_pool(&database_url).await?;

    // Run migrations on startup to ensure schema is present
    sqlx::migrate!("./migrations").run(&pool).await?;

    // Initialize the sync hub for WebSocket real-time sync
    let sync_hub = Arc::new(SyncHub::new());

    let state = AppState {
        pool,
        jwt_secret: Arc::new(jwt_secret),
        static_dir: Arc::new(static_dir_path.clone()),
        index_html: Arc::new(index_html_path.clone()),
        sync_hub: Some(sync_hub),
    };

    let serve_dir = ServeDir::new(static_dir_path)
        .not_found_service(ServeFile::new(index_html_path));

    let app = Router::new()
        .nest("/api", api::router())
        .fallback_service(serve_dir)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    tracing::info!(?addr, "listening");
    axum::serve(tokio::net::TcpListener::bind(addr).await?, app).await?;
    Ok(())
}
