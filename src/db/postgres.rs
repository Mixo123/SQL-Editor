//! PostgreSQL database support using postgres crate.

use postgres::types::Type;
use postgres::{Client, NoTls};
use std::collections::HashMap;

use super::types::SchemaMap;

pub fn test_postgres(dsn: &str) -> Result<(), String> {
    let mut client = Client::connect(dsn, NoTls).map_err(|e| e.to_string())?;
    client.simple_query("SELECT 1;").map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_postgres_schema(dsn: &str) -> Result<SchemaMap, String> {
    let mut client = Client::connect(dsn, NoTls).map_err(|e| e.to_string())?;
    let mut schema = HashMap::new();

    let tables_sql = "
        SELECT table_name 
        FROM information_schema.tables 
        WHERE table_schema NOT IN ('pg_catalog', 'information_schema')
        ORDER BY table_name;
    ";
    let rows = client.query(tables_sql, &[]).map_err(|e| e.to_string())?;
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
    let col_rows = client.query(cols_sql, &[]).map_err(|e| e.to_string())?;
    for row in col_rows {
        let tbl: String = row.get(0);
        let col: String = row.get(1);
        if let Some(list) = schema.get_mut(&tbl) {
            list.push(col);
        }
    }

    Ok(schema)
}

fn format_pg_cell(row: &postgres::Row, idx: usize, ty: &Type) -> Option<String> {
    if ty == &Type::BOOL {
        return row.get::<_, Option<bool>>(idx).map(|v| v.to_string());
    }
    if ty == &Type::INT2 {
        return row.get::<_, Option<i16>>(idx).map(|v| v.to_string());
    }
    if ty == &Type::INT4 {
        return row.get::<_, Option<i32>>(idx).map(|v| v.to_string());
    }
    if ty == &Type::INT8 {
        return row.get::<_, Option<i64>>(idx).map(|v| v.to_string());
    }
    if ty == &Type::FLOAT4 {
        return row.get::<_, Option<f32>>(idx).map(|v| v.to_string());
    }
    if ty == &Type::FLOAT8 {
        return row.get::<_, Option<f64>>(idx).map(|v| v.to_string());
    }
    if ty == &Type::TEXT || ty == &Type::VARCHAR || ty == &Type::BPCHAR || ty == &Type::NAME {
        return row.get::<_, Option<String>>(idx);
    }
    // General fallback
    if let Ok(opt_str) = row.try_get::<_, Option<String>>(idx) {
        return opt_str;
    }
    if let Ok(opt_bytes) = row.try_get::<_, Option<&[u8]>>(idx) {
        return opt_bytes.map(|b| format!("<{} bytes>", b.len()));
    }
    Some("<value>".to_string())
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
    let mut client = Client::connect(dsn, NoTls).map_err(|e| e.to_string())?;

    // Check if query produces rows
    let statement = client.prepare(sql).map_err(|e| e.to_string())?;
    let columns = statement.columns();

    if !columns.is_empty() {
        let col_names: Vec<String> = columns.iter().map(|c| c.name().to_string()).collect();
        let col_types: Vec<Type> = columns.iter().map(|c| c.type_().clone()).collect();
        let col_count = col_names.len();

        let rows = client.query(&statement, &[]).map_err(|e| e.to_string())?;

        let mut first = true;
        let mut total = 0;
        let mut batch = Vec::new();

        for row in rows {
            let mut record = Vec::with_capacity(col_count);
            for i in 0..col_count {
                let cell = format_pg_cell(&row, i, &col_types[i]);
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

        if !batch.is_empty() || first {
            on_batch(if first { Some(col_names) } else { None }, batch);
        }
    } else {
        let affected = client.execute(&statement, &[]).map_err(|e| e.to_string())?;
        on_batch(
            Some(vec!["status".to_string()]),
            vec![vec![Some(format!("Rows affected: {}", affected))]],
        );
    }

    Ok(())
}

