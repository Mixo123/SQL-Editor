//! Global application configuration and default visual styling constants.

#![allow(dead_code)]

pub const MAX_FETCH_ROWS: usize = 50_000;
pub const MAX_RENDER_ROWS: usize = 10_000;
pub const STREAM_BATCH: usize = 2_000;
pub const RENDER_CHUNK: usize = 200;
pub const RENDER_BUDGET_MS: u64 = 25;
pub const MAX_HIGHLIGHT_CHARS: usize = 200_000;

pub const BG: &str = "#1e1e1e";
pub const BG_PANEL: &str = "#252526";
pub const BG_HEADER: &str = "#2d2d2d";
pub const BG_TAB: &str = "#2d2d2d";
pub const BG_TAB_ACTIVE: &str = "#1e1e1e";
pub const FG: &str = "#d4d4d4";

pub const BTN_PRIMARY: &str = "#0e639c";
pub const BTN_PRIMARY_H: &str = "#1177bb";
pub const BTN_SECONDARY: &str = "#3a3d41";
pub const BTN_SECONDARY_H: &str = "#4a4d51";
pub const BTN_TEXT: &str = "#ffffff";
pub const BTN_TEXT_DIM: &str = "#dddddd";
