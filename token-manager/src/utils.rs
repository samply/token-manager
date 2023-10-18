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
        
        "{}"
        "#,
        token,
        projects.join(","), body
    )
}

pub fn send_email(query: &HttpParams, token: &Uuid, list_projects: &Vec<String>, body:&String) -> Result<(), ()> {
    let from_email = CONFIG.from_email.clone(); 
    let pwd_email = CONFIG.pwd_email.clone();  
    
    let r_script = generate_r_script(token.clone(), list_projects.clone(), body.clone());

    let email_builder = Message::builder()
        .from(from_email.parse().unwrap())
        .to(query.email.clone().parse().unwrap())
        .subject("DataSHIELD Token Creation")
        .header(ContentType::TEXT_PLAIN)
        .body(r_script.clone())
        .unwrap();

    // Sets the credentials for the mail server
    let creds = Credentials::new(from_email.to_owned(), pwd_email.to_owned());

    // Open a remote connection to Gmail
    let mailer = SmtpTransport::relay("smtp.gmail.com")
        .unwrap()
        .credentials(creds)
        .build();

    // Send the email using the mailer and email_builder
    match mailer.send(&email_builder) {
        Ok(_) => {
            println!("Email sent successfully!");
            Ok(())
        },
        Err(e) => {
            error!("Failed to send email: {:?}", e);
            Ok(())
        }
    }
}