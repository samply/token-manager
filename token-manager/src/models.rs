use crate::schema::tokens;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct TokenParams {
    pub email: String,
    pub project_id: String,
    pub bridgehead_ids: Vec<String>,
}

#[derive(Deserialize)]
pub struct ScriptParams {
    pub project: String,
    pub user: String,
}

#[derive(Serialize)]
pub struct OpalRequest {
    pub name: String,
    pub project: String,
}

#[derive(Debug, Serialize, Deserialize, Queryable, Selectable)]
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
    pub bk: &'a str,
    pub status: &'a str,
    pub user_id: &'a str,
    pub created_at: &'a str,
}
