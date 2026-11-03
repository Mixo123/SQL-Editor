//! PostgreSQL database support using postgres crate.

use postgres::types::Type;
use postgres::{Client, NoTls, SimpleQueryMessage};
use std::collections::HashMap;

use super::types::SchemaMap;

pub fn format_pg_error(e: &postgres::Error) -> String {
    if let Some(db_err) = e.as_db_error() {
        let mut msg = format!("{}: {}", db_err.severity(), db_err.message());
        msg.push_str(&format!(" [SQLSTATE {}]", db_err.code().code()));
        if let Some(detail) = db_err.detail() {
            msg.push_str(&format!("\nSzczegóły: {}", detail));
        }
        if let Some(hint) = db_err.hint() {
            msg.push_str(&format!("\nWskazówka: {}", hint));
        }
        if let Some(pos) = db_err.position() {
            msg.push_str(&format!("\nPozycja w zapytaniu: {:?}", pos));
        }
        if let Some(w) = db_err.where_() {
            msg.push_str(&format!("\nKontekst: {}", w));
        }
        if let Some(table) = db_err.table() {
            msg.push_str(&format!("\nTabela: {}", table));
        }
        if let Some(column) = db_err.column() {
            msg.push_str(&format!("\nKolumna: {}", column));
        }
        msg
    } else {
        e.to_string()
    }
}

pub fn test_postgres(dsn: &str) -> Result<(), String> {
    let mut client = Client::connect(dsn, NoTls).map_err(|e| format_pg_error(&e))?;
    client.simple_query("SELECT 1;").map_err(|e| format_pg_error(&e))?;
    Ok(())
}

pub fn get_postgres_schema(dsn: &str) -> Result<SchemaMap, String> {
    let mut client = Client::connect(dsn, NoTls).map_err(|e| format_pg_error(&e))?;
    let mut schema = HashMap::new();

    let tables_sql = "
        SELECT table_name 
        FROM information_schema.tables 
        WHERE table_schema NOT IN ('pg_catalog', 'information_schema')
        ORDER BY table_name;
    ";
    let rows = client.query(tables_sql, &[]).map_err(|e| format_pg_error(&e))?;
    for row in rows {
        let tbl: String = row.get(0);
        schema.insert(tbl, Vec::new());
    }

    let cols_sql = "
        SELECT table_name, column_name 
        FROM information_schema.columns 
        WHERE table_schema NOT IN ('pg_catalog', 'information_schema')
        ORDER BY table_name, ordinal_position;
    ";
    let col_rows = client.query(cols_sql, &[]).map_err(|e| format_pg_error(&e))?;
    for row in col_rows {
        let tbl: String = row.get(0);
        let col: String = row.get(1);
        if let Some(list) = schema.get_mut(&tbl) {
            list.push(col);
        }
    }

    Ok(schema)
}

pub fn execute_postgres_stream<F>(
    dsn: &str,
    sql: &str,
    limit: usize,
    mut on_batch: F,
) -> Result<(), String>
where
    F: FnMut(Option<Vec<String>>, Vec<Vec<Option<String>>>),
{
    let mut client = Client::connect(dsn, NoTls).map_err(|e| format_pg_error(&e))?;

    // First attempt to prepare statement to check for column metadata
    match client.prepare(sql) {
        Ok(statement) => {
            let columns = statement.columns();
            if !columns.is_empty() {
                let col_names: Vec<String> = columns.iter().map(|c| c.name().to_string()).collect();
                let col_types: Vec<Type> = columns.iter().map(|c| c.type_().clone()).collect();
                let col_count = col_names.len();

                let messages = client.simple_query(sql).map_err(|e| format_pg_error(&e))?;

                let mut first = true;
                let mut total = 0;
                let mut batch = Vec::new();

                for msg in messages {
                    if let SimpleQueryMessage::Row(row) = msg {
                        let mut record = Vec::with_capacity(col_count);
                        for i in 0..col_count {
                            let cell = row.get(i).map(|s| {
                                if col_types.get(i) == Some(&Type::BOOL) {
                                    if s == "t" {
                                        "true".to_string()
                                    } else if s == "f" {
                                        "false".to_string()
                                    } else {
                                        s.to_string()
                                    }
                                } else {
                                    s.to_string()
                                }
                            });
                            record.push(cell);
                        }
                        batch.push(record);

                        if batch.len() >= 2000 {
                            on_batch(if first { Some(col_names.clone()) } else { None }, batch);
                            first = false;
                            batch = Vec::new();
                        }

                        total += 1;
                        if total >= limit {
                            break;
                        }
                    }
                }

                if !batch.is_empty() || first {
                    on_batch(if first { Some(col_names) } else { None }, batch);
                }
            } else {
                let affected = client.execute(&statement, &[]).map_err(|e| format_pg_error(&e))?;
                on_batch(
                    Some(vec!["status".to_string()]),
                    vec![vec![Some(format!("Rows affected: {}", affected))]],
                );
            }
        }
        Err(_prep_err) => {
            // Some statements cannot be prepared (e.g. multiple statements, or certain commands),
            // fallback directly to simple_query
            let messages = client.simple_query(sql).map_err(|e| format_pg_error(&e))?;

            let mut first = true;
            let mut total = 0;
            let mut batch = Vec::new();
            let mut col_names: Vec<String> = Vec::new();
            let mut affected_total: Option<u64> = None;

            for msg in messages {
                match msg {
                    SimpleQueryMessage::Row(row) => {
                        if col_names.is_empty() {
                            col_names = row.columns().iter().map(|c| c.name().to_string()).collect();
                        }
                        let col_count = row.len();
                        let mut record = Vec::with_capacity(col_count);
                        for i in 0..col_count {
                            record.push(row.get(i).map(|s| s.to_string()));
                        }
                        batch.push(record);

                        if batch.len() >= 2000 {
                            on_batch(if first { Some(col_names.clone()) } else { None }, batch);
                            first = false;
                            batch = Vec::new();
                        }

                        total += 1;
                        if total >= limit {
                            break;
                        }
                    }
                    SimpleQueryMessage::CommandComplete(affected) => {
                        affected_total = Some(affected);
                    }
                    _ => {}
                }
            }

            if !col_names.is_empty() {
                if !batch.is_empty() || first {
                    on_batch(if first { Some(col_names) } else { None }, batch);
                }
            } else if let Some(affected) = affected_total {
                on_batch(
                    Some(vec!["status".to_string()]),
                    vec![vec![Some(format!("Rows affected: {}", affected))]],
                );
            } else {
                on_batch(
                    Some(vec!["status".to_string()]),
                    vec![vec![Some("Query executed successfully".to_string())]],
                );
            }
        }
    }

    Ok(())
}

