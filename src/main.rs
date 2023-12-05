mod config;
mod db;
mod handlers;
mod models;
mod routes;
mod schema;

use crate::config::CONFIG;
use axum::Router;
use routes::configure_routes;
use tracing::{info, Level};
use tracing_subscriber::{fmt::SubscriberBuilder, EnvFilter};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let env_filter = EnvFilter::from_default_env().add_directive(Level::INFO.into());
    let subscriber = SubscriberBuilder::default()
        .with_env_filter(env_filter)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("Starting server token ON!");
    let app = Router::new().nest("/api", configure_routes(db::setup_db()?));

    axum::Server::bind(&CONFIG.addr)
        .serve(app.into_make_service())
        .await?;
    Ok(())
}
