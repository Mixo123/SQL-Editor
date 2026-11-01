//! About dialog.

use egui::{Align2, Color32, Context, Id, RichText, Vec2, Window};

use crate::i18n::tr;
use crate::theme::Theme;
use crate::version::{GITHUB_REPO, VERSION};

pub struct AboutDialog {
    pub open: bool,
    pub request_check_updates: bool,
}

impl AboutDialog {
    pub fn new() -> Self {
        Self {
            open: false,
            request_check_updates: false,
        }
    }

    pub fn show(&mut self, ctx: &Context, theme: &Theme) {
        if !self.open {
            return;
        }

        let mut close_dialog = false;

        Window::new(RichText::new(tr("about_title")).size(16.0).color(theme.c_fg()))
            .id(Id::new("about_modal"))
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .collapsible(false)
            .resizable(false)
            .default_width(460.0)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(8.0);
                    ui.label(RichText::new("SQL Editor").size(22.0).strong().color(Color32::WHITE));
                    ui.label(
                        RichText::new(format!("{} v{}", tr("about_version"), VERSION))
                            .size(13.0)
                            .color(Color32::from_rgb(133, 215, 255)),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(tr("about_desc"))
                            .size(11.0)
                            .color(Color32::from_rgb(200, 200, 200)),
                    );
                    ui.add_space(16.0);

                    // Check updates button
                    let check_btn = egui::Button::new(
                        RichText::new(tr("about_check_updates_btn"))
                            .color(Color32::WHITE)
                            .strong(),
                    )
                    .fill(theme.c_btn_primary());
                    if ui.add_sized(Vec2::new(300.0, 32.0), check_btn).clicked() {
                        self.request_check_updates = true;
                        close_dialog = true;
                    }

                    ui.add_space(6.0);

                    // GitHub link button
                    let gh_btn = egui::Button::new(RichText::new(tr("about_github_btn")).color(theme.c_fg()))
                        .fill(theme.c_btn_secondary());
                    if ui.add_sized(Vec2::new(300.0, 32.0), gh_btn).clicked() {
                        let _ = open::that(format!("https://github.com/{}/releases", GITHUB_REPO));
                    }

                    ui.add_space(10.0);

                    // Close button
                    if ui.button(tr("btn_close")).clicked() {
                        close_dialog = true;
                    }
                    ui.add_space(8.0);
                });
            });

        if close_dialog {
            self.open = false;
        }
    }
}
