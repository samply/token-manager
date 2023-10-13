use std::path::PathBuf;

use beam_lib::AppId;
use clap::Parser;
use http::{HeaderValue, Uri};
use once_cell::sync::Lazy;
use reqwest::{Certificate, Client, Proxy};
use tracing::{debug, info, warn};

use crate::errors::FocusError;


#[derive(clap::ValueEnum, Clone, Debug)]
pub enum Obfuscate {
    No,
    Yes,
}

pub(crate) static CONFIG: Lazy<Config> = Lazy::new(|| {
    debug!("Loading config");
    Config::load().unwrap_or_else(|e| {
        eprintln!("Unable to start as there was an error reading the config:\n{}\n\nTerminating -- please double-check your startup parameters with --help and refer to the documentation.", e);
        std::process::exit(1);
    })
});

const CLAP_FOOTER: &str = "For proxy support, environment variables HTTP_PROXY, HTTPS_PROXY, ALL_PROXY and NO_PROXY (and their lower-case variants) are supported. Usually, you want to set HTTP_PROXY *and* HTTPS_PROXY or set ALL_PROXY if both values are the same.\n\nFor updates and detailed usage instructions, visit https://github.com/samply/focus";

#[derive(Parser, Debug)]
#[clap(
    name("🔭 Focus"),
    version,
    arg_required_else_help(true),
    after_help(CLAP_FOOTER)
)]
struct CliArgs {
    /// The beam proxy's base URL, e.g. https://proxy1.beam.samply.de
    #[clap(long, env, value_parser)]
    beam_proxy_url: Uri,

    /// This application's beam AppId, e.g. focus.proxy1.broker.samply.de
    #[clap(long, env, value_parser)]
    beam_app_id_long: String,

    /// This applications beam API key
    #[clap(long, env, value_parser)]
    api_key: String,

    /// Number of retries for reaching the beam proxy and FHIR server, respectively
    #[clap(long, env, value_parser, default_value = "32")]
    retry_count: usize,

    /// The FHIR servers base URL, e.g. https://blaze.site/fhir
    #[clap(long, env, value_parser)]
    blaze_url: Uri,

    /// Should the results be obfuscated
    #[clap(long, env, value_parser = clap::value_parser!(Obfuscate), default_value = "yes")]
    obfuscate: Obfuscate,

    /// Should zero values be obfuscated - default false
    #[clap(long, env, value_parser)]
    obfuscate_zero: bool,

    /// The mode of obfuscating values below 10: 0 - return zero, 1 - return ten, 2 - obfuscate using Laplace distribution and rounding
    #[clap(long, env, value_parser, default_value = "1")]
    obfuscate_below_10_mode: usize,

    /// Sensitivity parameter for obfuscating the counts in the Patient stratifier
    #[clap(long, env, value_parser, default_value = "1.")]
    delta_patient: f64,

    /// Sensitivity parameter for obfuscating the counts in the Specimen stratifier
    #[clap(long, env, value_parser, default_value = "20.")]
    delta_specimen: f64,

    /// Sensitivity parameter for obfuscating the counts in the Diagnosis stratifier
    #[clap(long, env, value_parser, default_value = "3.")]
    delta_diagnosis: f64,

    /// Privacy budget parameter for obfuscating the counts in the stratifiers
    #[clap(long, env, value_parser, default_value = "0.1")]
    epsilon: f64,

    /// The granularity of the rounding of the obfuscated values
    #[clap(long, env, value_parser, default_value = "10")]
    rounding_step: usize,

    /// The path to the file containing BASE64 encoded queries whose results are to be cached
    #[clap(long, env, value_parser)]
    queries_to_cache_file_path: Option<String>,

    /// Outgoing HTTP proxy: Directory with CA certificates to trust for TLS connections (e.g. /etc/samply/cacerts/)
    #[clap(long, env, value_parser)]
    tls_ca_certificates_dir: Option<PathBuf>,
}

pub(crate) struct Config {
    pub beam_proxy_url: Uri,
    pub beam_app_id_long: AppId,
    pub api_key: String,
    pub retry_count: usize,
    pub blaze_url: Uri,
    pub obfuscate: Obfuscate,
    pub obfuscate_zero: bool,
    pub obfuscate_below_10_mode: usize,
    pub delta_patient: f64,
    pub delta_specimen: f64,
    pub delta_diagnosis: f64,
    pub epsilon: f64,
    pub rounding_step: usize,
    pub queries_to_cache_file_path: Option<String>,
    tls_ca_certificates: Vec<Certificate>,
    pub client: Client,
}

impl Config {
    fn load() -> Result<Self, FocusError> {
        let cli_args = CliArgs::parse();
        info!("Successfully read config and API keys from CLI and secrets files.");
        let tls_ca_certificates_dir = cli_args.tls_ca_certificates_dir;
        let tls_ca_certificates = load_certificates_from_dir(tls_ca_certificates_dir.clone())
            .map_err(|e| {
                FocusError::ConfigurationError(format!(
                    "Unable to read from TLS CA directory: {}",
                    e
                ))
            })?;
        let client = prepare_reqwest_client(&tls_ca_certificates)?;
        let config = Config {
            beam_proxy_url: cli_args.beam_proxy_url,
            beam_app_id_long: AppId::new_unchecked(cli_args.beam_app_id_long),
            api_key: cli_args.api_key,
            retry_count: cli_args.retry_count,
            blaze_url: cli_args.blaze_url,
            obfuscate: cli_args.obfuscate,
            obfuscate_zero: cli_args.obfuscate_zero,
            obfuscate_below_10_mode: cli_args.obfuscate_below_10_mode,
            delta_patient: cli_args.delta_patient,
            delta_specimen: cli_args.delta_specimen,
            delta_diagnosis: cli_args.delta_diagnosis,
            epsilon: cli_args.epsilon,
            rounding_step: cli_args.rounding_step,
            queries_to_cache_file_path: cli_args.queries_to_cache_file_path,
            tls_ca_certificates,
            client,
        };
        Ok(config)
    }
}

pub fn load_certificates_from_dir(
    ca_dir: Option<PathBuf>,
) -> Result<Vec<Certificate>, std::io::Error> {
    let mut result = Vec::new();
    if let Some(ca_dir) = ca_dir {
        for file in ca_dir.read_dir()? {
            //.map_err(|e| SamplyBeamError::ConfigurationFailed(format!("Unable to read from TLS CA directory {}: {}", ca_dir.to_string_lossy(), e)))
            let path = file?.path();
            let content = std::fs::read(&path)?;
            let cert = Certificate::from_pem(&content);
            if let Err(e) = cert {
                warn!(
                    "Unable to read certificate from file {}: {}",
                    path.to_string_lossy(),
                    e
                );
                continue;
            }
            result.push(cert.unwrap());
        }
    }
    Ok(result)
}

pub fn prepare_reqwest_client(certs: &Vec<Certificate>) -> Result<reqwest::Client, FocusError> {
    let mut client = reqwest::Client::builder()
        .tcp_nodelay(true)
        .user_agent(HeaderValue::from_static("SAMPLY_USER_AGENT"));
    for cert in certs {
        client = client.add_root_certificate(cert.to_owned());
    }
    let mut proxies: Vec<Proxy> = Vec::new();
    let no_proxy = reqwest::NoProxy::from_env();
    for var in ["http_proxy", "https_proxy", "all_proxy", "no_proxy"] {
        for (k, v) in std::env::vars().filter(|(k, _)| k.to_lowercase() == var) {
            std::env::set_var(k.to_uppercase(), v.clone());
            match k.as_str() {
                "http_proxy" => proxies.push(
                    Proxy::http(v)
                        .map_err(|e| FocusError::InvalidProxyConfig(e))?
                        .no_proxy(no_proxy.clone()),
                ),
                "https_proxy" => proxies.push(
                    Proxy::https(v)
                        .map_err(|e| FocusError::InvalidProxyConfig(e))?
                        .no_proxy(no_proxy.clone()),
                ),
                "all_proxy" => proxies.push(
                    Proxy::all(v)
                        .map_err(|e| FocusError::InvalidProxyConfig(e))?
                        .no_proxy(no_proxy.clone()),
                ),
                _ => (),
            };
        }
    }
    client
        .build()
        .map_err(|e| FocusError::ConfigurationError(format!("Cannot create http client: {}", e)))
}