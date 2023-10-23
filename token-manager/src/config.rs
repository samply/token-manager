use once_cell::sync::Lazy;
use std::net::IpAddr;
use clap::Parser;

pub(crate) static CONFIG: Lazy<Config> = Lazy::new(|| Config::parse());

#[derive(Debug, Parser)]
pub struct Config {
    #[clap(long, env)]
    pub opal_api_url: String,

    #[clap(long, env)]
    pub from_email: String,

    #[clap(long, env)]
    pub pwd_email: String,

    #[clap(long, env)]
    pub host: IpAddr,

    #[clap(long, env)]
    pub port: u16
}