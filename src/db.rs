use crate::config::CONFIG;
use crate::models::{NewToken, ScriptParams, TokenManager};
use axum::async_trait;
use axum::extract::{FromRequestParts, FromRef};
use axum::http::request::Parts;
use axum::{extract::Query, http::StatusCode, Json};
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use serde_json::json;
use tracing::{error, warn};
use tracing::info;

const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

pub fn setup_db() -> anyhow::Result<Pool<ConnectionManager<SqliteConnection>>> {
    let pool = Pool::new(ConnectionManager::<SqliteConnection>::new(
        &CONFIG.token_manager_db_url,
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

    pub async fn check_project_status(
        &mut self,
        project: String,
    ) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
        use crate::schema::tokens::dsl::*;

        match tokens
            .filter(project_id.eq(&project))
            .select(TokenManager::as_select())
            .load::<TokenManager>(&mut self.0)
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

    pub async fn generate_user_script(
        &mut self,
        query: ScriptParams,
    ) -> Result<String, String> {
        use crate::schema::tokens::dsl::*;

        let records = tokens
            .filter(project_id.eq(&query.project)) // Match project_id from the query parameters
            .filter(user_id.eq(&query.user)) // Match user_id from the query parameters
            .select(TokenManager::as_select())
            .load::<TokenManager>(&mut self.0);

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
                    Ok("No records found for the given project and user.".into())
                }
            }
            Err(err) => {
                error!("Error loading records: {}", err);
                Err(format!("Error loading records: {}", err))
            }
        }
    }
}

fn generate_r_script(script_lines: Vec<String>) -> String {
    let mut builder_script = String::from(
"library(DSI)
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
"logindata <- builder$build()
connections <- DSI::datashield.login(logins = logindata, assign = TRUE, symbol = 'D')\n",
    );

    builder_script
}
