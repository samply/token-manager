use clap::Parser;
use once_cell::sync::Lazy;
use std::net::SocketAddr;

pub(crate) static CONFIG: Lazy<Config> = Lazy::new(Config::parse);

#[derive(Debug, Parser)]
pub struct Config {
    #[clap(long, env)]
    pub opal_api_url: String,

    #[clap(long, env)]
    pub from_email: String,

    #[clap(long, env)]
    pub pwd_email: String,

    #[clap(long, env, default_value = "0.0.0.0:3030")]
    pub addr: SocketAddr,

    #[clap(long, env)]
    pub token_manager_db_url: String,
}
