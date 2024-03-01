use axum::{async_trait, extract::{FromRef, FromRequestParts}, http::{request::Parts, StatusCode}, Json};
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use diesel::result::Error;
use serde_json::json;
use anyhow::Result;
use tracing::{error, info, warn};

use crate::config::CONFIG;
use crate::schema::tokens::dsl::*;
use crate::schema::tokens;
use crate::enums::{OpalTokenStatus, OpalProjectStatus};
use crate::handlers::{check_project_status_request, fetch_project_tables_names_request, check_token_status_request};
use crate::models::{NewToken, TokenManager, TokenParams, TokenStatus, TokensQueryParams, ProjectQueryParams};

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
    pub fn save_token_db(&mut self, new_token: NewToken) {

        match diesel::insert_into(tokens::table)
            .values(&new_token)
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


        let target = tokens.filter(
            user_id.eq(&token_update.user_id)
                .and(project_id.eq(&token_update.project_id))
                .and(bk.eq(&token_update.bk))
        );

        match diesel::update(target)
            .set((
                token.eq(token_update.token),
                token_status.eq("UPDATED"),
                token_created_at.eq(&token_update.token_created_at),
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

    pub fn update_token_status_db(&mut self, token_update: TokenStatus) {


        let target = tokens.filter(
            user_id.eq(&token_update.user_id)
                .and(project_id.eq(&token_update.project_id))
                .and(bk.eq(&token_update.bk))
        );

        match diesel::update(target)
            .set((
                token_status.eq(token_update.token_status),
            ))
            .execute(&mut self.0)
        {
            Ok(_) => {
                info!("Token status updated in DB");
            }
            Err(error) => {
                warn!("Error updating token status: {}", error);
            }
        }
    }

    pub fn delete_project_db(&mut self,
                           project: String,
    ) {


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

    pub fn delete_token_db(&mut self,
                          name: String,
    ) {


        let target = tokens.filter(
            token_name.eq(&name)
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

    pub fn get_token_name(&mut self, user: String, project: String) -> Result<Option<String>, Error> {
        tokens
            .filter(user_id.eq(user))
            .filter(project_id.eq(project))
            .select(token_name)
            .first::<String>(&mut self.0)
            .optional() 
    }

    pub fn get_token_value(&mut self, user: String, project: String) -> Result<Option<String>, Error> {
        tokens
            .filter(user_id.eq(user))
            .filter(project_id.eq(project))
            .select(token)
            .first::<String>(&mut self.0)
            .optional() 
    }

    pub fn is_any_token_available(&mut self, params: TokenParams) -> Result<bool, Error> {
        
        let result = tokens
            .filter(user_id.eq(params.user_id))
            .filter(project_id.eq(params.project_id))
            .filter(bk.eq_any(params.bridgehead_ids))
            .first::<TokenManager>(&mut self.0)
            .optional();
    
            match result {
                Ok(Some(_)) => Ok(true), 
                Ok(None) => Ok(false), 
                Err(e) => Err(e),
            }
    }

    pub async fn check_script_status(&mut self, params: TokenParams) -> Result<String, (StatusCode, String)> {
        let token_available = self.is_any_token_available(params);
    
        match token_available {
            Ok(true) => Ok("true".to_string()),
            Ok(false) => Ok("false".to_string()),
            Err(_) => Err((StatusCode::INTERNAL_SERVER_ERROR, "Error checking token availability.".to_string())),
        }
    }

   pub async fn check_token_status(&mut self, params: TokensQueryParams) -> Result<Json<serde_json::Value>, (StatusCode, String)> {

        let mut token_status_json = json!({
            "project_id": params.project_id.clone(),
            "bk": params.bk.clone(),
            "user_id": params.user_id.clone(),
            "token_created_at": "",
            "project_status": OpalTokenStatus::NOTFOUND,
            "token_status": OpalProjectStatus::NOTFOUND,
        });

        if let Ok(json_response) = check_project_status_request(ProjectQueryParams { bk: params.bk.clone(), project_id: params.project_id.clone()}).await {
            token_status_json["project_status"] = json_response.0["project_status"].clone();
        } else {
            error!("Error retrieving project status");
        }

        let token_name_response = match self.get_token_name(params.user_id.clone(), params.project_id.clone()) {
            Ok(Some(name)) => name,
            Ok(None) => return Ok(Json(token_status_json)),
            Err(e) => return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
        };

        let records = match tokens
            .filter(user_id.eq(&params.user_id))
            .filter(bk.eq(&params.bk))
            .filter(project_id.eq(&params.project_id))
            .select(TokenManager::as_select())
            .load::<TokenManager>(&mut self.0)
        {
            Ok(records) if !records.is_empty() => records,
            Ok(_) => {
                info!("Token not found with user_id: {}", &params.user_id);
                return Ok(Json(token_status_json)); 
            }
            Err(err) => {
                error!("Error calling DB: {}", err);
                return Err((StatusCode::INTERNAL_SERVER_ERROR, err.to_string()));
            }
        };
        
        let record = &records[0];
        token_status_json["token_created_at"] = json!(record.token_created_at);
        let token_value = json!(record.token).as_str().unwrap_or_default().to_string();

        if let Ok(json_response) = check_token_status_request(params.user_id.clone(),  params.bk.clone(), params.project_id.clone(), token_name_response.clone(), token_value.clone()).await {
            token_status_json["token_status"] = json_response.0["token_status"].clone();
            
            let new_token_status = TokenStatus {
                project_id: &params.project_id.clone(),
                bk: &params.bk.clone(),
                token_status: OpalTokenStatus::CREATED.as_str(),
                user_id: &params.user_id.clone(),
            };
            self.update_token_status_db(new_token_status);

        } else {
            error!("Error retrieving token status");
        }
        Ok(Json(token_status_json))
    }

    pub async fn generate_user_script(&mut self, query: TokenParams) -> Result<String, String> {
        let tables_result = fetch_project_tables_names_request(query.clone()).await;
    
        match tables_result {
            Ok(tables) => {
                let mut script_lines = Vec::new();
    
                for bridgehead in &query.bridgehead_ids {

                    let records_result = tokens
                    .filter(project_id.eq(&query.project_id))
                    .filter(user_id.eq(&query.user_id))
                    .filter(bk.eq(bridgehead))
                    .order(id.desc())
                    .select(TokenManager::as_select())
                    .first::<TokenManager>(&mut self.0);
    
                    match records_result {
                        Ok(record) => {
                                for table in &tables {
                                    let site_name = record.bk.split('.').nth(1).expect("Valid app id");
                                    script_lines.push(format!(
                                        "builder$append(server='{}', url='https://{}/opal/', token='{}', table='{}', driver='OpalDriver')",
                                        site_name, record.bk, record.token, table
                                    ));
                                }
                            }
                        Err(_) => {
                            info!("No records were found");
                            return Ok("No records found for the given project and user.".into());
                        },
                    }
                }
    
                if !script_lines.is_empty() {
                    let script = generate_r_script(script_lines); // Assuming this function exists and works as intended
                    Ok(script)
                } else {
                    Ok("No records found for the given project and user.".into())
                }
            },
            Err(e) => {
                error!("Error in fetch_project_tables_names_request: {:?}", e);
                Err("Error obtaining table names.".into())
            },
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
