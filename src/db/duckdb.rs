//! DuckDB database support using duckdb crate.

use duckdb::types::{TimeUnit, ValueRef};
use duckdb::Connection;
use std::collections::HashMap;
use std::path::PathBuf;

use super::sqlite::resolve_db_path;
use super::types::SchemaMap;

pub fn format_duckdb_error(e: &duckdb::Error) -> String {
    match e {
        duckdb::Error::DuckDBFailure(err, msg) => {
            let detail = msg.as_deref().unwrap_or("");
            if !detail.is_empty() {
                format!("DuckDB Error: {}", detail)
            } else {
                format!("DuckDB Error: {:?}", err)
            }
        }
        _ => e.to_string(),
    }
}

pub fn open_duckdb(path_str: &str) -> Result<Connection, String> {
    let trimmed = path_str.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case(":memory:") {
        Connection::open_in_memory().map_err(|e| format_duckdb_error(&e))
    } else {
        let p = resolve_db_path(trimmed);
        if !p.is_file() {
            return Err(format!("DuckDB database file not found: {}", trimmed));
        }
        Connection::open(&p).map_err(|e| format_duckdb_error(&e))
    }
}

pub fn test_duckdb(path_str: &str) -> Result<(), String> {
    let conn = open_duckdb(path_str)?;
    conn.query_row("SELECT 1", [], |_| Ok(()))
        .map_err(|e| format_duckdb_error(&e))?;
    Ok(())
}

pub fn get_duckdb_schema(path_str: &str) -> Result<SchemaMap, String> {
    let conn = open_duckdb(path_str)?;
    let mut schema: SchemaMap = HashMap::new();

    // 1. Get all tables and columns from information_schema
    if let Ok(mut stmt) = conn.prepare(
        "SELECT table_name, column_name FROM information_schema.columns \
         WHERE table_schema NOT IN ('information_schema', 'pg_catalog') \
         ORDER BY table_name, ordinal_position",
    ) {
        if let Ok(mut rows) = stmt.query([]) {
            while let Ok(Some(row)) = rows.next() {
                if let (Ok(table), Ok(col)) = (row.get::<_, String>(0), row.get::<_, String>(1)) {
                    schema.entry(table).or_default().push(col);
                }
            }
        }
    }

    // 2. Also register any tables that may not have columns or were newly created
    if let Ok(mut stmt_tbl) = conn.prepare(
        "SELECT table_name FROM information_schema.tables \
         WHERE table_schema NOT IN ('information_schema', 'pg_catalog') \
         ORDER BY table_name",
    ) {
        if let Ok(mut rows_tbl) = stmt_tbl.query([]) {
            while let Ok(Some(row)) = rows_tbl.next() {
                if let Ok(table) = row.get::<_, String>(0) {
                    schema.entry(table).or_default();
                }
            }
        }
    }

    Ok(schema)
}

fn format_duckdb_value_ref(val_ref: ValueRef) -> Option<String> {
    match val_ref {
        ValueRef::Null => None,
        ValueRef::Boolean(b) => Some(b.to_string()),
        ValueRef::TinyInt(v) => Some(v.to_string()),
        ValueRef::SmallInt(v) => Some(v.to_string()),
        ValueRef::Int(v) => Some(v.to_string()),
        ValueRef::BigInt(v) => Some(v.to_string()),
        ValueRef::HugeInt(v) => Some(v.to_string()),
        ValueRef::UHugeInt(v) => Some(v.to_string()),
        ValueRef::UTinyInt(v) => Some(v.to_string()),
        ValueRef::USmallInt(v) => Some(v.to_string()),
        ValueRef::UInt(v) => Some(v.to_string()),
        ValueRef::UBigInt(v) => Some(v.to_string()),
        ValueRef::Float(v) => Some(v.to_string()),
        ValueRef::Double(v) => Some(v.to_string()),
        ValueRef::Decimal(d) => Some(d.to_string()),
        ValueRef::Text(bytes) => Some(String::from_utf8_lossy(bytes).into_owned()),
        ValueRef::Blob(bytes) => Some(format!("<blob {} bytes>", bytes.len())),
        ValueRef::Date32(days) => {
            chrono::DateTime::from_timestamp(days as i64 * 86400, 0)
                .map(|dt| dt.format("%Y-%m-%d").to_string())
                .or_else(|| Some(days.to_string()))
        }
        ValueRef::Time64(unit, val) => {
            let micros = match unit {
                TimeUnit::Second => val * 1_000_000,
                TimeUnit::Millisecond => val * 1_000,
                TimeUnit::Microsecond => val,
                TimeUnit::Nanosecond => val / 1_000,
            };
            let secs = micros / 1_000_000;
            let us = micros % 1_000_000;
            Some(format!("{:02}:{:02}:{:02}.{:06}", secs / 3600, (secs % 3600) / 60, secs % 60, us))
        }
        ValueRef::Timestamp(unit, val) => {
            let dt_opt = match unit {
                TimeUnit::Second => chrono::DateTime::from_timestamp(val, 0),
                TimeUnit::Millisecond => {
                    chrono::DateTime::from_timestamp(val / 1000, ((val % 1000) * 1_000_000) as u32)
                }
                TimeUnit::Microsecond => {
                    chrono::DateTime::from_timestamp(val / 1_000_000, ((val % 1_000_000) * 1000) as u32)
                }
                TimeUnit::Nanosecond => {
                    chrono::DateTime::from_timestamp(val / 1_000_000_000, (val % 1_000_000_000) as u32)
                }
            };
            Some(
                dt_opt
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S%.f").to_string())
                    .unwrap_or_else(|| val.to_string()),
            )
        }
        ValueRef::Interval { months, days, nanos } => {
            Some(format!("{} mon {} days {} ns", months, days, nanos))
        }
        _ => Some(format!("{:?}", val_ref)),
    }
}

pub fn execute_duckdb_stream<F>(
    path_str: &str,
    sql: &str,
    limit: usize,
    mut on_batch: F,
) -> Result<(), String>
where
    F: FnMut(Option<Vec<String>>, Vec<Vec<Option<String>>>),
{
    let conn = open_duckdb(path_str)?;

    let mut stmt = conn.prepare(sql).map_err(|e| format_duckdb_error(&e))?;

    let col_count = stmt.column_count();
    if col_count > 0 {
        let mut first = true;
        let mut total = 0;
        let mut batch = Vec::new();

        let mut rows = stmt.query([]).map_err(|e| format_duckdb_error(&e))?;
        let cols: Vec<String> = stmt.column_names();

        while let Some(row) = rows.next().map_err(|e| format_duckdb_error(&e))? {
            let mut record = Vec::with_capacity(col_count);
            for i in 0..col_count {
                let val_ref = row.get_ref(i).map_err(|e| format_duckdb_error(&e))?;
                let str_val = format_duckdb_value_ref(val_ref);
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
        let affected = stmt.execute([]).map_err(|e| format_duckdb_error(&e))?;
        on_batch(
            Some(vec!["status".to_string()]),
            vec![vec![Some(format!("Rows affected: {}", affected))]],
        );
    }

    Ok(())
}
