use tauri::AppHandle;

use crate::db;
use crate::db::repository;

const UI_LANGUAGE_KEY: &str = "ui_language";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiLanguage {
    En,
    Zh,
}

impl UiLanguage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Zh => "zh",
        }
    }

    pub fn from_value(value: &str) -> Option<Self> {
        let normalized = value.trim().to_ascii_lowercase();
        if normalized == "en" || normalized.starts_with("en-") {
            return Some(Self::En);
        }
        if normalized == "zh" || normalized.starts_with("zh-") {
            return Some(Self::Zh);
        }
        None
    }
}

pub fn resolve_ui_language(app: &AppHandle) -> UiLanguage {
    if let Ok(connection) = db::get_connection(app) {
        if let Ok(Some(value)) = repository::get_app_metadata(&connection, UI_LANGUAGE_KEY) {
            if let Some(language) = UiLanguage::from_value(&value) {
                return language;
            }
        }
    }

    UiLanguage::Zh
}

pub fn persist_ui_language(app: &AppHandle, value: &str) -> Result<UiLanguage, String> {
    let language =
        UiLanguage::from_value(value).ok_or_else(|| format!("Unsupported UI language: {value}"))?;

    db::initialize(app)?;
    let connection = db::get_connection(app)?;
    repository::set_app_metadata(&connection, UI_LANGUAGE_KEY, language.as_str())?;

    Ok(language)
}
