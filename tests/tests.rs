use sql_editor::autocomplete::get_autocomplete_suggestions;
use sql_editor::crypto::{decrypt_str, encrypt_str};
use sql_editor::db::sqlite::{execute_sqlite_stream, get_sqlite_schema};
use sql_editor::db::types::{is_unbounded_select, split_sql_statements};
use sql_editor::export::{export_csv, export_json, export_markdown, export_xlsx};
use sql_editor::i18n::{set_language, tr, tr_fmt};
use sql_editor::theme::{color_to_hex, get_theme, hex_to_color};
use sql_editor::updater::parse_version;
use std::collections::HashMap;

#[test]
fn test_split_sql_statements() {
    let sql = "SELECT 1; SELECT 2; /* comment with ; */ SELECT 'hello;world';";
    let stmts = split_sql_statements(sql);
    assert_eq!(stmts.len(), 3);
    assert_eq!(stmts[0], "SELECT 1");
    assert_eq!(stmts[1], "SELECT 2");
    assert_eq!(stmts[2], "/* comment with ; */ SELECT 'hello;world'");
}

use egui_extras::{Column, TableBuilder};

#[test]
fn test_utf8_pasted_text() {
    let text = "SELECT * FROM pracownicy WHERE imię = 'Michał';\nSELECT id, nazwisko, pensja FROM pracownicy WHERE miasto = 'Kraków'";
    let stmt = sql_editor::autocomplete::get_current_statement(text);
    assert!(!stmt.is_empty());
    assert!(stmt.contains("Kraków"));
}

#[test]
fn test_table_builder_horizontal_scroll() {
    let ctx = egui::Context::default();
    let _ = ctx.run(Default::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::horizontal().show(ui, |ui| {
                let mut table = TableBuilder::new(ui)
                    .striped(true)
                    .resizable(true);
                for _ in 0..30 {
                    table = table.column(Column::initial(120.0).at_least(60.0));
                }
                table.header(20.0, |mut h| {
                    for i in 0..30 {
                        h.col(|ui| { ui.label(format!("Col {}", i)); });
                    }
                }).body(|mut b| {
                    b.row(20.0, |mut r| {
                        for i in 0..30 {
                            r.col(|ui| { ui.label(format!("Val {}", i)); });
                        }
                    });
                });
            });
        });
    });
}

#[test]
fn test_is_unbounded_select() {
    assert!(is_unbounded_select("SELECT * FROM users"));
    assert!(!is_unbounded_select("SELECT * FROM users LIMIT 10"));
    assert!(!is_unbounded_select("INSERT INTO users VALUES (1)"));
    assert!(is_unbounded_select("WITH cte AS (SELECT 1) SELECT * FROM cte"));
    assert!(!is_unbounded_select("WITH cte AS (SELECT 1) SELECT * FROM cte LIMIT 5"));
}

#[test]
fn test_crypto_compatibility() {
    let original = "my_database_password_2026!";
    let encrypted = encrypt_str(original);
    assert!(encrypted.starts_with("enc:"));
    let decrypted = decrypt_str(&encrypted);
    assert_eq!(decrypted, original);
}

#[test]
fn test_autocomplete() {
    let mut schema = HashMap::new();
    schema.insert("users".to_string(), vec!["id".to_string(), "email".to_string(), "name".to_string()]);
    schema.insert("orders".to_string(), vec!["order_id".to_string(), "total".to_string()]);

    // Test keyword suggestion
    let (suggs, pref_len) = get_autocomplete_suggestions("SEL", &schema, None);
    assert!(suggs.iter().any(|s| s.text == "SELECT"));
    assert_eq!(pref_len, 3);

    // Test table suggestion after FROM
    let (suggs, _) = get_autocomplete_suggestions("SELECT * FROM u", &schema, None);
    assert!(suggs.iter().any(|s| s.text == "users"));

    // Test dot notation users.e
    let (suggs, _) = get_autocomplete_suggestions("SELECT users.e", &schema, None);
    assert!(suggs.iter().any(|s| s.text == "email"));

    // Autocomplete should NOT trigger when typing spaces, newlines, or numbers
    assert!(get_autocomplete_suggestions("SELECT * FROM ", &schema, None).0.is_empty());
    assert!(get_autocomplete_suggestions("SELECT 1;\n", &schema, None).0.is_empty());
    assert!(get_autocomplete_suggestions("", &schema, None).0.is_empty());
    assert!(get_autocomplete_suggestions("WHERE id = 123", &schema, None).0.is_empty());
}

#[test]
fn test_word_and_line_selection() {
    use sql_editor::autocomplete::{find_line_range_at, find_word_range_at};

    let sql = "SELECT user_id, imię FROM pracownicy_kraków;\nWHERE pensja > 5000;\nORDER BY id;";
    let chars: Vec<char> = sql.chars().collect();

    // Double-click on 'user_id'
    let idx_user_id = sql.find("user_id").unwrap() + 3;
    let (w_start, w_end) = find_word_range_at(&chars, idx_user_id);
    let selected_word: String = chars[w_start..w_end].iter().collect();
    assert_eq!(selected_word, "user_id");

    // Double-click on Unicode Polish word 'pracownicy_kraków'
    let idx_pl = sql.find("pracownicy_kraków").unwrap() + 5;
    let (pl_start, pl_end) = find_word_range_at(&chars, idx_pl);
    let selected_pl: String = chars[pl_start..pl_end].iter().collect();
    assert_eq!(selected_pl, "pracownicy_kraków");

    // Triple-click on line 1
    let (l1_start, l1_end) = find_line_range_at(&chars, idx_user_id);
    let line1: String = chars[l1_start..l1_end].iter().collect();
    assert_eq!(line1, "SELECT user_id, imię FROM pracownicy_kraków;\n");

    // Triple-click on line 2
    let idx_line2 = sql.find("pensja").unwrap();
    let (l2_start, l2_end) = find_line_range_at(&chars, idx_line2);
    let line2: String = chars[l2_start..l2_end].iter().collect();
    assert_eq!(line2, "WHERE pensja > 5000;\n");

    // Triple-click on last line (line 3, no trailing newline)
    let idx_line3 = sql.find("ORDER").unwrap();
    let (l3_start, l3_end) = find_line_range_at(&chars, idx_line3);
    let line3: String = chars[l3_start..l3_end].iter().collect();
    assert_eq!(line3, "ORDER BY id;");
}

#[test]
fn test_i18n() {
    set_language("pl");
    assert_eq!(tr("btn_run"), "▶ Uruchom");
    assert_eq!(tr_fmt("ctx_edit_conn", &[("name", "Baza1")]), "Edytuj 'Baza1'");

    set_language("en");
    assert_eq!(tr("btn_run"), "▶ Run");
    assert_eq!(tr_fmt("ctx_edit_conn", &[("name", "Baza1")]), "Edit 'Baza1'");

    set_language("de");
    assert_eq!(tr("btn_run"), "▶ Ausführen");

    set_language("es");
    assert_eq!(tr("btn_run"), "▶ Ejecutar");
}

#[test]
fn test_theme_colors() {
    let c = hex_to_color("#0e639c");
    assert_eq!(c.r(), 14);
    assert_eq!(c.g(), 99);
    assert_eq!(c.b(), 156);
    assert_eq!(color_to_hex(c), "#0e639c");

    let theme = get_theme("Dracula", &HashMap::new());
    assert_eq!(theme.name, "Dracula");
    assert_eq!(theme.bg, "#282a36");
}

#[test]
fn test_parse_version() {
    assert_eq!(parse_version("v1.2.3"), (1, 2, 3));
    assert_eq!(parse_version("0.5.0"), (0, 5, 0));
    assert_eq!(parse_version("0.5.2"), (0, 5, 2));
    assert!(parse_version("v0.5.3") > parse_version("v0.5.2"));
    assert_eq!(sql_editor::version::VERSION, "0.5.0");
}

#[test]
fn test_egui_text_edit_state() {
    let ctx = egui::Context::default();
    let id = egui::Id::new("test");
    if let Some(state) = egui::text_edit::TextEditState::load(&ctx, id) {
        let _ = state.cursor.char_range().map(|r| r.primary.index);
    }
    let _ = egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(5));
    let _ = ctx.run(Default::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            let r = ui.label("hello");
            let _ = r.double_clicked();
            let _ = r.triple_clicked();
        });
    });
}

#[test]
fn test_sqlite_query_and_schema() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let db_path = tmp_dir.path().join("test.db");
    let db_str = db_path.to_str().unwrap();

    // Create table & insert data
    {
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute(
            "CREATE TABLE employees (id INTEGER PRIMARY KEY, name TEXT, salary REAL)",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO employees VALUES (1, 'Alice', 5000.0)", []).unwrap();
        conn.execute("INSERT INTO employees VALUES (2, 'Bob', NULL)", []).unwrap();
        conn.execute("INSERT INTO employees VALUES (3, 'Charlie', 85000.50)", []).unwrap();
    }

    // Schema introspection
    let schema = get_sqlite_schema(db_str).unwrap();
    assert!(schema.contains_key("employees"));
    let cols = &schema["employees"];
    assert_eq!(cols, &["id", "name", "salary"]);

    // Query stream
    let mut received_cols = None;
    let mut received_rows = Vec::new();

    execute_sqlite_stream(db_str, "SELECT * FROM employees ORDER BY id", 100, |cols, rows| {
        if let Some(c) = cols {
            received_cols = Some(c);
        }
        received_rows.extend(rows);
    })
    .unwrap();

    assert_eq!(received_cols.unwrap(), vec!["id", "name", "salary"]);
    assert_eq!(received_rows.len(), 3);
    assert_eq!(received_rows[0], vec![Some("1".to_string()), Some("Alice".to_string()), Some("5000".to_string())]);
    assert_eq!(received_rows[1], vec![Some("2".to_string()), Some("Bob".to_string()), None]);
    assert_eq!(received_rows[2], vec![Some("3".to_string()), Some("Charlie".to_string()), Some("85000.5".to_string())]);

    // Ensure no cell anywhere is `<value>`
    for row in &received_rows {
        for cell in row {
            if let Some(val) = cell {
                assert_ne!(val, "<value>");
            }
        }
    }

    // Test Exports
    let cols = ["id".into(), "name".into(), "salary".into()];
    let csv_file = tmp_dir.path().join("out.csv");
    export_csv(&csv_file, &cols, &received_rows).unwrap();
    assert!(csv_file.exists());

    let json_file = tmp_dir.path().join("out.json");
    export_json(&json_file, &cols, &received_rows).unwrap();
    assert!(json_file.exists());

    let md_file = tmp_dir.path().join("out.md");
    export_markdown(&md_file, &cols, &received_rows).unwrap();
    assert!(md_file.exists());

    let xlsx_file = tmp_dir.path().join("out.xlsx");
    export_xlsx(&xlsx_file, &cols, &received_rows).unwrap();
    assert!(xlsx_file.exists());

    // Test multiple queries splitting and independent execution
    let multi_sql = "SELECT id, name FROM employees WHERE id = 1; SELECT salary FROM employees WHERE id = 2;";
    let stmts = split_sql_statements(multi_sql);
    assert_eq!(stmts.len(), 2);
    assert_eq!(stmts[0], "SELECT id, name FROM employees WHERE id = 1");
    assert_eq!(stmts[1], "SELECT salary FROM employees WHERE id = 2");

    let mut q1_cols = None;
    let mut q1_rows = Vec::new();
    execute_sqlite_stream(db_str, &stmts[0], 100, |cols, rows| {
        if let Some(c) = cols { q1_cols = Some(c); }
        q1_rows.extend(rows);
    }).unwrap();

    let mut q2_cols = None;
    let mut q2_rows = Vec::new();
    execute_sqlite_stream(db_str, &stmts[1], 100, |cols, rows| {
        if let Some(c) = cols { q2_cols = Some(c); }
        q2_rows.extend(rows);
    }).unwrap();

    assert_eq!(q1_cols.unwrap(), vec!["id", "name"]);
    assert_eq!(q1_rows.len(), 1);
    assert_eq!(q1_rows[0], vec![Some("1".to_string()), Some("Alice".to_string())]);

    assert_eq!(q2_cols.unwrap(), vec!["salary"]);
    assert_eq!(q2_rows.len(), 1);
    assert_eq!(q2_rows[0], vec![None]);

    // Test detailed database error formatting
    let err = execute_sqlite_stream(db_str, "SELECT * FROM non_existing_table_xyz", 100, |_, _| {})
        .unwrap_err();
    assert!(err.contains("SQLite Error"));
    assert!(err.contains("no such table: non_existing_table_xyz"));
    assert!(err.contains("kod 1"));
}

#[test]
fn test_session_save_and_load() {
    use sql_editor::session::{load_session_from, save_session_to, AppSession, SavedTab};
    use tempfile::tempdir;

    let tmp = tempdir().unwrap();
    let session_path = tmp.path().join("session.json");

    let session = AppSession {
        active_conn_name: Some("Production SQLite".to_string()),
        active_tab_index: 1,
        editor_height: Some(420.0),
        tabs: vec![
            SavedTab {
                name: "query1.sql".to_string(),
                path: Some(tmp.path().join("query1.sql")),
                content: "SELECT * FROM users;".to_string(),
                modified: false,
            },
            SavedTab {
                name: "Bez tytułu 2".to_string(),
                path: None,
                content: "SELECT COUNT(*) FROM orders;".to_string(),
                modified: true,
            },
        ],
    };

    save_session_to(&session, &session_path);
    assert!(session_path.exists());

    let loaded = load_session_from(&session_path).expect("Failed to load saved session");
    assert_eq!(loaded.active_conn_name, Some("Production SQLite".to_string()));
    assert_eq!(loaded.active_tab_index, 1);
    assert_eq!(loaded.editor_height, Some(420.0));
    assert_eq!(loaded.tabs.len(), 2);
    assert_eq!(loaded.tabs[0].name, "query1.sql");
    assert_eq!(loaded.tabs[0].content, "SELECT * FROM users;");
    assert!(!loaded.tabs[0].modified);
    assert_eq!(loaded.tabs[1].name, "Bez tytułu 2");
    assert_eq!(loaded.tabs[1].content, "SELECT COUNT(*) FROM orders;");
    assert!(loaded.tabs[1].modified);
}

#[test]
fn test_updater_formatters() {
    use sql_editor::updater::{format_size, format_speed};

    assert_eq!(format_size(0), "0 B");
    assert_eq!(format_size(500 * 1024), "500.0 KB");
    assert_eq!(format_size(15 * 1024 * 1024), "15.0 MB");

    assert_eq!(format_speed(250.0), "250 B/s");
    assert_eq!(format_speed(1500.0), "1.5 KB/s");
    assert_eq!(format_speed(2.5 * 1024.0 * 1024.0), "2.5 MB/s");
}

#[test]
fn test_in_app_download() {
    use std::io::Cursor;
    use std::sync::atomic::AtomicBool;
    use std::sync::{Arc, Mutex};
    use tempfile::tempdir;

    let payload = b"test payload for update download in sql editor rust";
    let reader = Cursor::new(payload);
    let tmp = tempdir().unwrap();
    let target_file = tmp.path().join("sql-editor-macos.dmg");

    let progress = Arc::new(Mutex::new(sql_editor::updater::DownloadProgress {
        asset_name: "sql-editor-macos.dmg".to_string(),
        target_path: target_file.clone(),
        downloaded_bytes: 0,
        total_bytes: Some(payload.len() as u64),
        speed_bytes_per_sec: 0.0,
        finished: false,
        error: None,
    }));
    let cancel = Arc::new(AtomicBool::new(false));

    sql_editor::updater::download_from_reader(
        reader,
        &target_file,
        Some(payload.len() as u64),
        Arc::clone(&progress),
        Arc::clone(&cancel),
    );

    let p = progress.lock().unwrap().clone();
    assert!(p.finished);
    assert!(p.error.is_none());
    assert_eq!(p.downloaded_bytes, payload.len() as u64);
    assert_eq!(p.total_bytes, Some(payload.len() as u64));
    assert!(target_file.exists());
    let content = std::fs::read(&target_file).unwrap();
    assert_eq!(content, payload);

    // Test cancellation
    let target_file2 = tmp.path().join("sql-editor-cancelled.dmg");
    let progress2 = Arc::new(Mutex::new(sql_editor::updater::DownloadProgress {
        asset_name: "sql-editor-cancelled.dmg".to_string(),
        target_path: target_file2.clone(),
        downloaded_bytes: 0,
        total_bytes: Some(payload.len() as u64),
        speed_bytes_per_sec: 0.0,
        finished: false,
        error: None,
    }));
    let cancel2 = Arc::new(AtomicBool::new(true)); // cancel immediately

    sql_editor::updater::download_from_reader(
        Cursor::new(payload),
        &target_file2,
        Some(payload.len() as u64),
        Arc::clone(&progress2),
        Arc::clone(&cancel2),
    );

    let p2 = progress2.lock().unwrap().clone();
    assert!(!p2.finished);
    assert!(!target_file2.exists());
}
