use crate::models::HttpParams;
use crate::config::CONFIG;
use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};
use uuid::Uuid;
use log::error;

pub fn generate_token() -> Uuid {
    Uuid::new_v4()
} 

pub fn split_and_trim(input: &str) -> Vec<String> {
    input.split(',')
        .map(|s| s.trim().to_string())
        .collect()
}

pub fn generate_r_script(token: Uuid, projects: Vec<String>, body: String) -> String {
    format!(
        r#"
        library(DSI)
        library(DSOpal)
        library(dsBaseClient)

        token <- "{}"
        projects <- "{}"

        builder <- DSI::newDSLoginBuilder(.silent = FALSE)
        builder$append(server='DockerOpal', url="https://opal:8443/opal/", token=token, table=projects, driver="OpalDriver", options = "list(ssl_verifyhost=0,ssl_verifypeer=0)")
        
        logindata <- builder$build()
        connections <- DSI::datashield.login(logins = logindata, assign = TRUE, symbol = "D")
        
        {}
        "#,
        token,
        projects.join(","), body
    )
}