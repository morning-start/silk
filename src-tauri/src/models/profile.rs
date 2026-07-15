use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub agent_type: String,
    pub config_json: String,
    pub is_active: bool,
    pub sort_index: Option<i64>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewProfile {
    pub name: String,
    pub agent_type: String,
    pub config_json: String,
    pub is_active: Option<bool>,
    pub sort_index: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateProfile {
    pub name: Option<String>,
    pub config_json: Option<String>,
    pub is_active: Option<bool>,
    pub sort_index: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ProfileRoleMapping {
    pub id: String,
    pub profile_id: String,
    pub role_key: String,
    pub model_mapping_id: String,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewProfileRoleMapping {
    pub profile_id: String,
    pub role_key: String,
    pub model_mapping_id: String,
}
