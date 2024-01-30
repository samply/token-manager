use crate::config::CONFIG;
use crate::enums::OpalProjectStatusResponse;
use crate::handlers::{fetch_project_tables_request, check_project_status_request};
use crate::models::{NewToken, TokenManager, TokenParams};
use axum::{async_trait, extract::{FromRef, FromRequestParts}, http::{request::Parts, StatusCode}, Json};
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use serde_json::json;
use tracing::{error, warn, info};

const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

pub fn setup_db() -> anyhow::Result<Pool<ConnectionManager<SqliteConnection>>> {
    let pool = Pool::new(ConnectionManager::<SqliteConnection>::new(
        &CONFIG.token_manager_db_path,
    ))?;
    info!("Running migrations");
    pool.get()?.run_pending_migrations(MIGRATIONS).unwrap();
    info!("Migrations complete");
    info!("Database setup complete");
    Ok(pool)
}

pub struct Db(PooledConnection<ConnectionManager<SqliteConnection>>);

#[async_trait]
impl<S> FromRequestParts<S> for Db
where
    Pool<ConnectionManager<SqliteConnection>>: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(_parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let pool: Pool<ConnectionManager<SqliteConnection>> = FromRef::from_ref(state);

        pool.get().map(Self).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
    }
}

impl Db {
    pub fn save_token_db(&mut self, token: NewToken) {
        use crate::schema::tokens;

        match diesel::insert_into(tokens::table)
            .values(&token)
            .on_conflict_do_nothing()
            .execute(&mut self.0)
        {
            Ok(_) => {
                info!("New Token Saved in DB");
            }
            Err(error) => {
                warn!("Error connecting to {}", error);
            }
        }
    }

    pub fn update_token_db(&mut self, token_update: NewToken) {
        use crate::schema::tokens::dsl::*;

        let target = tokens.filter(
            user_id.eq(&token_update.user_id)
                .and(project_id.eq(&token_update.project_id))
                .and(bk.eq(&token_update.bk))
        );
    
        match diesel::update(target)
            .set((
                token.eq(token_update.token),
                token_status.eq("UPDATED"),
                created_at.eq(&token_update.created_at),
            ))
            .execute(&mut self.0)
        {
            Ok(_) => {
                info!("Token updated in DB");
            }
            Err(error) => {
                warn!("Error updating token: {}", error);
            }
        }
    }

    pub fn delete_token_db(&mut self, 
        project: String
    ) {
        use crate::schema::tokens::dsl::*;
    
        let target = tokens.filter(
            project_id.eq(&project)
        );
    
        match diesel::delete(target).execute(&mut self.0) {
            Ok(_) => {
                info!("Project and Tokens deleted from DB");
            }
            Err(error) => {
                warn!("Error deleting token: {}", error);
            }
        }
    }

    pub fn delete_user_db(&mut self, 
        user: String
    ) {
        use crate::schema::tokens::dsl::*;
    
        let target = tokens.filter(
            user_id.eq(&user)
        );
    
        match diesel::delete(target).execute(&mut self.0) {
            Ok(_) => {
                info!("Tokens deleted from DB");
            }
            Err(error) => {
                warn!("Error deleting token: {}", error);
            }
        }
    }

    pub async fn check_project_status(
        &mut self,
        project: String,
        bridgehead: String
    ) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
        use crate::schema::tokens::dsl::*;

        let project_status_result = check_project_status_request(project.clone(), bridgehead.clone()).await;

        let status_result = match project_status_result {
            Ok(OpalProjectStatusResponse::Ok { status }) => status, 
            Ok(OpalProjectStatusResponse::Err { status_code, error }) => {
                let status = StatusCode::from_u16(status_code as u16).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
                return Err((status,  error));
            },
            Err(e) => {
                eprintln!("Error retrieving project status: {:?}", e);
                return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()));
            }
        };
        
        match tokens
            .filter(project_id.eq(&project))
            .filter(bk.eq(&bridgehead))
            .select(TokenManager::as_select())
            .load::<TokenManager>(&mut self.0)
        {
            Ok(records) => {
                if !records.is_empty() {
                    info!("Project found with project_id: {:?}", &records);
                    let record = &records[0];
                    
                    let response = json!({
                            "project_id": record.project_id,
                            "bk": record.bk,
                            "user_id": record.user_id,
                            "created_at": record.created_at,
                            "project_status": status_result,
                            "token_status": record.token_status,
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

    pub async fn generate_user_script(&mut self, query: TokenParams) -> Result<String, String> {
        use crate::schema::tokens::dsl::*;
    
        let tables_result = fetch_project_tables_request(query.clone()).await; 
    
        // Initialize script lines vector
        let mut script_lines = Vec::new();
    
        // First check if tables_result is Ok and records are available
        if let Ok(tables) = tables_result {
            info!("Result from status_project_from_beam: {:?}", tables);
    
            let records = tokens
                .filter(project_id.eq(&query.project_id))
                .filter(user_id.eq(&query.user_id))
                .select(TokenManager::as_select())
                .load::<TokenManager>(&mut self.0);
    
            match records {
                Ok(records) => {
                    if !records.is_empty() {
                        // Nested loop: For each table, loop through each record
                        for record in &records {
                            for table in &tables {
                                script_lines.push(format!(
                                    "builder$append(server='{}', url='https://{}/opal/', token='{}', table='{}', driver='OpalDriver')",
                                    record.bk, record.bk, record.token, table
                                ));
                            }
                        }
                        let script = generate_r_script(script_lines);
                        Ok(script)
                    } else {
                        info!("No records were found");
                        Ok("No records found for the given project and user.".into())
                    }
                }
                Err(err) => {
                    error!("Error loading records: {}", err);
                    Err(format!("Error loading records: {}", err))
                }
            }
        } else {
            if let Err(e) = tables_result {
                info!("Error in status_project_from_beam: {:?}", e);
            }
            Err("Error obtaining table names.".into())
        }
    }
}    

fn generate_r_script(script_lines: Vec<String>) -> String {
    let mut builder_script = String::from(
r#"library(DSI)
library(DSOpal)
library(dsBaseClient)
set_config(use_proxy(url="http://beam-connect", port=8062))
set_config( config( ssl_verifyhost = 0L, ssl_verifypeer = 0L ) )

builder <- DSI::newDSLoginBuilder(.silent = FALSE)
"#,
    );

    // Append each line to the script.
    for line in script_lines {
        builder_script.push_str(&line);
        builder_script.push('\n');
    }

    // Finish the script with the login and assignment commands.
    builder_script.push_str(
        "logindata <- builder$build()
connections <- DSI::datashield.login(logins = logindata, assign = TRUE, symbol = 'D')\n",
    );

    builder_script
}
