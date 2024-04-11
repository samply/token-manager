use anyhow::Result;
use axum::{
    async_trait,
    extract::{FromRef, FromRequestParts},
    http::{request::Parts, StatusCode},
    Json,
};
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use diesel::result::Error;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use serde_json::json;
use std::collections::HashSet;
use tracing::{error, info, warn};

use crate::config::CONFIG;
use crate::enums::{OpalProjectStatus, OpalTokenStatus};
use crate::handlers::{
    check_project_status_request, check_token_status_request, fetch_project_tables_names_request,
};
use crate::models::{
    NewToken, ProjectQueryParams, TokenManager, TokenParams, TokenStatus, TokensQueryParams,
};
use crate::schema::tokens;
use crate::schema::tokens::dsl::*;
use crate::utils::{decrypt_data, generate_r_script};

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

        pool.get()
            .map(Self)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
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
                info!("Token Saved in DB for user: {} in BK: {}", new_token.user_id, new_token.bk);
            }
            Err(error) => {
                warn!("Error connecting to {}", error);
            }
        }
    }

    pub fn update_token_db(&mut self, token_update: NewToken) {
        let maybe_last_id = tokens
            .filter(
                user_id
                    .eq(&token_update.user_id)
                    .and(project_id.eq(&token_update.project_id))
                    .and(bk.eq(&token_update.bk)),
            )
            .select(id)
            .order(id.desc())
            .first::<i32>(&mut self.0)
            .optional(); 

        if let Ok(Some(last_id)) = maybe_last_id {
            let target = tokens.filter(id.eq(last_id));
            match diesel::update(target)
                .set((
                    token.eq(&token_update.token),
                    token_status.eq("UPDATED"),
                    token_created_at.eq(&token_update.token_created_at),
                ))
                .execute(&mut self.0)
            {
                Ok(_) => info!("Token Updated in DB for user: {} in BK: {}", token_update.user_id, token_update.bk),
                Err(error) => warn!("Error updating token: {}", error),
            }
        } else if let Err(error) = maybe_last_id {
            warn!("Error finding last token record: {}", error);
        }
    }

    pub fn update_token_status_db(&mut self, token_update: TokenStatus) {
        let target = tokens.filter(
            user_id
                .eq(&token_update.user_id)
                .and(project_id.eq(&token_update.project_id))
                .and(bk.eq(&token_update.bk)),
        );

        match diesel::update(target)
            .set((token_status.eq(token_update.token_status),))
            .execute(&mut self.0)
        {
            Ok(_) => {
                info!("Token status updated in DB for user: {} in BK: {}", token_update.user_id, token_update.bk);
            }
            Err(error) => {
                warn!("Error updating token status: {}", error);
            }
        }
    }

    pub fn delete_project_db(&mut self, project_params: &ProjectQueryParams,) {
        let target = tokens.filter(project_id.eq(&project_params.project_id));

        match diesel::delete(target).execute(&mut self.0) {
            Ok(_) => {
                info!("Project and Token deleted from DB for project: {} in BK: {}", &project_params.project_id, &project_params.bk);
            }
            Err(error) => {
                warn!("Error deleting token: {}", error);
            }
        }
    }

    pub fn delete_token_db(&mut self, token_name_id: String, token_params: &TokensQueryParams) {
        let target = tokens.filter(token_name.eq(&token_name_id));

        match diesel::delete(target).execute(&mut self.0) {
            Ok(_) => {
                info!("Token deleted from DB for user: {} in BK: {}", token_params.user_id, token_params.bk);
            }
            Err(error) => {
                warn!("Error deleting token: {}", error);
            }
        }
    }

    pub fn get_token_name(
        &mut self,
        token_params: &TokensQueryParams,
    ) -> Result<Option<String>, Error> {
        tokens
            .filter(user_id.eq(token_params.user_id.clone()))
            .filter(project_id.eq(token_params.project_id.clone()))
            .filter(bk.eq(token_params.bk.clone()))
            .order(id.desc())
            .select(token_name)
            .first::<String>(&mut self.0)
            .optional()
    }

    pub fn get_token_value(
        &mut self,
        user: String,
        project: String,
        bridgehead: String,
    ) -> Result<Option<String>, Error> {
        tokens
            .filter(user_id.eq(user))
            .filter(project_id.eq(project))
            .filter(bk.eq(bridgehead))
            .order(id.desc())
            .select(token)
            .first::<String>(&mut self.0)
            .optional()
    }

    pub fn is_token_available(&mut self, params: &TokenParams) -> Result<bool, Error> {
        let result = tokens
            .filter(user_id.eq(&params.user_id))
            .filter(project_id.eq(&params.project_id))
            .filter(bk.eq_any(&params.bridgehead_ids))
            .first::<TokenManager>(&mut self.0)
            .optional();

        match result {
            Ok(Some(_)) => Ok(true),
            Ok(None) => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub async fn check_script_status(
        &mut self,
        params: TokenParams,
    ) -> Result<String, (StatusCode, String)> {
        let token_available = self.is_token_available(&params);

        match token_available {
            Ok(true) => {
                info!("Token available for user: {}", params.user_id);
                Ok("true".to_string())
            },
            Ok(false) => {
                info!("No Token available for user: {}", params.user_id);
                Ok("false".to_string())
            },
            Err(_) => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Error checking token availability.".to_string(),
            )),
        }
    }

    pub async fn check_token_status(
        &mut self,
        params: TokensQueryParams,
    ) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
        let mut token_status_json = json!({
            "project_id": params.project_id.clone(),
            "bk": params.bk.clone(),
            "user_id": params.user_id.clone(),
            "token_created_at": "",
            "project_status": OpalTokenStatus::NOTFOUND,
            "token_status": OpalProjectStatus::NOTFOUND,
        });

        if let Ok(json_response) = check_project_status_request(ProjectQueryParams {
            bk: params.bk.clone(),
            project_id: params.project_id.clone(),
        })
        .await
        {
            token_status_json["project_status"] = json_response.0["project_status"].clone();
        } else {
            error!("Error retrieving project status");
        }

        let token_name_response = match self.get_token_name(&params) {
            Ok(Some(name)) => name,
            Ok(None) =>{ 
                info!(
                    "Received status response for token. User ID: {}, BK: {}, Response: {}",
                    params.user_id,
                    params.bk,
                    token_status_json["token_status"]
                );
                return Ok(Json(token_status_json))
            },
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
                info!(
                    "Received status response for token. User ID: {}, BK: {}, Response: {}",
                    params.user_id,
                    params.bk,
                    token_status_json["token_status"]
                );
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

        if let Ok(json_response) = check_token_status_request(
            params.user_id.clone(),
            params.bk.clone(),
            params.project_id.clone(),
            token_name_response.clone(),
            token_value.clone(),
        )
        .await
        {
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

        info!(
            "Received status response for token. User ID: {}, BK: {}, Response: {}",
            params.user_id,
            params.bk,
            token_status_json["token_status"]
        );

        Ok(Json(token_status_json))
    }

    pub async fn generate_user_script(&mut self, query: TokenParams) -> Result<String, String> {
        let tables_per_bridgehead_result = fetch_project_tables_names_request(query.clone()).await;

        match tables_per_bridgehead_result {
            Ok(tables_per_bridgehead) => {
                let mut script_lines = Vec::new();
                let all_tables = tables_per_bridgehead
                    .values()
                    .flat_map(|tables| tables.iter())
                    .collect::<HashSet<_>>();

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
                            let token_decrypt = decrypt_data(
                                record.token.clone(),
                                &record.token_name.clone().as_bytes()[..16],
                            );
                            if let Some(tables) = tables_per_bridgehead.get(bridgehead) {
                                let tables_set: HashSet<_> = tables.iter().collect();
                                let missing_tables: HashSet<_> =
                                    all_tables.difference(&tables_set).collect();
                                if !missing_tables.is_empty() {
                                    info!(
                                        "Bridgehead {} is missing tables: {:?}",
                                        bridgehead, missing_tables
                                    );
                                    script_lines.push(format!(
                                        "\n # Tables not available for bridgehead '{}': {:?}",
                                        bridgehead, missing_tables
                                    ));
                                }

                                for table in tables {
                                    let site_name =
                                        record.bk.split('.').nth(1).expect("Valid app id");
                                    script_lines.push(format!(
                                        "builder$append(server='{}', url='https://{}/opal/', token='{}', table='{}', driver='OpalDriver')",
                                        site_name, record.bk, token_decrypt, table.clone()
                                    ));
                                }
                                script_lines.push("".to_string());
                            }
                        }
                        Err(_) => {
                            info!("Token not available for Bridgehead {}", bridgehead);
                            script_lines.push(format!(
                                "\n # Token not available for bridgehead '{}'",
                                bridgehead
                            ));
                        }
                    }
                }

                if !script_lines.is_empty() {
                    let script = generate_r_script(script_lines);
                    Ok(script)
                } else {
                    Ok("No records found for the given project and user.".into())
                }
            }
            Err(e) => {
                error!("Error in fetch_project_tables_names_request: {:?}", e);
                Err("Error obtaining table names.".into())
            }
        }
    }
}
