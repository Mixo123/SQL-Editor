//! Session persistence for open tabs, active connection, and editor layout.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::crypto::session_file;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedTab {
    pub name: String,
    pub path: Option<PathBuf>,
    pub content: String,
    pub modified: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppSession {
    pub active_conn_name: Option<String>,
    pub active_tab_index: usize,
    pub editor_height: Option<f32>,
    pub tabs: Vec<SavedTab>,
}

impl Default for AppSession {
    fn default() -> Self {
        Self {
            active_conn_name: None,
            active_tab_index: 0,
            editor_height: None,
            tabs: Vec::new(),
        }
    }
}

pub fn load_session_from(file: &Path) -> Option<AppSession> {
    if !file.exists() {
        return None;
    }
    let data = fs::read_to_string(file).ok()?;
    serde_json::from_str(&data).ok()
}

pub fn save_session_to(session: &AppSession, file: &Path) {
    if let Some(parent) = file.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(session) {
        let _ = fs::write(file, json);
    }
}

pub fn load_session() -> Option<AppSession> {
    load_session_from(&session_file())
}

pub fn save_session(session: &AppSession) {
    save_session_to(session, &session_file());
}
