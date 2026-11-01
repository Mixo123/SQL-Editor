//! Database connectivity and query execution engine.

pub mod mysql;
pub mod postgres;
pub mod sqlite;
pub mod types;

use std::fs;

pub use types::{ConnectionMeta, SchemaMap};

use crate::crypto::{conn_file, ensure_app_dir};

pub fn load_connections() -> Vec<ConnectionMeta> {
    let path = conn_file();
    if !path.exists() {
        return Vec::new();
    }
    if let Ok(data) = fs::read_to_string(&path) {
        if let Ok(list) = serde_json::from_str::<Vec<ConnectionMeta>>(&data) {
            return list;
        }
    }
    Vec::new()
}

pub fn save_connections(conns: &[ConnectionMeta]) -> Result<(), String> {
    ensure_app_dir();
    let path = conn_file();
    let tmp = path.with_extension("tmp");
    let json = serde_json::to_string_pretty(conns).map_err(|e| e.to_string())?;
    fs::write(&tmp, json).map_err(|e| e.to_string())?;
    fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn test_connection(meta: &ConnectionMeta) -> Result<(), String> {
    let dsn = meta.build_dsn();
    match meta.kind.as_str() {
        "sqlite" => sqlite::test_sqlite(&dsn),
        "postgres" | "postgresql" => postgres::test_postgres(&dsn),
        "mysql" => mysql::test_mysql(&dsn),
        "oracle" => {
            Err("Oracle database driver requires Oracle Instant Client C libraries. Please connect via PostgreSQL, MySQL, or SQLite in this build.".to_string())
        }
        "mssql" => {
            Err("Microsoft SQL Server driver requires FreeTDS / ODBC libraries. Please connect via PostgreSQL, MySQL, or SQLite in this build.".to_string())
        }
        "bigquery" => {
            Err("Google BigQuery requires Google Cloud Service Account credentials JSON and GCP API access. Please check configuration.".to_string())
        }
        _ => Err(format!("Unknown database kind: {}", meta.kind)),
    }
}

pub fn get_schema(meta: &ConnectionMeta) -> Result<SchemaMap, String> {
    let dsn = meta.build_dsn();
    match meta.kind.as_str() {
        "sqlite" => sqlite::get_sqlite_schema(&dsn),
        "postgres" | "postgresql" => postgres::get_postgres_schema(&dsn),
        "mysql" => mysql::get_mysql_schema(&dsn),
        _ => Ok(std::collections::HashMap::new()),
    }
}

pub fn stream_query<F>(
    meta: &ConnectionMeta,
    sql: &str,
    limit: usize,
    on_batch: F,
) -> Result<(), String>
where
    F: FnMut(Option<Vec<String>>, Vec<Vec<Option<String>>>),
{
    let dsn = meta.build_dsn();
    match meta.kind.as_str() {
        "sqlite" => sqlite::execute_sqlite_stream(&dsn, sql, limit, on_batch),
        "postgres" | "postgresql" => postgres::execute_postgres_stream(&dsn, sql, limit, on_batch),
        "mysql" => mysql::execute_mysql_stream(&dsn, sql, limit, on_batch),
        "oracle" | "mssql" | "bigquery" => {
            test_connection(meta)
        }
        _ => Err(format!("Unsupported database kind: {}", meta.kind)),
    }
}
