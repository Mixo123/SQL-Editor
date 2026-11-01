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
    assert_eq!(parse_version("0.5.2"), (0, 5, 2));
    assert!(parse_version("v0.5.3") > parse_version("v0.5.2"));
}

#[test]
fn test_egui_text_edit_state() {
    let ctx = egui::Context::default();
    let id = egui::Id::new("test");
    if let Some(state) = egui::text_edit::TextEditState::load(&ctx, id) {
        let _ = state.cursor.char_range().map(|r| r.primary.index);
    }
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
    assert_eq!(received_rows.len(), 2);
    assert_eq!(received_rows[0], vec![Some("1".to_string()), Some("Alice".to_string()), Some("5000".to_string())]);
    assert_eq!(received_rows[1], vec![Some("2".to_string()), Some("Bob".to_string()), None]);

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
}
