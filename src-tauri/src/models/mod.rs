use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapInfo {
    pub product_name: String,
    pub version: String,
    pub identifier: String,
    pub app_data_dir: String,
    pub database_path: String,
    pub phase0_complete: bool,
    pub phase1_complete: bool,
    pub phase2_complete: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseHealth {
    pub database_path: String,
    pub exists: bool,
    pub writable: bool,
    pub schema_version: Option<String>,
    pub initialized_at: Option<String>,
    pub migration_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppliedMigration {
    pub version: i64,
    pub name: String,
    pub applied_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableStat {
    pub table_name: String,
    pub row_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderProfileRecord {
    pub id: String,
    pub provider_key: String,
    pub display_name: String,
    pub base_url: Option<String>,
    pub api_format: String,
    pub api_key_env: Option<String>,
    pub enabled: bool,
    pub extra_json: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderProfileUpsertInput {
    pub id: String,
    pub provider_key: String,
    pub display_name: String,
    pub base_url: Option<String>,
    pub api_format: String,
    pub api_key_env: Option<String>,
    pub enabled: bool,
    pub extra_json: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseSummary {
    pub schema_version: Option<String>,
    pub initialized_at: Option<String>,
    pub applied_migrations: Vec<AppliedMigration>,
    pub tables: Vec<TableStat>,
    pub provider_profiles: Vec<ProviderProfileRecord>,
}
