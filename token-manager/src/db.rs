use crate::config::CONFIG;
use crate::models::{NewToken, ScriptParams, TokenManager};
use crate::utils::generate_r_script;
use axum::{extract::Query, http::StatusCode, Json};
use diesel::prelude::*;
use log::error;
use serde_json::json;
use tracing::info;

pub fn check_db_status() -> Result<SqliteConnection, diesel::ConnectionError> {
    let database_url = &CONFIG.token_manager_db_url;
    SqliteConnection::establish(database_url)
}

pub fn establish_connection() -> SqliteConnection {
    info!(
        "Connecting to database: {}",
        CONFIG.token_manager_db_url.clone()
    );
    let database_url = &CONFIG.token_manager_db_url;
    let connection_result = SqliteConnection::establish(&database_url);

    match connection_result {
        Ok(connection) => {
            // Log success or perform other actions
            info!(
                "Successfully connected to database: {}",
                CONFIG.token_manager_db_url.clone()
            );
            connection
        }
        Err(err) => panic!("Error connecting to {}: {}", database_url, err),
    }
}

pub fn save_token_db(token: NewToken) {
    use crate::schema::tokens;
    let connection = &mut establish_connection();

    match diesel::insert_into(tokens::table)
        .values(&token)
        .execute(connection)
    {
        Ok(_) => {
            info!("New Token Saved in DB");
        }
        Err(error) => {
            info!("Error connecting to {}", error);
        }
    }
}

pub async fn check_project_status(
    project: String,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    use crate::schema::tokens::dsl::*;

    let connection = &mut establish_connection();

    match tokens
        .filter(project_id.eq(&project))
        .select(TokenManager::as_select())
        .load::<TokenManager>(connection)
    {
        Ok(records) => {
            if !records.is_empty() {
                info!("Project found with project_id: {:?}", &records);
                let response = json!({
                    "status": "success",
                    "data": records
                });
                Ok(Json(response))
            } else {
                info!("Project not found with project_id: {}", project);
                let error_response = r#"{
                    "status": "error",
                    "message": "Project not found with project_id"
                }"#;
                Err((StatusCode::NOT_FOUND, error_response.into()))
            }
        }
        Err(err) => {
            error!("Error calling DB: {}", err);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Cannot connect to database".into(),
            ))
        }
    }
}

pub async fn generate_user_script(query: Query<ScriptParams>) -> Result<String, String> {
    use crate::schema::tokens::dsl::*;

    let connection = &mut establish_connection();

    let records = tokens
        .filter(project_id.eq(&query.project)) // Match project_id from the query parameters
        .filter(user_id.eq(&query.user)) // Match user_id from the query parameters
        .select(TokenManager::as_select())
        .load::<TokenManager>(connection);

    match records {
        Ok(records) => {
            let mut script_lines = Vec::new();
            if !records.is_empty() {
                for record in records {
                    info!("Records Extracted: {:?}", record);
                    script_lines.push(format!("builder$append(server='DockerOpal', url='https://{}:8443/opal/', token='{}', table='{}', driver='OpalDriver', options = list(ssl_verifyhost=0,ssl_verifypeer=0))",
                    record.bk , record.token, record.project_id
                    ));
                }

                let script = generate_r_script(script_lines);
                info!("Script Generated: {:?}", script);
                Ok(script)
            } else {
                info!("No records were found");
                return Ok("No records found for the given project and user.".into());
            }
        }
        Err(err) => {
            error!("Error loading records: {}", err);
            Err(format!("Error loading records: {}", err))
        }
    }
}
