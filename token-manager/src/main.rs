mod handlers;
mod utils;
mod config;
mod errors;
mod models;

use std::net::SocketAddr;
use axum::Router;
use tracing::{info, Level};
use tracing_subscriber::{fmt::SubscriberBuilder, EnvFilter};
use handlers::call_opal_api;
use crate::config::CONFIG;

#[tokio::main]
async fn main() {
    let env_filter = EnvFilter::from_default_env().add_directive(Level::INFO.into());
    let subscriber = SubscriberBuilder::default().with_env_filter(env_filter).finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    info!("Starting server ON!");

    let app = Router::new().route("/api/token", axum::routing::post(call_opal_api));

    let addr = SocketAddr::new(CONFIG.host, CONFIG.port);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}