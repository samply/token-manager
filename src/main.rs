mod config;
mod db;
mod enums;
mod handlers;
mod models;
mod routes;
mod schema;
mod utils;

use crate::config::CONFIG;
use axum::Router;
use routes::configure_routes;
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::{fmt::SubscriberBuilder, EnvFilter};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let env_filter = EnvFilter::try_new(&CONFIG.rust_log)
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap();

    let subscriber = SubscriberBuilder::default()
        .with_env_filter(env_filter)
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;

    info!("Starting server token ON!");
    let app = Router::new().nest("/api", configure_routes(db::setup_db()?));

    axum::serve(TcpListener::bind(&CONFIG.addr).await?, app.into_make_service())
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c().await.unwrap();
        })
        .await
        .map_err(Into::into)
}
