//! SQLite database support using rusqlite.

use rusqlite::types::ValueRef;
use rusqlite::Connection;
use std::collections::HashMap;
use std::path::PathBuf;

use super::types::SchemaMap;

pub fn resolve_db_path(p: &str) -> PathBuf {
    if p.starts_with("~/") || p == "~" {
        if let Some(ud) = directories::UserDirs::new() {
            return ud.home_dir().join(p.trim_start_matches("~/"));
        }
    }
    PathBuf::from(p)
}

pub fn test_sqlite(path_str: &str) -> Result<(), String> {
    let p = resolve_db_path(path_str);
    if !p.is_file() {
        return Err(format!("SQLite database file not found: {}", path_str));
    }
    let conn = Connection::open(&p).map_err(|e| e.to_string())?;
    conn.query_row("SELECT name FROM sqlite_master LIMIT 1", [], |_| Ok(()))
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_sqlite_schema(path_str: &str) -> Result<SchemaMap, String> {
    let p = resolve_db_path(path_str);
    if !p.is_file() {
        return Ok(HashMap::new());
    }
    let conn = Connection::open(&p).map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite_%' ORDER BY name")
        .map_err(|e| e.to_string())?;

    let tables: Vec<String> = stmt
        .query_map([], |row| row.get(0))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    let mut schema = HashMap::new();
    for tbl in tables {
        let escaped = tbl.replace('"', "\"\"");
        let pragma_sql = format!("PRAGMA table_info(\"{}\")", escaped);
        if let Ok(mut pstmt) = conn.prepare(&pragma_sql) {
            let cols: Vec<String> = pstmt
                .query_map([], |row| row.get(1))
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
                .unwrap_or_default();
            schema.insert(tbl, cols);
        }
    }

    Ok(schema)
}

pub fn execute_sqlite_stream<F>(
    path_str: &str,
    sql: &str,
    limit: usize,
    mut on_batch: F,
) -> Result<(), String>
where
    F: FnMut(Option<Vec<String>>, Vec<Vec<Option<String>>>),
{
    let p = resolve_db_path(path_str);
    if !p.is_file() {
        return Err(format!("SQLite database file not found: {}", path_str));
    }
    let conn = Connection::open(&p).map_err(|e| e.to_string())?;

    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;

    let col_count = stmt.column_count();
    if col_count > 0 {
        let cols: Vec<String> = stmt.column_names().iter().map(|&s| s.to_string()).collect();
        let mut first = true;
        let mut total = 0;
        let mut batch = Vec::new();

        let mut rows = stmt.query([]).map_err(|e| e.to_string())?;

        while let Some(row) = rows.next().map_err(|e| e.to_string())? {
            let mut record = Vec::with_capacity(col_count);
            for i in 0..col_count {
                let val_ref = row.get_ref(i).map_err(|e| e.to_string())?;
                let str_val = match val_ref {
                    ValueRef::Null => None,
                    ValueRef::Integer(v) => Some(v.to_string()),
                    ValueRef::Real(v) => Some(v.to_string()),
                    ValueRef::Text(bytes) => Some(String::from_utf8_lossy(bytes).into_owned()),
                    ValueRef::Blob(bytes) => Some(format!("<blob {} bytes>", bytes.len())),
                };
                record.push(str_val);
            }
            batch.push(record);

            if batch.len() >= 2000 {
                on_batch(if first { Some(cols.clone()) } else { None }, batch);
                first = false;
                batch = Vec::new();
            }

            total += 1;
            if total >= limit {
                break;
            }
        }

        if !batch.is_empty() || first {
            on_batch(if first { Some(cols) } else { None }, batch);
        }
    } else {
        let affected = stmt.execute([]).map_err(|e| e.to_string())?;
        on_batch(
            Some(vec!["status".to_string()]),
            vec![vec![Some(format!("Rows affected: {}", affected))]],
        );
    }

    Ok(())
}

