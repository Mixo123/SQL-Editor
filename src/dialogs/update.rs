//! Update dialog showing new version info and download options.

use egui::{Align2, Color32, Context, Id, RichText, ScrollArea, Vec2, Window};

use crate::i18n::tr;
use crate::theme::Theme;
use crate::updater::UpdateCheckResult;

pub struct UpdateDialog {
    pub open: bool,
    pub result: Option<UpdateCheckResult>,
}

impl UpdateDialog {
    pub fn new() -> Self {
        Self {
            open: false,
            result: None,
        }
    }

    pub fn show_result(&mut self, res: UpdateCheckResult) {
        self.open = true;
        self.result = Some(res);
    }

    pub fn show(&mut self, ctx: &Context, theme: &Theme) {
        if !self.open {
            return;
        }

        let Some(ref res) = self.result else {
            return;
        };

        let mut close_dialog = false;

        let title = if res.update_available {
            "🚀 Dostępna nowa aktualizacja!"
        } else if !res.success {
            "Sprawdzanie aktualizacji"
        } else {
            "SQL Editor jest aktualny"
        };

        Window::new(RichText::new(title).size(16.0).color(theme.c_fg()))
            .id(Id::new("update_modal"))
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .collapsible(false)
            .resizable(true)
            .default_size(Vec2::new(560.0, 480.0))
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 10.0);

                if !res.success {
                    let err = res.error.as_deref().unwrap_or("Nieznany błąd.");
                    ui.label(RichText::new("Nie udało się połączyć z serwerem wydań GitHub:").color(Color32::from_rgb(255, 120, 120)));
                    ui.label(err);
                } else if res.update_available {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Twoja wersja:").strong());
                        ui.label(&res.current_version);
                        ui.label(RichText::new("→ Nowa wersja:").strong().color(Color32::from_rgb(100, 255, 100)));
                        ui.label(RichText::new(&res.latest_version).strong().color(Color32::from_rgb(100, 255, 100)));
                    });

                    ui.label(RichText::new(format!("Wydanie: {}", res.release_name)).strong());

                    ui.separator();
                    ui.label(RichText::new("Opis zmian (Changelog):").strong());

                    ScrollArea::vertical()
                        .max_height(200.0)
                        .show(ui, |ui| {
                            egui::Frame::canvas(ui.style())
                                .fill(theme.c_bg())
                                .inner_margin(8.0)
                                .show(ui, |ui| {
                                    ui.label(RichText::new(&res.release_notes).monospace().size(11.0));
                                });
                        });

                    ui.separator();
                    ui.label(RichText::new("Dostępne instalatory dla Twojego systemu:").strong());

                    for asset in &res.recommended_assets {
                        ui.horizontal(|ui| {
                            let btn_text = format!("⬇ Pobierz: {} ({})", asset.label, asset.size_formatted);
                            let btn = if asset.recommended {
                                egui::Button::new(RichText::new(btn_text).color(Color32::WHITE).strong())
                                    .fill(theme.c_btn_primary())
                            } else {
                                egui::Button::new(RichText::new(btn_text).color(theme.c_fg()))
                                    .fill(theme.c_btn_secondary())
                            };

                            if ui.add(btn).clicked() {
                                let _ = open::that(&asset.download_url);
                            }
                        });
                    }
                } else {
                    ui.vertical_centered(|ui| {
                        ui.add_space(16.0);
                        ui.label(RichText::new("✓").size(32.0).color(Color32::from_rgb(100, 255, 100)));
                        ui.label(RichText::new(format!("Używasz najnowszej wersji SQL Editor (v{}).", res.current_version)).size(13.0));
                        ui.label("Brak nowszych aktualizacji.");
                        ui.add_space(16.0);
                    });
                }

                ui.separator();

                ui.horizontal(|ui| {
                    if ui.button("🌐 Otwórz stronę wydań na GitHubie").clicked() {
                        let _ = open::that(&res.html_url);
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(tr("btn_close")).clicked() {
                            close_dialog = true;
                        }
                    });
                });
            });

        if close_dialog {
            self.open = false;
        }
    }
}
