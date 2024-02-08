use crate::schema::tokens;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Clone)]
pub struct TokenParams {
    pub user_id: String,
    pub project_id: String,
    pub bridgehead_ids: Vec<String>,
}

#[derive(Deserialize, Clone)]
pub struct ScriptParams {
    pub project_id: String,
    pub user_id: String,
}

#[derive(Serialize, Debug)]
pub struct OpalRequest {
    pub request_type: String,
    pub name: Option<String>,
    pub project: Option<String>,
    pub token: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Queryable, Selectable)]
#[diesel(table_name = crate::schema::tokens)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct TokenManager {
    pub id: i32,
    pub token_name: String,
    pub token: String,
    pub project_id: String,
    pub project_status: String,
    pub bk: String,
    pub token_status: String,
    pub user_id: String,
    pub token_created_at: String,
}

#[derive(Insertable)]
#[diesel(table_name = tokens)]
pub struct NewToken<'a> {
    pub token_name: &'a str,
    pub token: &'a str,
    pub project_id: &'a str,
    pub project_status: &'a str,
    pub bk: &'a str,
    pub token_status: &'a str,
    pub user_id: &'a str,
    pub token_created_at: &'a str,
}

#[derive(Insertable)]
#[diesel(table_name = tokens)]
pub struct TokenStatus<'a> {
    pub project_id: &'a str,
    pub bk: &'a str,
    pub token_status: &'a str,
    pub user_id: &'a str,
}


#[derive(Deserialize)]
pub struct TokensQueryParams {
    pub user_id: String,
    pub bk: String,
    pub project_id: String,
}

#[derive(Deserialize)]
pub struct ProjectStatusQuery {
    pub bk: String,
    pub project_id: String,
}
