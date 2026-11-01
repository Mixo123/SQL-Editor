//! Entry point for SQL Editor application.

use eframe::egui::{IconData, Vec2, ViewportBuilder};
use sql_editor::app::SqlEditorApp;
use std::sync::Arc;

fn load_icon() -> Option<Arc<IconData>> {
    let icon_bytes = include_bytes!("../assets/icon.png");
    if let Ok(img) = image::load_from_memory(icon_bytes) {
        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();
        return Some(Arc::new(IconData {
            rgba: rgba.into_raw(),
            width,
            height,
        }));
    }
    None
}

fn main() -> eframe::Result<()> {
    let mut viewport = ViewportBuilder::default()
        .with_title("SQL Editor")
        .with_inner_size(Vec2::new(1400.0, 860.0))
        .with_min_inner_size(Vec2::new(800.0, 560.0));

    if let Some(icon) = load_icon() {
        viewport = viewport.with_icon(icon);
    }

    let native_options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "SQL Editor",
        native_options,
        Box::new(|cc| Ok(Box::new(SqlEditorApp::new(cc)))),
    )
}
