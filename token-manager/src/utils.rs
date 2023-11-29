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

pub fn generate_r_script(script_lines: Vec<String>) -> String {
    let mut builder_script = String::from(
        "
        library(DSI)
        library(DSOpal)
        library(dsBaseClient)

        builder <- DSI::newDSLoginBuilder(.silent = FALSE)
        ",
    );

    // Append each line to the script.
    for line in script_lines {
        builder_script.push_str(&line);
        builder_script.push('\n');
    }

    // Finish the script with the login and assignment commands.
    builder_script.push_str(
        "
        logindata <- builder$build()
        connections <- DSI::datashield.login(logins = logindata, assign = TRUE, symbol = 'D')
        ",
    );

    builder_script
}