use beam_lib::{AppId, BeamClient};
use clap::Parser;
use once_cell::sync::Lazy;
use reqwest::Url;
use std::{convert::Infallible, net::SocketAddr};

pub(crate) static CONFIG: Lazy<Config> = Lazy::new(Config::parse);

#[derive(Debug, Parser)]
pub struct Config {
    #[clap(long, env, default_value = "0.0.0.0:3030")]
    pub addr: SocketAddr,

    /// Url of the local beam proxy which is required to have sockets enabled
    #[clap(long, env)]
    pub beam_url: Url,

    /// Beam api key
    #[clap(env)]
    pub beam_secret: String,

    #[clap(env, default_value = "token-manager")]
    pub opal_beam_name: String,

    /// The app id of this application
    #[clap(long, env, value_parser=|id: &str| Ok::<_, Infallible>(AppId::new_unchecked(id)))]
    pub beam_id: AppId,

    #[clap(long, env, default_value = "./file.db ")]
    pub token_manager_db_path: String,

    #[clap(env, default_value = "0123456789abcdef0123456789ABCDEF")]
    pub token_encrypt_key: String,

    #[clap(long, env, default_value = "info")]
    pub rust_log: String,
}

pub static BEAM_CLIENT: Lazy<BeamClient> = Lazy::new(|| {
    BeamClient::new(
        &CONFIG.beam_id,
        &CONFIG.beam_secret,
        CONFIG.beam_url.clone(),
    )
});
