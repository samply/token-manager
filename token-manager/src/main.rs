mod handlers;
mod utils;
mod config;
mod errors;
mod models;
mod routes;

use std::net::SocketAddr;
use axum::Router;
use tracing::{info, Level};
use tracing_subscriber::{fmt::SubscriberBuilder, EnvFilter};
use routes::configure_routes;
use crate::config::CONFIG;

#[tokio::main]
async fn main() {
    let env_filter = EnvFilter::from_default_env().add_directive(Level::INFO.into());
    let subscriber = SubscriberBuilder::default().with_env_filter(env_filter).finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    info!("Starting server token ON!");
    // Create Axum router and configure routes
    let app = Router::new()
        .nest("/api", configure_routes());

    let addr = SocketAddr::new(CONFIG.host, CONFIG.port);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}