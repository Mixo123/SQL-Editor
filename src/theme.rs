//! Theme and appearance management for SQL Editor.
//! Provides preset themes, customizable colors, and visual styling.

#![allow(dead_code)]

use egui::Color32;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const DEFAULT_THEME: &str = "Dark (Default)";

pub const PRESET_THEME_NAMES: &[&str] = &[
    "Dark (Default)",
    "Dracula",
    "One Dark Pro",
    "Monokai",
    "Nord",
    "Solarized Dark",
    "GitHub Light",
];

pub fn hex_to_color(hex: &str) -> Color32 {
    let clean = hex.trim().trim_start_matches('#');
    if clean.len() == 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&clean[0..2], 16),
            u8::from_str_radix(&clean[2..4], 16),
            u8::from_str_radix(&clean[4..6], 16),
        ) {
            return Color32::from_rgb(r, g, b);
        }
    } else if clean.len() == 8 {
        if let (Ok(r), Ok(g), Ok(b), Ok(a)) = (
            u8::from_str_radix(&clean[0..2], 16),
            u8::from_str_radix(&clean[2..4], 16),
            u8::from_str_radix(&clean[4..6], 16),
            u8::from_str_radix(&clean[6..8], 16),
        ) {
            return Color32::from_rgba_premultiplied(r, g, b, a);
        }
    }
    Color32::from_rgb(200, 200, 200)
}

pub fn color_to_hex(c: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    pub name: String,
    pub bg: String,
    pub bg_panel: String,
    pub bg_header: String,
    pub fg: String,
    pub btn_primary: String,
    pub btn_primary_h: String,
    pub btn_secondary: String,
    pub btn_secondary_h: String,
    pub select_bg: String,
    pub kw: String,
    pub str: String,
    pub num: String,
    pub cmt: String,
    pub ident: String,
}

impl Theme {
    pub fn c_bg(&self) -> Color32 { hex_to_color(&self.bg) }
    pub fn c_bg_panel(&self) -> Color32 { hex_to_color(&self.bg_panel) }
    pub fn c_bg_header(&self) -> Color32 { hex_to_color(&self.bg_header) }
    pub fn c_fg(&self) -> Color32 { hex_to_color(&self.fg) }
    pub fn c_btn_primary(&self) -> Color32 { hex_to_color(&self.btn_primary) }
    pub fn c_btn_primary_h(&self) -> Color32 { hex_to_color(&self.btn_primary_h) }
    pub fn c_btn_secondary(&self) -> Color32 { hex_to_color(&self.btn_secondary) }
    pub fn c_btn_secondary_h(&self) -> Color32 { hex_to_color(&self.btn_secondary_h) }
    pub fn c_select_bg(&self) -> Color32 { hex_to_color(&self.select_bg) }
    pub fn c_kw(&self) -> Color32 { hex_to_color(&self.kw) }
    pub fn c_str(&self) -> Color32 { hex_to_color(&self.str) }
    pub fn c_num(&self) -> Color32 { hex_to_color(&self.num) }
    pub fn c_cmt(&self) -> Color32 { hex_to_color(&self.cmt) }
    pub fn c_ident(&self) -> Color32 { hex_to_color(&self.ident) }
}

pub fn get_preset_theme(name: &str) -> Theme {
    match name {
        "Dracula" => Theme {
            name: "Dracula".to_string(),
            bg: "#282a36".to_string(),
            bg_panel: "#21222c".to_string(),
            bg_header: "#191a21".to_string(),
            fg: "#f8f8f2".to_string(),
            btn_primary: "#bd93f9".to_string(),
            btn_primary_h: "#ff79c6".to_string(),
            btn_secondary: "#44475a".to_string(),
            btn_secondary_h: "#6272a4".to_string(),
            select_bg: "#44475a".to_string(),
            kw: "#ff79c6".to_string(),
            str: "#f1fa8c".to_string(),
            num: "#bd93f9".to_string(),
            cmt: "#6272a4".to_string(),
            ident: "#50fa7b".to_string(),
        },
        "One Dark Pro" => Theme {
            name: "One Dark Pro".to_string(),
            bg: "#282c34".to_string(),
            bg_panel: "#21252b".to_string(),
            bg_header: "#1e2227".to_string(),
            fg: "#abb2bf".to_string(),
            btn_primary: "#61afef".to_string(),
            btn_primary_h: "#528bff".to_string(),
            btn_secondary: "#3e4451".to_string(),
            btn_secondary_h: "#4b5263".to_string(),
            select_bg: "#3e4451".to_string(),
            kw: "#c678dd".to_string(),
            str: "#98c379".to_string(),
            num: "#d19a66".to_string(),
            cmt: "#5c6370".to_string(),
            ident: "#e06c75".to_string(),
        },
        "Monokai" => Theme {
            name: "Monokai".to_string(),
            bg: "#272822".to_string(),
            bg_panel: "#1e1f1c".to_string(),
            bg_header: "#171814".to_string(),
            fg: "#f8f8f2".to_string(),
            btn_primary: "#a6e22e".to_string(),
            btn_primary_h: "#f92672".to_string(),
            btn_secondary: "#3e3d32".to_string(),
            btn_secondary_h: "#49483e".to_string(),
            select_bg: "#49483e".to_string(),
            kw: "#f92672".to_string(),
            str: "#e6db74".to_string(),
            num: "#ae81ff".to_string(),
            cmt: "#75715e".to_string(),
            ident: "#66d9ef".to_string(),
        },
        "Nord" => Theme {
            name: "Nord".to_string(),
            bg: "#2e3440".to_string(),
            bg_panel: "#242933".to_string(),
            bg_header: "#1e222a".to_string(),
            fg: "#eceff4".to_string(),
            btn_primary: "#88c0d0".to_string(),
            btn_primary_h: "#81a1c1".to_string(),
            btn_secondary: "#3b4252".to_string(),
            btn_secondary_h: "#434c5e".to_string(),
            select_bg: "#434c5e".to_string(),
            kw: "#81a1c1".to_string(),
            str: "#a3be8c".to_string(),
            num: "#b48ead".to_string(),
            cmt: "#616e88".to_string(),
            ident: "#8fbcbb".to_string(),
        },
        "Solarized Dark" => Theme {
            name: "Solarized Dark".to_string(),
            bg: "#002b36".to_string(),
            bg_panel: "#073642".to_string(),
            bg_header: "#00212b".to_string(),
            fg: "#839496".to_string(),
            btn_primary: "#268bd2".to_string(),
            btn_primary_h: "#2aa198".to_string(),
            btn_secondary: "#073642".to_string(),
            btn_secondary_h: "#586e75".to_string(),
            select_bg: "#073642".to_string(),
            kw: "#859900".to_string(),
            str: "#2aa198".to_string(),
            num: "#d33682".to_string(),
            cmt: "#586e75".to_string(),
            ident: "#b58900".to_string(),
        },
        "GitHub Light" => Theme {
            name: "GitHub Light".to_string(),
            bg: "#ffffff".to_string(),
            bg_panel: "#f6f8fa".to_string(),
            bg_header: "#ebeef1".to_string(),
            fg: "#24292f".to_string(),
            btn_primary: "#0969da".to_string(),
            btn_primary_h: "#0858b7".to_string(),
            btn_secondary: "#d0d7de".to_string(),
            btn_secondary_h: "#afb8c1".to_string(),
            select_bg: "#b6e3ff".to_string(),
            kw: "#cf222e".to_string(),
            str: "#0a3069".to_string(),
            num: "#0550ae".to_string(),
            cmt: "#6e7781".to_string(),
            ident: "#116329".to_string(),
        },
        _ => Theme {
            name: "Dark (Default)".to_string(),
            bg: "#1e1e1e".to_string(),
            bg_panel: "#252526".to_string(),
            bg_header: "#2d2d2d".to_string(),
            fg: "#d4d4d4".to_string(),
            btn_primary: "#0e639c".to_string(),
            btn_primary_h: "#1177bb".to_string(),
            btn_secondary: "#3a3d41".to_string(),
            btn_secondary_h: "#4a4d51".to_string(),
            select_bg: "#264f78".to_string(),
            kw: "#569cd6".to_string(),
            str: "#ce9178".to_string(),
            num: "#b5cea8".to_string(),
            cmt: "#6a9955".to_string(),
            ident: "#9cdcfe".to_string(),
        },
    }
}

pub fn get_theme(name: &str, overrides: &HashMap<String, String>) -> Theme {
    let mut theme = get_preset_theme(name);
    if let Some(v) = overrides.get("bg") { theme.bg = v.clone(); }
    if let Some(v) = overrides.get("bg_panel") { theme.bg_panel = v.clone(); }
    if let Some(v) = overrides.get("bg_header") { theme.bg_header = v.clone(); }
    if let Some(v) = overrides.get("fg") { theme.fg = v.clone(); }
    if let Some(v) = overrides.get("btn_primary") { theme.btn_primary = v.clone(); }
    if let Some(v) = overrides.get("btn_secondary") { theme.btn_secondary = v.clone(); }
    if let Some(v) = overrides.get("select_bg") { theme.select_bg = v.clone(); }
    if let Some(v) = overrides.get("kw") { theme.kw = v.clone(); }
    if let Some(v) = overrides.get("str") { theme.str = v.clone(); }
    if let Some(v) = overrides.get("num") { theme.num = v.clone(); }
    if let Some(v) = overrides.get("cmt") { theme.cmt = v.clone(); }
    if let Some(v) = overrides.get("ident") { theme.ident = v.clone(); }
    theme
}
