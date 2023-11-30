use serde::{Deserialize, Serialize};
use uuid::Uuid;
use diesel::prelude::*;
use crate::schema::tokens;

#[derive(Deserialize)]
pub struct HttpParams {
    pub email: String,
    pub project_id: String,       // Comma-separated values
    pub bridgehead_ids: String, // Comma-separated values
}

#[derive(Deserialize)]
pub struct ScriptParams {
    pub project: String,
    pub user: String,       
}

#[derive(Serialize)]
pub struct OpalRequest {
    pub name: String,
    pub token: Uuid,
    pub projects: String
}

#[derive(Serialize)]
pub struct TokenManagerResponse {
    pub email: String,
    pub token: Uuid,
    pub projects: Vec<String>,     // projects are a list of strings
    pub bridgeheads: Vec<String>, // bridgeheads are a list of strings
    pub r_script: String,
}

#[derive(Serialize)]
pub struct ProjectRequest {
    pub name: String,
    pub title: String
}

#[derive(Debug)]
#[derive(Serialize, Deserialize, Queryable, Selectable)]
#[diesel(table_name = crate::schema::tokens)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct TokenManager {
    pub id: i32,
    pub token: String,
    pub project_id: String,
    pub bk: String,
    pub status: String, 
    pub user_id: String,
    pub created_at: String,
}

#[derive(Insertable)]
#[diesel(table_name = tokens)]
pub struct NewToken<'a> {
    pub token: &'a str,
    pub project_id: &'a str,
    pub bk:  &'a str,
    pub status:  &'a str,
    pub user_id:  &'a str,
    pub created_at:  &'a str,
}