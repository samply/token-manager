use std::env;
use once_cell::sync::Lazy;
use uuid::Uuid;
use crate::errors::FocusError;
use tracing::{debug, info, warn};

pub(crate) static CONFIG: Lazy<Config> = Lazy::new(|| {
    debug!("Loading config");
    Config::load().unwrap_or_else(|e| {
        eprintln!("Unable to start as there was an error reading the config:\n{}\n\nTerminating -- please double-check your startup parameters with --help and refer to the documentation.", e);
        std::process::exit(1);
    })
});

pub(crate) struct Config {
    pub opal_api_url: String,
    pub from_email: String,
    pub pwd_email: String,
    pub host: String,
    pub port: String,
}

impl Config {
    fn load() -> Result<Self, FocusError> {

        // Retrieve the required environment variables
        let opal_api_url = env::var("OPAL_API_URL").expect("OPAL_API_URL must be set");
        let from_email = env::var("FROM_EMAIL").expect("FROM_EMAIL must be set");
        let pwd_email = env::var("PWD_EMAIL").expect("PWD_EMAIL must be set");
        let host = env::var("HOST").expect("HOST must be set");
        let port = env::var("PORT").expect("PORT must be set");

        let config = Config {
            opal_api_url,
            from_email,
            pwd_email,
            host,
            port
        };
        Ok(config)
    }
}