//! Update dialog showing new version info, download options, and in-app downloader with progress bar.

use egui::{Align2, Color32, Context, Id, ProgressBar, RichText, ScrollArea, Vec2, Window};
use std::sync::atomic::Ordering;

use crate::i18n::tr;
use crate::theme::Theme;
use crate::updater::{
    format_size, format_speed, start_download, ActiveDownload, UpdateCheckResult,
};

pub struct UpdateDialog {
    pub open: bool,
    pub result: Option<UpdateCheckResult>,
    pub download: Option<ActiveDownload>,
}

impl UpdateDialog {
    pub fn new() -> Self {
        Self {
            open: false,
            result: None,
            download: None,
        }
    }

    pub fn show_result(&mut self, res: UpdateCheckResult) {
        self.open = true;
        self.result = Some(res);
        self.download = None;
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
            .default_size(Vec2::new(580.0, 520.0))
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 10.0);

                if !res.success {
                    let err = res.error.as_deref().unwrap_or("Nieznany błąd.");
                    ui.label(
                        RichText::new("Nie udało się połączyć z serwerem wydań GitHub:")
                            .color(Color32::from_rgb(255, 120, 120)),
                    );
                    ui.label(err);
                } else if res.update_available {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Twoja wersja:").strong());
                        ui.label(&res.current_version);
                        ui.label(
                            RichText::new("→ Nowa wersja:")
                                .strong()
                                .color(Color32::from_rgb(100, 255, 100)),
                        );
                        ui.label(
                            RichText::new(&res.latest_version)
                                .strong()
                                .color(Color32::from_rgb(100, 255, 100)),
                        );
                    });

                    ui.label(RichText::new(format!("Wydanie: {}", res.release_name)).strong());

                    ui.separator();
                    ui.label(RichText::new("Opis zmian (Changelog):").strong());

                    ScrollArea::vertical()
                        .max_height(160.0)
                        .show(ui, |ui| {
                            egui::Frame::canvas(ui.style())
                                .fill(theme.c_bg())
                                .inner_margin(8.0)
                                .show(ui, |ui| {
                                    ui.label(
                                        RichText::new(&res.release_notes).monospace().size(11.0),
                                    );
                                });
                        });

                    ui.separator();

                    // Check active download state
                    let dl_snapshot = self.download.as_ref().map(|dl| {
                        let p = dl.progress.lock().unwrap().clone();
                        let is_running = !p.finished && p.error.is_none();
                        (is_running, p)
                    });

                    let is_downloading = dl_snapshot
                        .as_ref()
                        .map(|(running, _)| *running)
                        .unwrap_or(false);

                    if is_downloading {
                        ctx.request_repaint();
                    }

                    // 1. Available download options
                    ui.label(RichText::new("Dostępne instalatory dla Twojego systemu:").strong());

                    let mut start_download_target: Option<(String, String, u64)> = None;

                    for asset in &res.recommended_assets {
                        ui.horizontal(|ui| {
                            let btn_text = format!(
                                "⬇ Pobierz w aplikacji: {} ({})",
                                asset.label, asset.size_formatted
                            );
                            let btn = if asset.recommended {
                                egui::Button::new(
                                    RichText::new(btn_text).color(Color32::WHITE).strong(),
                                )
                                .fill(theme.c_btn_primary())
                            } else {
                                egui::Button::new(RichText::new(btn_text).color(theme.c_fg()))
                                    .fill(theme.c_btn_secondary())
                            };

                            // Disable button while a download is running
                            let enabled = !is_downloading;
                            if ui.add_enabled(enabled, btn).clicked() {
                                start_download_target = Some((
                                    asset.download_url.clone(),
                                    asset.name.clone(),
                                    asset.size_bytes,
                                ));
                            }
                        });
                    }

                    if let Some((url, name, size)) = start_download_target {
                        self.download = Some(start_download(&url, &name, Some(size)));
                    }

                    // 2. In-App Download Progress & Management
                    if let Some((_, ref prog)) = dl_snapshot {
                        ui.separator();
                        egui::Frame::none()
                            .fill(theme.c_bg_panel())
                            .inner_margin(10.0)
                            .show(ui, |ui| {
                                if !prog.finished && prog.error.is_none() {
                                    ui.label(
                                        RichText::new(format!(
                                            "⏳ Pobieranie: {}",
                                            prog.asset_name
                                        ))
                                        .strong()
                                        .size(13.0)
                                        .color(theme.c_fg()),
                                    );

                                    let total = prog.total_bytes.unwrap_or(0);
                                    let ratio = if total > 0 {
                                        (prog.downloaded_bytes as f32 / total as f32)
                                            .clamp(0.0, 1.0)
                                    } else {
                                        0.0
                                    };
                                    let pct = ratio * 100.0;
                                    let speed_str = format_speed(prog.speed_bytes_per_sec);
                                    let size_str = if total > 0 {
                                        format!(
                                            "{} / {}",
                                            format_size(prog.downloaded_bytes),
                                            format_size(total)
                                        )
                                    } else {
                                        format_size(prog.downloaded_bytes)
                                    };

                                    let progress_label = if total > 0 {
                                        format!("{:.1}% ({}) • {}", pct, size_str, speed_str)
                                    } else {
                                        format!("{} • {}", size_str, speed_str)
                                    };

                                    ui.horizontal(|ui| {
                                        ui.add(
                                            ProgressBar::new(ratio)
                                                .text(progress_label)
                                                .animate(true)
                                                .desired_width(ui.available_width() - 80.0),
                                        );

                                        if ui.button(tr("btn_cancel")).clicked() {
                                            if let Some(ref dl) = self.download {
                                                dl.cancel_flag.store(true, Ordering::Relaxed);
                                            }
                                            self.download = None;
                                        }
                                    });

                                    ui.label(
                                        RichText::new(format!(
                                            "Zapisywanie w: {}",
                                            prog.target_path.display()
                                        ))
                                        .size(11.0)
                                        .color(Color32::GRAY),
                                    );
                                } else if prog.finished {
                                    ui.label(
                                        RichText::new("✓ Pobieranie zakończone pomyślnie!")
                                            .color(Color32::from_rgb(100, 255, 100))
                                            .strong()
                                            .size(14.0),
                                    );
                                    ui.label(
                                        RichText::new(format!(
                                            "Plik: {}",
                                            prog.target_path.display()
                                        ))
                                        .size(11.0)
                                        .color(theme.c_fg()),
                                    );
                                    ui.add_space(6.0);
                                    ui.horizontal(|ui| {
                                        let run_btn = egui::Button::new(
                                            RichText::new("▶ Otwórz / Zainstaluj")
                                                .color(Color32::WHITE)
                                                .strong(),
                                        )
                                        .fill(theme.c_btn_primary());

                                        if ui.add(run_btn).clicked() {
                                            let _ = open::that(&prog.target_path);
                                        }

                                        if let Some(parent) = prog.target_path.parent() {
                                            if ui.button("📂 Pokaż w folderze").clicked() {
                                                let _ = open::that(parent);
                                            }
                                        }

                                        if ui.button("Pobierz ponownie").clicked() {
                                            self.download = None;
                                        }
                                    });
                                } else if let Some(ref err) = prog.error {
                                    ui.label(
                                        RichText::new(format!("❌ Błąd pobierania: {}", err))
                                            .color(Color32::from_rgb(255, 100, 100))
                                            .strong(),
                                    );
                                    ui.add_space(4.0);
                                    ui.horizontal(|ui| {
                                        if ui.button("Spróbuj ponownie").clicked() {
                                            self.download = None;
                                        }
                                        if ui.button("🌐 Pobierz w przeglądarce").clicked() {
                                            if let Some(asset) = res
                                                .recommended_assets
                                                .iter()
                                                .find(|a| a.name == prog.asset_name)
                                            {
                                                let _ = open::that(&asset.download_url);
                                            } else {
                                                let _ = open::that(&res.html_url);
                                            }
                                        }
                                    });
                                }
                            });
                    }
                } else {
                    ui.vertical_centered(|ui| {
                        ui.add_space(12.0);
                        ui.label(
                            RichText::new("✓")
                                .size(32.0)
                                .color(Color32::from_rgb(100, 255, 100)),
                        );
                        ui.label(
                            RichText::new("Twoja wersja programu jest aktualna!")
                                .size(15.0)
                                .strong()
                                .color(theme.c_fg()),
                        );
                        ui.add_space(8.0);
                    });

                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Zainstalowana wersja:").strong());
                        ui.label(format!("v{}", res.current_version));
                        ui.add_space(16.0);
                        if !res.latest_version.is_empty() {
                            ui.label(RichText::new("Najnowsza na GitHubie:").strong());
                            ui.label(
                                RichText::new(format!("v{}", res.latest_version))
                                    .color(Color32::from_rgb(100, 255, 100)),
                            );
                        }
                    });
                    ui.label("Nie znaleziono nowszych wydań.");
                    ui.add_space(12.0);
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
