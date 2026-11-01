//! Connection dialog (Add / Edit database connection).

use egui::{Align2, Color32, Context, Id, RichText, Vec2, Window};
use std::sync::mpsc::{channel, Receiver};
use std::thread;

use crate::db::types::ConnectionMeta;
use crate::db::{save_connections, test_connection};
use crate::i18n::{tr, tr_fmt};
use crate::theme::Theme;

pub struct ConnectionDialog {
    pub open: bool,
    pub is_editing: bool,
    pub original_name: String,
    pub meta: ConnectionMeta,
    pub test_rx: Option<Receiver<Result<(), String>>>,
    pub test_status: Option<Result<String, String>>,
    pub error_msg: Option<String>,
}

impl ConnectionDialog {
    pub fn new() -> Self {
        Self {
            open: false,
            is_editing: false,
            original_name: String::new(),
            meta: ConnectionMeta {
                name: String::new(),
                kind: "sqlite".to_string(),
                host: "localhost".to_string(),
                port: "5432".to_string(),
                db: String::new(),
                user: String::new(),
                password: String::new(),
                ssl: false,
                path: String::new(),
                project: String::new(),
                location: String::new(),
            },
            test_rx: None,
            test_status: None,
            error_msg: None,
        }
    }

    pub fn open_add(&mut self) {
        self.open = true;
        self.is_editing = false;
        self.original_name = String::new();
        self.meta = ConnectionMeta {
            name: String::new(),
            kind: "sqlite".to_string(),
            host: "localhost".to_string(),
            port: "5432".to_string(),
            db: String::new(),
            user: String::new(),
            password: String::new(),
            ssl: false,
            path: String::new(),
            project: String::new(),
            location: String::new(),
        };
        self.test_rx = None;
        self.test_status = None;
        self.error_msg = None;
    }

    pub fn open_edit(&mut self, meta: &ConnectionMeta) {
        self.open = true;
        self.is_editing = true;
        self.original_name = meta.name.clone();
        let mut copy = meta.clone();
        // show decrypted password in the dialog for editing
        copy.password = meta.get_decrypted_password();
        self.meta = copy;
        self.test_rx = None;
        self.test_status = None;
        self.error_msg = None;
    }

    pub fn show(
        &mut self,
        ctx: &Context,
        theme: &Theme,
        connections: &mut Vec<ConnectionMeta>,
        active_conn: &mut Option<String>,
    ) {
        if !self.open {
            return;
        }

        // Check async test status
        if let Some(ref rx) = self.test_rx {
            if let Ok(res) = rx.try_recv() {
                self.test_status = Some(match res {
                    Ok(_) => Ok(tr("msg_test_success")),
                    Err(e) => Err(e),
                });
                self.test_rx = None;
            }
        }

        let title = if self.is_editing {
            tr("conn_title_edit")
        } else {
            tr("conn_title_add")
        };

        let mut close_dialog = false;

        Window::new(RichText::new(&title).size(16.0).color(theme.c_fg()))
            .id(Id::new("conn_modal"))
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .collapsible(false)
            .resizable(false)
            .default_width(520.0)
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 10.0);

                // Kind selection
                ui.horizontal(|ui| {
                    ui.label(RichText::new(tr("lbl_conn_type")).strong());
                    let prev_kind = self.meta.kind.clone();
                    egui::ComboBox::from_id_salt("conn_kind_select")
                        .selected_text(&self.meta.kind)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.meta.kind, "sqlite".to_string(), "SQLite");
                            ui.selectable_value(&mut self.meta.kind, "postgres".to_string(), "PostgreSQL");
                            ui.selectable_value(&mut self.meta.kind, "mysql".to_string(), "MySQL");
                            ui.selectable_value(&mut self.meta.kind, "oracle".to_string(), "Oracle");
                            ui.selectable_value(&mut self.meta.kind, "mssql".to_string(), "Microsoft SQL Server");
                            ui.selectable_value(&mut self.meta.kind, "bigquery".to_string(), "Google BigQuery");
                        });

                    if self.meta.kind != prev_kind {
                        // Adjust default ports
                        if self.meta.kind == "postgres" && (self.meta.port.is_empty() || self.meta.port == "3306" || self.meta.port == "1521" || self.meta.port == "1433") {
                            self.meta.port = "5432".to_string();
                        } else if self.meta.kind == "mysql" {
                            self.meta.port = "3306".to_string();
                        } else if self.meta.kind == "oracle" {
                            self.meta.port = "1521".to_string();
                        } else if self.meta.kind == "mssql" {
                            self.meta.port = "1433".to_string();
                        }
                    }
                });

                // Name field
                ui.horizontal(|ui| {
                    ui.label(RichText::new(tr("lbl_conn_name")).strong());
                    ui.text_edit_singleline(&mut self.meta.name);
                });

                // Specific fields depending on kind
                match self.meta.kind.as_str() {
                    "sqlite" => {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(tr("lbl_db_file")).strong());
                            ui.text_edit_singleline(&mut self.meta.path);
                            if ui.button("📂 Przeglądaj…").clicked() {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("SQLite Database", &["db", "sqlite", "sqlite3"])
                                    .pick_file()
                                {
                                    self.meta.path = path.to_string_lossy().to_string();
                                    if self.meta.name.is_empty() {
                                        if let Some(stem) = path.file_stem() {
                                            self.meta.name = stem.to_string_lossy().to_string();
                                        }
                                    }
                                }
                            }
                        });
                    }
                    "bigquery" => {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(tr("lbl_project_id")).strong());
                            ui.text_edit_singleline(&mut self.meta.project);
                        });
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(tr("lbl_key_json")).strong());
                            ui.text_edit_singleline(&mut self.meta.path);
                            if ui.button("📂…").clicked() {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("JSON Key", &["json"])
                                    .pick_file()
                                {
                                    self.meta.path = path.to_string_lossy().to_string();
                                }
                            }
                        });
                        ui.label(RichText::new(tr("hint_bq_auth")).size(11.0).italics().color(Color32::GRAY));
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(tr("lbl_default_dataset")).strong());
                            ui.text_edit_singleline(&mut self.meta.db);
                        });
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(tr("lbl_location")).strong());
                            ui.text_edit_singleline(&mut self.meta.location);
                        });
                    }
                    _ => {
                        // Host & Port
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(tr("lbl_host")).strong());
                            ui.text_edit_singleline(&mut self.meta.host);
                            ui.label(RichText::new(tr("lbl_port")).strong());
                            ui.text_edit_singleline(&mut self.meta.port);
                        });

                        // Database / Service
                        ui.horizontal(|ui| {
                            let db_lbl = if self.meta.kind == "oracle" {
                                tr("lbl_service_sid")
                            } else {
                                tr("lbl_database")
                            };
                            ui.label(RichText::new(db_lbl).strong());
                            ui.text_edit_singleline(&mut self.meta.db);
                        });

                        // User & Password
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(tr("lbl_user")).strong());
                            ui.text_edit_singleline(&mut self.meta.user);
                            ui.label(RichText::new(tr("lbl_password")).strong());
                            ui.add(egui::TextEdit::singleline(&mut self.meta.password).password(true));
                        });

                        // SSL Checkbox
                        let ssl_lbl = if self.meta.kind == "oracle" {
                            tr("chk_require_ssl_tcps")
                        } else if self.meta.kind == "mssql" {
                            tr("chk_require_ssl_encrypt")
                        } else {
                            tr("chk_require_ssl")
                        };
                        ui.checkbox(&mut self.meta.ssl, ssl_lbl);
                    }
                }

                // Error / Test status feedback
                if let Some(ref err) = self.error_msg {
                    ui.label(RichText::new(err).color(Color32::from_rgb(255, 100, 100)));
                }
                if let Some(ref st) = self.test_status {
                    match st {
                        Ok(msg) => {
                            ui.label(RichText::new(format!("✓ {}", msg)).color(Color32::from_rgb(100, 255, 100)));
                        }
                        Err(err) => {
                            ui.label(RichText::new(format!("✗ {}", err)).color(Color32::from_rgb(255, 100, 100)));
                        }
                    }
                }
                if self.test_rx.is_some() {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Testowanie połączenia...");
                    });
                }

                ui.separator();

                // Buttons row
                ui.horizontal(|ui| {
                    if self.is_editing {
                        let del_btn = egui::Button::new(RichText::new(tr("btn_delete")).color(Color32::WHITE))
                            .fill(Color32::from_rgb(150, 40, 40));
                        if ui.add(del_btn).clicked() {
                            connections.retain(|c| c.name != self.original_name);
                            if active_conn.as_deref() == Some(&self.original_name) {
                                *active_conn = None;
                            }
                            let _ = save_connections(connections);
                            close_dialog = true;
                        }
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Save Button
                        let save_btn = egui::Button::new(RichText::new(tr("btn_save")).color(Color32::WHITE).strong())
                            .fill(theme.c_btn_primary());
                        if ui.add(save_btn).clicked() {
                            let name = self.meta.name.trim().to_string();
                            if name.is_empty() {
                                self.error_msg = Some(tr("msg_name_required"));
                                return;
                            }
                            // Check unique name if new or renamed
                            if (!self.is_editing || name != self.original_name)
                                && connections.iter().any(|c| c.name == name)
                            {
                                self.error_msg = Some(tr_fmt("msg_conn_exists", &[("name", &name)]));
                                return;
                            }

                            let mut final_meta = self.meta.clone();
                            final_meta.name = name.clone();
                            // encrypt password before storing
                            let plain_pwd = self.meta.password.clone();
                            final_meta.set_password_plain(&plain_pwd);

                            if self.is_editing {
                                if let Some(idx) = connections.iter().position(|c| c.name == self.original_name) {
                                    connections[idx] = final_meta;
                                }
                            } else {
                                connections.push(final_meta);
                            }

                            if active_conn.is_none() || self.is_editing {
                                *active_conn = Some(name);
                            }

                            let _ = save_connections(connections);
                            close_dialog = true;
                        }

                        // Cancel Button
                        if ui.button(tr("btn_cancel")).clicked() {
                            close_dialog = true;
                        }

                        // Test Button
                        if ui.button(tr("btn_test")).clicked() {
                            self.error_msg = None;
                            self.test_status = None;
                            let mut test_meta = self.meta.clone();
                            test_meta.set_password_plain(&self.meta.password);
                            let (tx, rx) = channel();
                            self.test_rx = Some(rx);
                            thread::spawn(move || {
                                let res = test_connection(&test_meta);
                                let _ = tx.send(res);
                            });
                        }
                    });
                });
            });

        if close_dialog {
            self.open = false;
        }
    }
}
