//! Main Application UI and State Management.

#![allow(dead_code)]

use egui::{
    menu, Align, Align2, Color32, Context, FontId, Key, Layout, Pos2, Rect,
    RichText, ScrollArea, Sense, Stroke, TextStyle, Ui, Vec2,
};
use egui_extras::{Column, TableBuilder};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::time::Instant;

use crate::autocomplete::{get_autocomplete_suggestions, Suggestion};
use crate::config::*;
use crate::db::types::{is_unbounded_select, split_sql_statements, ConnectionMeta, SchemaMap};
use crate::db::{get_schema, load_connections, save_connections, stream_query};
use crate::dialogs::{
    load_settings, AboutDialog, AppSettings, ConnectionDialog, SettingsDialog,
    UpdateDialog,
};
use crate::export::{export_csv, export_json, export_markdown, export_xlsx};
use crate::highlighter::highlight_sql;
use crate::i18n::{set_language, tr, tr_fmt};
use crate::theme::{get_theme, Theme};
use crate::updater::{check_for_updates_async, UpdateCheckResult};

#[derive(Debug, Clone)]
pub struct EditorTab {
    pub id: usize,
    pub name: String,
    pub path: Option<PathBuf>,
    pub content: String,
    pub modified: bool,
}

#[derive(Debug, Clone)]
pub struct QueryResult {
    pub title: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Option<String>>>,
    pub fetched_count: usize,
    pub elapsed_secs: f64,
    pub finished: bool,
    pub error: Option<String>,
}

pub enum QueryMsg {
    Header(Vec<String>),
    Batch(Vec<Vec<Option<String>>>),
    Finished(f64),
    Error(String),
}

pub struct SqlEditorApp {
    // Tabs & Editor
    pub tabs: Vec<EditorTab>,
    pub active_tab_id: usize,
    pub next_tab_id: usize,
    pub editor_height: f32,

    // Database & Connections
    pub connections: Vec<ConnectionMeta>,
    pub active_conn_name: Option<String>,
    pub schema_cache: HashMap<String, SchemaMap>,
    pub schema_rx: Option<Receiver<(String, SchemaMap)>>,

    // Query Results
    pub query_results: Vec<QueryResult>,
    pub active_result_idx: usize,
    pub busy: bool,
    pub status_text: String,
    pub query_rx: Option<Receiver<QueryMsg>>,
    pub cancel_sender: Option<Sender<()>>,
    pub query_start_time: Option<Instant>,

    // Autocomplete popup state
    pub show_autocomplete: bool,
    pub suggestions: Vec<Suggestion>,
    pub selected_suggestion: usize,
    pub autocomplete_prefix_len: usize,

    // Unsaved changes confirm dialog
    pub confirm_close_dirty: Option<usize>, // tab id to close after confirm
    pub confirm_quit: bool,
    pub confirm_unbounded: Option<Vec<String>>, // statements waiting for unbounded execution approval

    // Context menus
    pub conn_context_menu: Option<(String, Pos2)>,
    pub selected_table_cell: Option<(usize, usize)>, // (row_idx, col_idx)

    // Settings & Appearance
    pub settings: AppSettings,
    pub theme: Theme,

    // Dialogs
    pub conn_dialog: ConnectionDialog,
    pub settings_dialog: SettingsDialog,
    pub about_dialog: AboutDialog,
    pub update_dialog: UpdateDialog,

    // Background update check
    pub update_rx: Option<Receiver<UpdateCheckResult>>,
}

impl SqlEditorApp {
    pub fn new(_cc: &eframe::CreationContext) -> Self {
        let settings = load_settings();
        set_language(&settings.language);
        let theme = get_theme(&settings.theme_name, &settings.custom_theme_colors);
        let connections = load_connections();

        let initial_tab = EditorTab {
            id: 1,
            name: format!("{} 1", tr("tab_untitled")),
            path: None,
            content: "-- Write your SQL query here\nSELECT 1;\n".to_string(),
            modified: false,
        };

        let mut app = Self {
            tabs: vec![initial_tab],
            active_tab_id: 1,
            next_tab_id: 2,
            editor_height: 380.0,
            connections,
            active_conn_name: None,
            schema_cache: HashMap::new(),
            schema_rx: None,
            query_results: Vec::new(),
            active_result_idx: 0,
            busy: false,
            status_text: tr("status_ready"),
            query_rx: None,
            cancel_sender: None,
            query_start_time: None,
            show_autocomplete: false,
            suggestions: Vec::new(),
            selected_suggestion: 0,
            autocomplete_prefix_len: 0,
            confirm_close_dirty: None,
            confirm_quit: false,
            confirm_unbounded: None,
            conn_context_menu: None,
            selected_table_cell: None,
            settings: settings.clone(),
            theme,
            conn_dialog: ConnectionDialog::new(),
            settings_dialog: SettingsDialog::new(),
            about_dialog: AboutDialog::new(),
            update_dialog: UpdateDialog::new(),
            update_rx: None,
        };

        if app.settings.check_updates {
            app.update_rx = Some(check_for_updates_async());
        }

        // Auto select first connection if available
        let first_conn = app.connections.first().map(|c| c.name.clone());
        if let Some(name) = first_conn {
            app.select_connection(&name);
        }

        app
    }

    pub fn fetch_schema(&mut self, name: &str) {
        if let Some(conn) = self.connections.iter().find(|c| c.name == name).cloned() {
            let (tx, rx) = channel();
            self.schema_rx = Some(rx);
            thread::spawn(move || {
                let schema = get_schema(&conn).unwrap_or_default();
                let _ = tx.send((conn.name, schema));
            });
        }
    }

    pub fn select_connection(&mut self, name: &str) {
        self.active_conn_name = Some(name.to_string());
        if !self.schema_cache.contains_key(name) {
            self.fetch_schema(name);
        }
    }

    pub fn new_tab(&mut self) {
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        self.tabs.push(EditorTab {
            id,
            name: format!("{} {}", tr("tab_untitled"), id),
            path: None,
            content: String::new(),
            modified: false,
        });
        self.active_tab_id = id;
    }

    pub fn close_tab(&mut self, id: usize) {
        if let Some(idx) = self.tabs.iter().position(|t| t.id == id) {
            if self.tabs[idx].modified {
                self.confirm_close_dirty = Some(id);
                return;
            }
            self.tabs.remove(idx);
            if self.tabs.is_empty() {
                self.new_tab();
            } else if self.active_tab_id == id {
                let new_idx = idx.min(self.tabs.len() - 1);
                self.active_tab_id = self.tabs[new_idx].id;
            }
        }
    }

    pub fn active_tab_mut(&mut self) -> Option<&mut EditorTab> {
        let id = self.active_tab_id;
        self.tabs.iter_mut().find(|t| t.id == id)
    }

    pub fn active_tab(&self) -> Option<&EditorTab> {
        let id = self.active_tab_id;
        self.tabs.iter().find(|t| t.id == id)
    }

    pub fn open_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("SQL Files", &["sql"])
            .add_filter("All Files", &["*"])
            .pick_file()
        {
            if let Ok(content) = fs::read_to_string(&path) {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| tr("tab_untitled"));
                let id = self.next_tab_id;
                self.next_tab_id += 1;
                self.tabs.push(EditorTab {
                    id,
                    name,
                    path: Some(path),
                    content,
                    modified: false,
                });
                self.active_tab_id = id;
            }
        }
    }

    pub fn save_current_file(&mut self) {
        let Some(tab) = self.active_tab_mut() else { return };
        if let Some(ref path) = tab.path {
            if fs::write(path, &tab.content).is_ok() {
                tab.modified = false;
                self.status_text = format!("Zapisano {}", path.display());
            }
        } else {
            self.save_file_as();
        }
    }

    pub fn save_file_as(&mut self) {
        let Some(tab) = self.active_tab_mut() else { return };
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("SQL Files", &["sql"])
            .set_file_name(&tab.name)
            .save_file()
        {
            if fs::write(&path, &tab.content).is_ok() {
                tab.name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| tr("tab_untitled"));
                tab.path = Some(path.clone());
                tab.modified = false;
                self.status_text = format!("Zapisano {}", path.display());
            }
        }
    }

    pub fn run_sql(&mut self, _selection_only: bool) {
        if self.busy {
            return;
        }
        if self.active_conn_name.is_none() {
            self.status_text = tr("msg_no_connection");
            return;
        }
        let Some(tab) = self.active_tab() else {
            return;
        };

        let sql = tab.content.trim().to_string();
        let statements = split_sql_statements(&sql);

        if statements.is_empty() {
            self.status_text = tr("msg_no_query");
            return;
        }

        // Check unbounded select warning
        if self.settings.warn_unbounded_select && statements.iter().any(|s| is_unbounded_select(s)) {
            self.confirm_unbounded = Some(statements);
            return;
        }

        self.execute_statements(statements);
    }

    pub fn execute_statements(&mut self, statements: Vec<String>) {
        let Some(conn_name) = self.active_conn_name.clone() else { return };
        let Some(conn_meta) = self.connections.iter().find(|c| c.name == conn_name).cloned() else { return };

        self.busy = true;
        self.status_text = tr("status_executing");
        self.query_start_time = Some(Instant::now());

        // Reset query results
        self.query_results.clear();
        self.active_result_idx = 0;

        for (i, _) in statements.iter().enumerate() {
            self.query_results.push(QueryResult {
                title: format!("{} {}", tr("result_tab_title"), i + 1),
                columns: Vec::new(),
                rows: Vec::new(),
                fetched_count: 0,
                elapsed_secs: 0.0,
                finished: false,
                error: None,
            });
        }

        let (tx, rx) = channel();
        self.query_rx = Some(rx);

        thread::spawn(move || {
            let start = Instant::now();
            for stmt in &statements {
                let tx_clone = tx.clone();
                let res = stream_query(
                    &conn_meta,
                    stmt,
                    MAX_FETCH_ROWS,
                    |cols, rows| {
                        if let Some(c) = cols {
                            let _ = tx_clone.send(QueryMsg::Header(c));
                        }
                        if !rows.is_empty() {
                            let _ = tx_clone.send(QueryMsg::Batch(rows));
                        }
                    },
                );

                if let Err(e) = res {
                    let _ = tx.send(QueryMsg::Error(e));
                    break;
                }
            }
            let elapsed = start.elapsed().as_secs_f64();
            let _ = tx.send(QueryMsg::Finished(elapsed));
        });
    }

    pub fn poll_query_results(&mut self) {
        let mut finished = false;
        if let Some(ref rx) = self.query_rx {
            while let Ok(msg) = rx.try_recv() {
                let curr_res = self.query_results.get_mut(self.active_result_idx);
                match msg {
                    QueryMsg::Header(cols) => {
                        if let Some(res) = curr_res {
                            res.columns = cols;
                        }
                    }
                    QueryMsg::Batch(rows) => {
                        if let Some(res) = curr_res {
                            res.fetched_count += rows.len();
                            res.rows.extend(rows);
                        }
                    }
                    QueryMsg::Error(err) => {
                        if let Some(res) = curr_res {
                            res.error = Some(err.clone());
                            res.columns = vec!["Error".to_string()];
                            res.rows = vec![vec![Some(err)]];
                            res.finished = true;
                        }
                        self.status_text = "Błąd wykonania zapytania".to_string();
                    }
                    QueryMsg::Finished(elapsed) => {
                        if let Some(res) = curr_res {
                            res.finished = true;
                            res.elapsed_secs = elapsed;
                            self.status_text = format!(
                                "{} row(s) • {} col(s) • {:.2}s",
                                res.fetched_count,
                                res.columns.len(),
                                elapsed
                            );
                        }
                        finished = true;
                    }
                }
            }
        }
        if finished {
            self.busy = false;
            self.query_rx = None;
            if let Some(conn_name) = self.active_conn_name.clone() {
                self.fetch_schema(&conn_name);
            }
        }

        // Check background schema results
        if let Some(ref rx) = self.schema_rx {
            if let Ok((name, schema)) = rx.try_recv() {
                self.schema_cache.insert(name, schema);
                self.schema_rx = None;
            }
        }

        // Check update results
        if let Some(ref rx) = self.update_rx {
            if let Ok(res) = rx.try_recv() {
                if res.update_available {
                    self.update_dialog.show_result(res);
                }
                self.update_rx = None;
            }
        }
    }

    pub fn insert_autocomplete_suggestion(&mut self, text_edit_id: egui::Id, ctx: &Context) {
        let prefix_len = self.autocomplete_prefix_len;
        if let Some(sug) = self.suggestions.get(self.selected_suggestion).cloned() {
            if let Some(tab) = self.active_tab_mut() {
                let cursor_idx = egui::text_edit::TextEditState::load(ctx, text_edit_id)
                    .and_then(|s| s.cursor.char_range())
                    .map(|r| r.primary.index)
                    .unwrap_or(tab.content.len());

                let text_before: String = tab.content.chars().take(cursor_idx).collect();
                let before_prefix: String = text_before
                    .chars()
                    .take(text_before.chars().count().saturating_sub(prefix_len))
                    .collect();
                let text_after: String = tab.content.chars().skip(cursor_idx).collect();

                tab.content = format!("{}{}{}", before_prefix, sug.text, text_after);
                tab.modified = true;

                let new_cursor = before_prefix.chars().count() + sug.text.chars().count();
                if let Some(mut state) = egui::text_edit::TextEditState::load(ctx, text_edit_id) {
                    state.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(new_cursor))));
                    state.store(ctx, text_edit_id);
                }
            }
        }
        self.show_autocomplete = false;
    }

    pub fn export_active_result(&mut self, format_type: &str) {
        let Some(res) = self.query_results.get(self.active_result_idx) else {
            self.status_text = tr("msg_export_no_results");
            return;
        };

        if res.rows.is_empty() {
            self.status_text = tr("msg_export_no_results");
            return;
        }

        let (ext, filter_name) = match format_type {
            "csv" => ("csv", "CSV File"),
            "json" => ("json", "JSON File"),
            "markdown" => ("md", "Markdown File"),
            "xlsx" => ("xlsx", "Excel Workbook"),
            _ => ("csv", "CSV File"),
        };

        if let Some(path) = rfd::FileDialog::new()
            .add_filter(filter_name, &[ext])
            .set_file_name(&format!("results.{}", ext))
            .save_file()
        {
            let outcome = match format_type {
                "csv" => export_csv(&path, &res.columns, &res.rows),
                "json" => export_json(&path, &res.columns, &res.rows),
                "markdown" => export_markdown(&path, &res.columns, &res.rows),
                "xlsx" => export_xlsx(&path, &res.columns, &res.rows),
                _ => export_csv(&path, &res.columns, &res.rows),
            };

            match outcome {
                Ok(_) => {
                    self.status_text = format!("Wyeksportowano {} wierszy do {}", res.rows.len(), path.display());
                }
                Err(e) => {
                    self.status_text = format!("{}: {}", tr("msg_export_failed"), e);
                }
            }
        }
    }
}

impl eframe::App for SqlEditorApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.poll_query_results();

        let theme = self.theme.clone();

        let text_edit_id = egui::Id::new("sql_main_editor");

        // Autocomplete keyboard handling
        let mut do_insert_autocomplete = false;
        if self.show_autocomplete && !self.suggestions.is_empty() {
            ctx.input(|i| {
                if i.key_pressed(Key::ArrowDown) {
                    self.selected_suggestion = (self.selected_suggestion + 1) % self.suggestions.len();
                } else if i.key_pressed(Key::ArrowUp) {
                    self.selected_suggestion = (self.selected_suggestion + self.suggestions.len() - 1) % self.suggestions.len();
                } else if (i.key_pressed(Key::Enter) && !i.modifiers.ctrl && !i.modifiers.command) || i.key_pressed(Key::Tab) {
                    do_insert_autocomplete = true;
                } else if i.key_pressed(Key::Escape) {
                    self.show_autocomplete = false;
                }
            });
        }

        if do_insert_autocomplete {
            self.insert_autocomplete_suggestion(text_edit_id, ctx);
        }

        // Keyboard shortcuts
        ctx.input(|i| {
            if (i.modifiers.command || i.modifiers.ctrl) && i.key_pressed(Key::Enter) {
                self.run_sql(false);
            }
            if i.key_pressed(Key::F5) {
                self.run_sql(false);
            }
            if (i.modifiers.command || i.modifiers.ctrl) && i.key_pressed(Key::S) {
                if i.modifiers.shift {
                    self.save_file_as();
                } else {
                    self.save_current_file();
                }
            }
            if (i.modifiers.command || i.modifiers.ctrl) && i.key_pressed(Key::O) {
                self.open_file();
            }
            if (i.modifiers.command || i.modifiers.ctrl) && i.key_pressed(Key::T) {
                self.new_tab();
            }
            if (i.modifiers.command || i.modifiers.ctrl) && i.key_pressed(Key::W) {
                self.close_tab(self.active_tab_id);
            }
            if (i.modifiers.command || i.modifiers.ctrl) && i.key_pressed(Key::E) {
                self.export_active_result("csv");
            }
            // Zoom editor font with Ctrl/Cmd + Scroll
            if i.modifiers.command || i.modifiers.ctrl {
                if i.raw_scroll_delta.y > 0.0 && self.settings.editor_font_size < 32 {
                    self.settings.editor_font_size += 1;
                } else if i.raw_scroll_delta.y < 0.0 && self.settings.editor_font_size > 8 {
                    self.settings.editor_font_size -= 1;
                }
            }
        });

        // 1. Top Menu Bar
        egui::TopBottomPanel::top("top_menu_panel")
            .frame(egui::Frame::none().fill(theme.c_bg_header()))
            .show(ctx, |ui| {
                menu::bar(ui, |ui| {
                    ui.menu_button(tr("menu_export"), |ui| {
                        if ui.button(tr("export_csv")).clicked() {
                            self.export_active_result("csv");
                            ui.close_menu();
                        }
                        if ui.button(tr("export_json")).clicked() {
                            self.export_active_result("json");
                            ui.close_menu();
                        }
                        if ui.button(tr("export_xlsx")).clicked() {
                            self.export_active_result("xlsx");
                            ui.close_menu();
                        }
                        if ui.button(tr("export_markdown")).clicked() {
                            self.export_active_result("markdown");
                            ui.close_menu();
                        }
                    });

                    ui.menu_button(tr("menu_settings"), |ui| {
                        if ui.button(tr("menu_preferences")).clicked() {
                            self.settings_dialog.open_dialog(&self.settings);
                            ui.close_menu();
                        }
                    });

                    ui.menu_button(tr("menu_help"), |ui| {
                        if ui.button(tr("menu_check_updates")).clicked() {
                            let res = crate::updater::check_for_updates();
                            self.update_dialog.show_result(res);
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button(tr("menu_about")).clicked() {
                            self.about_dialog.open = true;
                            ui.close_menu();
                        }
                    });
                });
            });

        // 2. Toolbar
        egui::TopBottomPanel::top("toolbar_panel")
            .frame(egui::Frame::none().fill(theme.c_bg_header()).inner_margin(6.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);

                    // Run button (Primary Accent)
                    let run_btn = egui::Button::new(
                        RichText::new(tr("btn_run"))
                            .color(Color32::WHITE)
                            .strong()
                            .size(13.0),
                    )
                    .fill(theme.c_btn_primary());
                    if ui.add(run_btn).clicked() {
                        self.run_sql(false);
                    }

                    // Save button
                    let save_btn = egui::Button::new(RichText::new(tr("btn_save")).color(theme.c_fg()))
                        .fill(theme.c_btn_secondary());
                    if ui.add(save_btn).clicked() {
                        self.save_current_file();
                    }

                    // Save As button
                    let save_as_btn = egui::Button::new(RichText::new(tr("btn_save_as")).color(theme.c_fg()))
                        .fill(theme.c_btn_secondary());
                    if ui.add(save_as_btn).clicked() {
                        self.save_file_as();
                    }

                    ui.separator();

                    // Connection selector dropdown
                    ui.label(RichText::new(tr("lbl_connection")).color(theme.c_fg()).strong());
                    let current_conn_name = self.active_conn_name.as_deref().unwrap_or("—");
                    egui::ComboBox::from_id_salt("toolbar_conn_select")
                        .selected_text(current_conn_name)
                        .show_ui(ui, |ui| {
                            let names: Vec<String> = self.connections.iter().map(|c| c.name.clone()).collect();
                            for name in names {
                                if ui.selectable_label(self.active_conn_name.as_deref() == Some(&name), &name).clicked() {
                                    self.select_connection(&name);
                                }
                            }
                        });

                    if self.busy {
                        ui.spinner();
                        ui.label(RichText::new("Wykonywanie…").color(Color32::from_rgb(255, 200, 100)));
                    }
                });
            });

        // 3. Status Bar at Bottom
        egui::TopBottomPanel::bottom("status_bar_panel")
            .frame(egui::Frame::none().fill(theme.c_bg_panel()).inner_margin(4.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&self.status_text).size(11.0).color(theme.c_fg()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("Font: {}pt", self.settings.editor_font_size))
                                .size(11.0)
                                .color(Color32::GRAY),
                        );
                    });
                });
            });

        // 4. Left Sidebar (Connections & Open Files)
        egui::SidePanel::left("left_sidebar_panel")
            .default_width(240.0)
            .width_range(160.0..=400.0)
            .frame(egui::Frame::none().fill(theme.c_bg_panel()).inner_margin(8.0))
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(0.0, 6.0);

                // Section 1: Connections Header
                ui.horizontal(|ui| {
                    ui.label(RichText::new(tr("hdr_connections")).strong().size(13.0).color(theme.c_fg()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("➕").clicked() {
                            self.conn_dialog.open_add();
                        }
                    });
                });

                // Connections List
                ScrollArea::vertical()
                    .id_salt("sidebar_conn_scroll")
                    .max_height(220.0)
                    .show(ui, |ui| {
                        let names_kinds: Vec<(String, String)> = self
                            .connections
                            .iter()
                            .map(|c| (c.name.clone(), c.kind.clone()))
                            .collect();

                        for (name, kind) in names_kinds {
                            let icon = match kind.as_str() {
                                "sqlite" => "🗄",
                                "postgres" | "postgresql" => "🐘",
                                "mysql" => "🐬",
                                "oracle" => "🔴",
                                "mssql" => "🪟",
                                "bigquery" => "☁️",
                                _ => "🔌",
                            };
                            let label_text = format!("{}  {}", icon, name);
                            let is_selected = self.active_conn_name.as_deref() == Some(&name);

                            let resp = ui.selectable_label(is_selected, label_text);
                            if resp.clicked() {
                                self.select_connection(&name);
                            }
                            resp.context_menu(|ui| {
                                if ui.button(tr_fmt("ctx_edit_conn", &[("name", &name)])).clicked() {
                                    if let Some(conn) = self.connections.iter().find(|c| c.name == name) {
                                        self.conn_dialog.open_edit(conn);
                                    }
                                    ui.close_menu();
                                }
                                if ui.button(tr_fmt("ctx_delete_conn", &[("name", &name)])).clicked() {
                                    self.connections.retain(|c| c.name != name);
                                    if self.active_conn_name.as_deref() == Some(&name) {
                                        self.active_conn_name = None;
                                    }
                                    let _ = save_connections(&self.connections);
                                    ui.close_menu();
                                }
                            });
                        }
                    });

                ui.separator();

                // Section 2: Open Files Header
                ui.horizontal(|ui| {
                    ui.label(RichText::new(tr("hdr_open_files")).strong().size(13.0).color(theme.c_fg()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("➕").clicked() {
                            self.new_tab();
                        }
                        if ui.button("📂").clicked() {
                            self.open_file();
                        }
                    });
                });

                // Open Files List
                ScrollArea::vertical()
                    .id_salt("sidebar_files_scroll")
                    .show(ui, |ui| {
                        let tab_info: Vec<(usize, String, bool)> = self
                            .tabs
                            .iter()
                            .map(|t| (t.id, t.name.clone(), t.modified))
                            .collect();

                        for (id, name, modified) in tab_info {
                            let dirty_str = if modified { " *" } else { "" };
                            let label_text = format!("📄  {}{}", name, dirty_str);
                            let is_active = self.active_tab_id == id;

                            ui.horizontal(|ui| {
                                if ui.selectable_label(is_active, label_text).clicked() {
                                    self.active_tab_id = id;
                                }
                            });
                        }
                    });
            });

        // 5. Central Area (Split: SQL Editor on Top, Results Table on Bottom)
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(theme.c_bg()))
            .show(ctx, |ui| {
                let total_height = ui.available_height();
                let available_for_editor = (total_height - 100.0).max(120.0);
                self.editor_height = self.editor_height.clamp(120.0, available_for_editor);

                // Top: Tabs Bar + Code Editor
                ui.vertical(|ui| {
                    ui.set_height(self.editor_height);

                    // Tab bar
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
                        let tab_headers: Vec<(usize, String, bool)> = self
                            .tabs
                            .iter()
                            .map(|t| (t.id, t.name.clone(), t.modified))
                            .collect();

                        for (id, name, modified) in tab_headers {
                            let dirty = if modified { "*" } else { "" };
                            let is_active = self.active_tab_id == id;
                            let title = format!(" {}{} ", name, dirty);

                            let bg_color = if is_active {
                                theme.c_bg()
                            } else {
                                theme.c_bg_header()
                            };

                            egui::Frame::none()
                                .fill(bg_color)
                                .inner_margin(Vec2::new(6.0, 4.0))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        if ui.selectable_label(is_active, RichText::new(title).color(theme.c_fg())).clicked() {
                                            self.active_tab_id = id;
                                        }
                                        if ui.small_button("×").clicked() {
                                            self.close_tab(id);
                                        }
                                    });
                                });
                        }

                        if ui.button("+").clicked() {
                            self.new_tab();
                        }
                    });

                    ui.separator();

                    // Text Editor Area
                    let font_size = self.settings.editor_font_size as f32;
                    let font_id = FontId::monospace(font_size);
                    let syntax_on = self.settings.syntax_highlighting;

                    let mut content_to_edit = if let Some(tab) = self.active_tab() {
                        tab.content.clone()
                    } else {
                        String::new()
                    };

                    let mut layouter = |ui: &Ui, text: &str, _wrap_width: f32| {
                        let job = highlight_sql(text, &theme, font_id.clone(), syntax_on);
                        ui.fonts(|f| f.layout_job(job))
                    };

                    let text_edit = egui::TextEdit::multiline(&mut content_to_edit)
                        .id(text_edit_id)
                        .font(TextStyle::Monospace)
                        .desired_width(f32::INFINITY)
                        .desired_rows(14)
                        .layouter(&mut layouter);

                    let response = ScrollArea::vertical()
                        .id_salt("editor_scroll_area")
                        .show(ui, |ui| ui.add(text_edit));

                    let cursor_idx = egui::text_edit::TextEditState::load(ui.ctx(), text_edit_id)
                        .and_then(|s| s.cursor.char_range())
                        .map(|r| r.primary.index)
                        .unwrap_or(content_to_edit.len());

                    let text_before: String = content_to_edit.chars().take(cursor_idx).collect();

                    if response.inner.changed() {
                        if let Some(tab) = self.active_tab_mut() {
                            tab.content = content_to_edit.clone();
                            tab.modified = true;
                        }

                        // Trigger Autocomplete
                        if self.settings.autocomplete {
                            let empty_schema = HashMap::new();
                            let schema = self.active_conn_name
                                .as_deref()
                                .and_then(|n| self.schema_cache.get(n))
                                .unwrap_or(&empty_schema);

                            let (suggestions, prefix_len) = get_autocomplete_suggestions(
                                &text_before,
                                schema,
                                Some(&content_to_edit),
                            );
                            if !suggestions.is_empty() {
                                self.suggestions = suggestions;
                                self.selected_suggestion = 0;
                                self.autocomplete_prefix_len = prefix_len;
                                self.show_autocomplete = true;
                            } else {
                                self.show_autocomplete = false;
                            }
                        }
                    }

                    // Autocomplete popup floating window
                    if self.show_autocomplete && !self.suggestions.is_empty() {
                        let mut insert_idx: Option<usize> = None;

                        egui::Window::new("Autocomplete")
                            .title_bar(false)
                            .resizable(false)
                            .default_width(320.0)
                            .max_height(240.0)
                            .anchor(Align2::LEFT_TOP, Vec2::new(140.0, 190.0))
                            .show(ctx, |ui| {
                                egui::Frame::canvas(ui.style())
                                    .fill(theme.c_bg_panel())
                                    .stroke(Stroke::new(1.0_f32, theme.c_bg_header()))
                                    .inner_margin(6.0)
                                    .show(ui, |ui| {
                                        ScrollArea::vertical().show(ui, |ui| {
                                            for (i, sug) in self.suggestions.iter().enumerate() {
                                                let is_sel = i == self.selected_suggestion;
                                                let (icon, tag_color, tag_text) = match sug.kind.as_str() {
                                                    "table" => ("🗄", Color32::from_rgb(100, 220, 120), "[table]"),
                                                    "column" => ("🏷", Color32::from_rgb(100, 200, 255), "[column]"),
                                                    "function" => ("ƒ", Color32::from_rgb(220, 130, 255), "[function]"),
                                                    _ => ("🔑", Color32::from_rgb(255, 180, 80), "[keyword]"),
                                                };

                                                let bg_col = if is_sel {
                                                    theme.c_select_bg()
                                                } else {
                                                    Color32::TRANSPARENT
                                                };

                                                let row_resp = egui::Frame::none()
                                                    .fill(bg_col)
                                                    .inner_margin(Vec2::new(6.0, 3.0))
                                                    .show(ui, |ui| {
                                                        ui.horizontal(|ui| {
                                                            ui.label(RichText::new(icon).size(13.0));
                                                            ui.label(
                                                                RichText::new(&sug.text)
                                                                    .monospace()
                                                                    .strong()
                                                                    .color(theme.c_fg())
                                                                    .size(12.0),
                                                            );
                                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                                ui.label(
                                                                    RichText::new(tag_text)
                                                                        .monospace()
                                                                        .size(11.0)
                                                                        .color(tag_color),
                                                                );
                                                            });
                                                        });
                                                    });

                                                if row_resp.response.interact(Sense::click()).clicked() {
                                                    insert_idx = Some(i);
                                                }
                                            }
                                        });
                                    });
                            });

                        if let Some(idx) = insert_idx {
                            self.selected_suggestion = idx;
                            self.insert_autocomplete_suggestion(text_edit_id, ctx);
                        }
                    }
                });

                // Draggable Splitter between Editor and Results
                let splitter_height = 8.0;
                let (splitter_rect, splitter_resp) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), splitter_height),
                    Sense::drag(),
                );

                if splitter_resp.dragged() {
                    self.editor_height += splitter_resp.drag_delta().y;
                    self.editor_height = self.editor_height.clamp(120.0, (total_height - 80.0).max(120.0));
                }

                if splitter_resp.hovered() || splitter_resp.dragged() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
                }

                let sep_bg = if splitter_resp.dragged() {
                    theme.c_btn_primary()
                } else if splitter_resp.hovered() {
                    theme.c_btn_primary_h()
                } else {
                    theme.c_bg_header()
                };

                ui.painter().rect_filled(splitter_rect, 0.0, sep_bg);

                // Handle in center of splitter
                let center = splitter_rect.center();
                let handle_rect = Rect::from_center_size(center, Vec2::new(36.0, 2.0));
                ui.painter().rect_filled(handle_rect, 1.0, theme.c_fg());

                // Bottom: Query Results Area
                ui.vertical(|ui| {
                    // Result tabs
                    if !self.query_results.is_empty() {
                        ui.horizontal(|ui| {
                            let result_count = self.query_results.len();
                            for i in 0..result_count {
                                let title = self.query_results[i].title.clone();
                                let is_active = self.active_result_idx == i;
                                if ui.selectable_label(is_active, &title).clicked() {
                                    self.active_result_idx = i;
                                }
                            }

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.button(tr("btn_copy_row")).clicked() {
                                    if let Some(res) = self.query_results.get(self.active_result_idx) {
                                        if let Some((r_idx, _)) = self.selected_table_cell {
                                            if let Some(row) = res.rows.get(r_idx) {
                                                let tsv = row
                                                    .iter()
                                                    .map(|c| c.as_deref().unwrap_or("NULL"))
                                                    .collect::<Vec<_>>()
                                                    .join("\t");
                                                ctx.copy_text(tsv);
                                                self.status_text = tr("status_copied_cell");
                                            }
                                        }
                                    }
                                }
                            });
                        });
                        ui.separator();
                    }

                    // Results Table with Virtual Scrolling
                    if let Some(res) = self.query_results.get(self.active_result_idx) {
                        if !res.columns.is_empty() {
                            let num_columns = res.columns.len();
                            let num_rows = res.rows.len();

                            let mut table = TableBuilder::new(ui)
                                .striped(true)
                                .resizable(true)
                                .cell_layout(Layout::left_to_right(Align::Center))
                                .min_scrolled_height(0.0);

                            for _ in 0..num_columns {
                                table = table.column(Column::initial(120.0).at_least(60.0));
                            }

                            table
                                .header(24.0, |mut header| {
                                    for col_name in &res.columns {
                                        header.col(|ui| {
                                            ui.label(RichText::new(col_name).strong().color(theme.c_fg()));
                                        });
                                    }
                                })
                                .body(|body| {
                                    body.rows(22.0, num_rows, |mut row| {
                                        let row_idx = row.index();
                                        if let Some(record) = res.rows.get(row_idx) {
                                            for col_idx in 0..num_columns {
                                                row.col(|ui| {
                                                    let val_opt = record.get(col_idx).and_then(|v| v.as_deref());
                                                    let (display_text, is_null) = match val_opt {
                                                        Some(s) => (s, false),
                                                        None => ("NULL", true),
                                                    };

                                                    let text_widget = if is_null {
                                                        RichText::new(display_text)
                                                            .italics()
                                                            .color(Color32::from_rgb(180, 140, 140))
                                                    } else {
                                                        RichText::new(display_text).color(theme.c_fg())
                                                    };

                                                    let cell_resp = ui.selectable_label(
                                                        self.selected_table_cell == Some((row_idx, col_idx)),
                                                        text_widget,
                                                    );

                                                    if cell_resp.clicked() {
                                                        self.selected_table_cell = Some((row_idx, col_idx));
                                                    }

                                                    // Double click: copy cell value
                                                    if cell_resp.double_clicked() {
                                                        ctx.copy_text(display_text.to_string());
                                                        self.status_text = tr("status_copied_cell");
                                                    }

                                                    // Context menu on table cell
                                                    cell_resp.context_menu(|ui| {
                                                        if ui.button(tr("ctx_copy_cell")).clicked() {
                                                            ctx.copy_text(display_text.to_string());
                                                            self.status_text = tr("status_copied_cell");
                                                            ui.close_menu();
                                                        }
                                                        if ui.button(tr("ctx_copy_row_tsv")).clicked() {
                                                            let tsv = record
                                                                .iter()
                                                                .map(|c| c.as_deref().unwrap_or("NULL"))
                                                                .collect::<Vec<_>>()
                                                                .join("\t");
                                                            ctx.copy_text(tsv);
                                                            self.status_text = tr("status_copied_cell");
                                                            ui.close_menu();
                                                        }
                                                        if ui.button(tr("ctx_copy_row_csv")).clicked() {
                                                            let csv = record
                                                                .iter()
                                                                .map(|c| format!("\"{}\"", c.as_deref().unwrap_or("NULL").replace('\"', "\"\"")))
                                                                .collect::<Vec<_>>()
                                                                .join(",");
                                                            ctx.copy_text(csv);
                                                            self.status_text = tr("status_copied_cell");
                                                            ui.close_menu();
                                                        }
                                                        if ui.button(tr("ctx_copy_with_headers")).clicked() {
                                                            let headers = res.columns.join("\t");
                                                            let tsv = record
                                                                .iter()
                                                                .map(|c| c.as_deref().unwrap_or("NULL"))
                                                                .collect::<Vec<_>>()
                                                                .join("\t");
                                                            ctx.copy_text(format!("{}\n{}", headers, tsv));
                                                            self.status_text = tr("status_copied_cell");
                                                            ui.close_menu();
                                                        }
                                                    });
                                                });
                                            }
                                        }
                                    });
                                });
                        } else if let Some(ref err) = res.error {
                            ui.label(RichText::new(format!("Error: {}", err)).color(Color32::from_rgb(255, 100, 100)));
                        }
                    } else {
                        ui.vertical_centered(|ui| {
                            ui.add_space(32.0);
                            ui.label(RichText::new("Brak wyników do wyświetlenia").color(Color32::GRAY));
                            ui.label(RichText::new("Uruchom zapytanie naciskając ▶ Uruchom lub Ctrl+Enter").size(11.0).color(Color32::GRAY));
                        });
                    }
                });
            });

        // 6. Show Modals
        let mut conn_list = self.connections.clone();
        let mut active_c = self.active_conn_name.clone();
        self.conn_dialog.show(ctx, &self.theme, &mut conn_list, &mut active_c);
        self.connections = conn_list;
        self.active_conn_name = active_c;

        self.settings_dialog.show(ctx, &mut self.theme, &mut self.settings);
        self.about_dialog.show(ctx, &self.theme);
        if self.about_dialog.request_check_updates {
            self.about_dialog.request_check_updates = false;
            self.status_text = tr("status_checking_updates");
            let res = crate::updater::check_for_updates();
            self.update_dialog.show_result(res);
            self.status_text = tr("status_ready");
        }
        self.update_dialog.show(ctx, &self.theme);

        // 7. Unbounded SELECT Confirmation Modal
        if let Some(ref stmts) = self.confirm_unbounded.clone() {
            let mut proceed = false;
            let mut cancel = false;

            egui::Window::new(tr("msg_warn_unbounded_title"))
                .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(tr("msg_warn_unbounded_prompt"));
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.button(tr("btn_cancel")).clicked() {
                            cancel = true;
                        }
                        let proceed_btn = egui::Button::new(
                            RichText::new("Kontynuuj mimo to").strong().color(Color32::WHITE),
                        )
                        .fill(self.theme.c_btn_primary());
                        if ui.add(proceed_btn).clicked() {
                            proceed = true;
                        }
                    });
                });

            if proceed {
                self.confirm_unbounded = None;
                self.execute_statements(stmts.clone());
            } else if cancel {
                self.confirm_unbounded = None;
                self.status_text = tr("status_query_cancelled");
            }
        }

        // 8. Unsaved dirty tab confirmation
        if let Some(tab_id) = self.confirm_close_dirty {
            let mut force_close = false;
            let mut cancel_close = false;

            egui::Window::new(tr("msg_unsaved_title"))
                .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("Ta karta zawiera niezapisane zmiany. Czy mimo to ją zamknąć?");
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button(tr("btn_cancel")).clicked() {
                            cancel_close = true;
                        }
                        if ui.button("Zamknij bez zapisywania").clicked() {
                            force_close = true;
                        }
                    });
                });

            if force_close {
                self.confirm_close_dirty = None;
                if let Some(idx) = self.tabs.iter().position(|t| t.id == tab_id) {
                    self.tabs.remove(idx);
                    if self.tabs.is_empty() {
                        self.new_tab();
                    } else if self.active_tab_id == tab_id {
                        let new_idx = idx.min(self.tabs.len() - 1);
                        self.active_tab_id = self.tabs[new_idx].id;
                    }
                }
            } else if cancel_close {
                self.confirm_close_dirty = None;
            }
        }
    }
}
