//! MySQL database support using mysql crate.

use mysql::prelude::*;
use mysql::{Conn, Opts, Value};
use std::collections::HashMap;

use super::types::SchemaMap;

pub fn test_mysql(dsn: &str) -> Result<(), String> {
    let opts = Opts::from_url(dsn).map_err(|e| e.to_string())?;
    let mut conn = Conn::new(opts).map_err(|e| e.to_string())?;
    conn.query_drop("SELECT 1").map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_mysql_schema(dsn: &str) -> Result<SchemaMap, String> {
    let opts = Opts::from_url(dsn).map_err(|e| e.to_string())?;
    let db_name = opts.get_db_name().unwrap_or("").to_string();
    let mut conn = Conn::new(opts).map_err(|e| e.to_string())?;
    let mut schema = HashMap::new();

    let sql = format!(
        "SELECT TABLE_NAME, COLUMN_NAME 
         FROM INFORMATION_SCHEMA.COLUMNS 
         WHERE TABLE_SCHEMA = '{}' 
         ORDER BY TABLE_NAME, ORDINAL_POSITION;",
        db_name
    );

    let rows: Vec<(String, String)> = conn
        .query_map(&sql, |(tbl, col)| (tbl, col))
        .map_err(|e| e.to_string())?;

    for (tbl, col) in rows {
        schema.entry(tbl).or_insert_with(Vec::new).push(col);
    }

    Ok(schema)
}

pub fn execute_mysql_stream<F>(
    dsn: &str,
    sql: &str,
    limit: usize,
    mut on_batch: F,
) -> Result<(), String>
where
    F: FnMut(Option<Vec<String>>, Vec<Vec<Option<String>>>),
{
    let opts = Opts::from_url(dsn).map_err(|e| e.to_string())?;
    let mut conn = Conn::new(opts).map_err(|e| e.to_string())?;

    let mut query_result = conn.query_iter(sql).map_err(|e| e.to_string())?;

    let cols: Vec<String> = query_result
        .columns()
        .as_ref()
        .iter()
        .map(|c| c.name_str().to_string())
        .collect();

    if !cols.is_empty() {
        let mut first = true;
        let mut total = 0;
        let mut batch = Vec::new();

        for row_res in query_result.by_ref() {
            let row = row_res.map_err(|e| e.to_string())?;
            let mut record = Vec::with_capacity(cols.len());

            for (i, _) in cols.iter().enumerate() {
                let val = &row[i];
                let cell = match val {
                    Value::NULL => None,
                    Value::Bytes(b) => Some(String::from_utf8_lossy(b).into_owned()),
                    Value::Int(v) => Some(v.to_string()),
                    Value::UInt(v) => Some(v.to_string()),
                    Value::Float(v) => Some(v.to_string()),
                    Value::Double(v) => Some(v.to_string()),
                    Value::Date(y, m, d, h, min, s, _) => {
                        Some(format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", y, m, d, h, min, s))
                    }
                    Value::Time(is_neg, d, h, m, s, _) => {
                        let sign = if *is_neg { "-" } else { "" };
                        Some(format!("{}{:02}:{:02}:{:02}", sign, d * 24 + (*h as u32), m, s))
                    }
                };
                record.push(cell);
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
        drop(query_result);
        let affected = conn.affected_rows();
        on_batch(
            Some(vec!["status".to_string()]),
            vec![vec![Some(format!("Rows affected: {}", affected))]],
        );
    }

    Ok(())
}
