use serde::Serialize;

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
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseHealth {
    pub database_path: String,
    pub exists: bool,
    pub writable: bool,
    pub schema_version: Option<String>,
    pub initialized_at: Option<String>,
}
