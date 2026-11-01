//! Settings dialog (General preferences & Themes customization).

use egui::{Align2, Color32, Context, FontId, Id, RichText, ScrollArea, Slider, Vec2, Window};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

use crate::crypto::{ensure_app_dir, settings_file};
use crate::highlighter::highlight_sql;
use crate::i18n::{set_language, tr, LANGUAGES};
use crate::theme::{color_to_hex, get_theme, hex_to_color, Theme, DEFAULT_THEME, PRESET_THEME_NAMES};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub language: String,
    pub warn_unbounded_select: bool,
    pub multiple_result_tabs: bool,
    pub syntax_highlighting: bool,
    pub autocomplete: bool,
    pub check_updates: bool,
    pub editor_font_size: u32,
    pub editor_font_family: String,
    pub theme_name: String,
    pub custom_theme_colors: HashMap<String, String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            language: "pl".to_string(),
            warn_unbounded_select: true,
            multiple_result_tabs: true,
            syntax_highlighting: true,
            autocomplete: true,
            check_updates: true,
            editor_font_size: 13,
            editor_font_family: "Menlo".to_string(),
            theme_name: DEFAULT_THEME.to_string(),
            custom_theme_colors: HashMap::new(),
        }
    }
}

pub fn load_settings() -> AppSettings {
    let path = settings_file();
    if path.exists() {
        if let Ok(data) = fs::read_to_string(&path) {
            if let Ok(s) = serde_json::from_str::<AppSettings>(&data) {
                return s;
            }
        }
    }
    AppSettings::default()
}

pub fn save_settings(settings: &AppSettings) -> Result<(), String> {
    ensure_app_dir();
    let path = settings_file();
    let tmp = path.with_extension("tmp");
    let json = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(&tmp, json).map_err(|e| e.to_string())?;
    fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
    Ok(())
}

pub struct SettingsDialog {
    pub open: bool,
    pub active_tab: usize, // 0 = General, 1 = Themes
    pub temp_settings: AppSettings,
}

impl SettingsDialog {
    pub fn new() -> Self {
        Self {
            open: false,
            active_tab: 0,
            temp_settings: AppSettings::default(),
        }
    }

    pub fn open_dialog(&mut self, current: &AppSettings) {
        self.open = true;
        self.active_tab = 0;
        self.temp_settings = current.clone();
    }

    pub fn show(
        &mut self,
        ctx: &Context,
        theme: &mut Theme,
        settings: &mut AppSettings,
    ) {
        if !self.open {
            return;
        }

        let mut close_dialog = false;
        let mut save_and_apply = false;

        Window::new(RichText::new(tr("settings_title")).size(16.0).color(theme.c_fg()))
            .id(Id::new("settings_modal"))
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .collapsible(false)
            .resizable(true)
            .default_size(Vec2::new(640.0, 600.0))
            .show(ctx, |ui| {
                // Tab headers
                ui.horizontal(|ui| {
                    if ui.selectable_label(self.active_tab == 0, RichText::new(tr("tab_general")).size(14.0)).clicked() {
                        self.active_tab = 0;
                    }
                    if ui.selectable_label(self.active_tab == 1, RichText::new(tr("tab_themes")).size(14.0)).clicked() {
                        self.active_tab = 1;
                    }
                });
                ui.separator();

                ScrollArea::vertical().show(ui, |ui| {
                    if self.active_tab == 0 {
                        // General Tab
                        ui.spacing_mut().item_spacing = Vec2::new(8.0, 12.0);

                        // Language selector
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(tr("lbl_language")).strong());
                            let curr_lang_name = crate::i18n::language_name(&self.temp_settings.language);
                            egui::ComboBox::from_id_salt("settings_lang_combo")
                                .selected_text(curr_lang_name)
                                .show_ui(ui, |ui| {
                                    for &(code, name) in LANGUAGES {
                                        if ui.selectable_label(self.temp_settings.language == code, name).clicked() {
                                            self.temp_settings.language = code.to_string();
                                            set_language(code);
                                        }
                                    }
                                });
                        });

                        ui.checkbox(&mut self.temp_settings.warn_unbounded_select, tr("opt_warn_unbounded"));
                        ui.checkbox(&mut self.temp_settings.multiple_result_tabs, tr("opt_multi_tabs"));
                        ui.checkbox(&mut self.temp_settings.syntax_highlighting, tr("opt_syntax_highlight"));
                        ui.checkbox(&mut self.temp_settings.autocomplete, tr("opt_autocomplete"));
                        ui.checkbox(&mut self.temp_settings.check_updates, tr("opt_check_updates"));
                    } else {
                        // Themes Tab
                        ui.spacing_mut().item_spacing = Vec2::new(8.0, 10.0);

                        // Preset selector
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(tr("lbl_theme_preset")).strong());
                            egui::ComboBox::from_id_salt("settings_theme_preset")
                                .selected_text(&self.temp_settings.theme_name)
                                .show_ui(ui, |ui| {
                                    for &name in PRESET_THEME_NAMES {
                                        if ui.selectable_label(self.temp_settings.theme_name == name, name).clicked() {
                                            self.temp_settings.theme_name = name.to_string();
                                            // Reset custom colors when switching preset
                                            self.temp_settings.custom_theme_colors.clear();
                                        }
                                    }
                                });
                        });

                        // Font size slider
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(tr("lbl_font_size")).strong());
                            ui.add(Slider::new(&mut self.temp_settings.editor_font_size, 9..=28).suffix(" pt"));
                        });

                        ui.separator();
                        ui.label(RichText::new(tr("sec_custom_colors")).strong());

                        // Color picker helpers
                        let preview_theme = get_theme(&self.temp_settings.theme_name, &self.temp_settings.custom_theme_colors);

                        let mut edit_color = |label: &str, key: &str, current_hex: &str| {
                            ui.horizontal(|ui| {
                                ui.label(label);
                                let mut color = hex_to_color(current_hex);
                                if ui.color_edit_button_srgba(&mut color).changed() {
                                    self.temp_settings.custom_theme_colors.insert(key.to_string(), color_to_hex(color));
                                }
                            });
                        };

                        edit_color(tr("color_bg").as_str(), "bg", &preview_theme.bg);
                        edit_color(tr("color_bg_panel").as_str(), "bg_panel", &preview_theme.bg_panel);
                        edit_color(tr("color_bg_header").as_str(), "bg_header", &preview_theme.bg_header);
                        edit_color(tr("color_fg").as_str(), "fg", &preview_theme.fg);
                        edit_color(tr("color_btn_primary").as_str(), "btn_primary", &preview_theme.btn_primary);

                        if ui.button(tr("btn_reset_theme")).clicked() {
                            self.temp_settings.theme_name = DEFAULT_THEME.to_string();
                            self.temp_settings.custom_theme_colors.clear();
                        }

                        ui.separator();
                        ui.label(RichText::new(tr("sec_live_preview")).strong());

                        // Live SQL preview box
                        let sample_sql = "-- Live SQL Preview\nSELECT id, name, created_at\nFROM users\nWHERE status = 'ACTIVE' AND age >= 18\nORDER BY created_at DESC\nLIMIT 50;";
                        let font_id = FontId::monospace(self.temp_settings.editor_font_size as f32);
                        let job = highlight_sql(sample_sql, &preview_theme, font_id, true);

                        egui::Frame::canvas(ui.style())
                            .fill(preview_theme.c_bg())
                            .stroke(egui::Stroke::new(1.0_f32, preview_theme.c_bg_header()))
                            .inner_margin(8.0)
                            .show(ui, |ui| {
                                ui.label(job);
                            });
                    }
                });

                ui.separator();

                // Bottom buttons
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let save_btn = egui::Button::new(RichText::new(tr("btn_save_apply")).color(Color32::WHITE).strong())
                            .fill(theme.c_btn_primary());
                        if ui.add(save_btn).clicked() {
                            save_and_apply = true;
                            close_dialog = true;
                        }

                        if ui.button(tr("btn_close")).clicked() {
                            close_dialog = true;
                        }
                    });
                });
            });

        if save_and_apply {
            *settings = self.temp_settings.clone();
            set_language(&settings.language);
            *theme = get_theme(&settings.theme_name, &settings.custom_theme_colors);
            let _ = save_settings(settings);
        }

        if close_dialog {
            self.open = false;
        }
    }
}
