//! Databricks SQL support via the Databricks SQL Statement Execution REST API (`/api/2.0/sql/statements`).

use serde_json::Value;
use std::collections::HashMap;
use std::thread;
use std::time::Duration;

use super::types::{ConnectionMeta, SchemaMap};

/// Transport abstraction for Databricks REST API calls (enables fast, deterministic testing).
pub trait DatabricksTransport {
    fn post_json(&self, url: &str, token: &str, body: &str) -> Result<Value, String>;
    fn get_json(&self, url: &str, token: &str) -> Result<Value, String>;
}

pub struct UreqDatabricksTransport {
    agent: ureq::Agent,
}

impl UreqDatabricksTransport {
    pub fn new() -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(15))
            .timeout_read(Duration::from_secs(65))
            .timeout_write(Duration::from_secs(15))
            .build();
        Self { agent }
    }
}

impl Default for UreqDatabricksTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl DatabricksTransport for UreqDatabricksTransport {
    fn post_json(&self, url: &str, token: &str, body: &str) -> Result<Value, String> {
        let auth_header = format!("Bearer {}", token);
        let resp = self
            .agent
            .post(url)
            .set("Authorization", &auth_header)
            .set("Content-Type", "application/json")
            .set("Accept", "application/json")
            .send_string(body)
            .map_err(format_ureq_error)?;

        resp.into_json()
            .map_err(|e| format!("Invalid JSON response from Databricks: {}", e))
    }

    fn get_json(&self, url: &str, token: &str) -> Result<Value, String> {
        let auth_header = format!("Bearer {}", token);
        let resp = self
            .agent
            .get(url)
            .set("Authorization", &auth_header)
            .set("Accept", "application/json")
            .call()
            .map_err(format_ureq_error)?;

        resp.into_json()
            .map_err(|e| format!("Invalid JSON response from Databricks: {}", e))
    }
}

/// Normalizes a Databricks workspace hostname or URL into a clean base URL (`https://...` or `http://...`).
pub fn resolve_base_url(host: &str) -> Result<String, String> {
    let trimmed = host.trim();
    if trimmed.is_empty() {
        return Err("Databricks Server Hostname is required".to_string());
    }

    let (scheme, rest) = if let Some(r) = trimmed.strip_prefix("https://") {
        ("https://", r)
    } else if let Some(r) = trimmed.strip_prefix("http://") {
        ("http://", r)
    } else {
        ("https://", trimmed)
    };

    let authority = rest
        .split(&['/', '?', '#'][..])
        .next()
        .unwrap_or("")
        .trim()
        .trim_end_matches('/');

    if authority.is_empty() {
        return Err(format!("Invalid Databricks Server Hostname: {}", trimmed));
    }

    Ok(format!("{}{}", scheme, authority))
}

/// Extracts the SQL Warehouse ID from either a raw Warehouse ID (`1a2b3c4d5e6f7g8h`)
/// or a full Databricks HTTP Path (`/sql/1.0/warehouses/1a2b3c4d5e6f7g8h`).
pub fn extract_warehouse_id(path_or_id: &str) -> Result<String, String> {
    let clean = path_or_id
        .trim()
        .split(&['?', '#'][..])
        .next()
        .unwrap_or("")
        .trim_matches('/');

    if clean.is_empty() {
        return Err("Databricks SQL Warehouse ID or HTTP Path is required".to_string());
    }

    for marker in ["warehouses/", "endpoints/"] {
        if let Some(pos) = clean.find(marker) {
            let after = &clean[pos + marker.len()..];
            if let Some(seg) = after.split('/').find(|s| !s.is_empty()) {
                return Ok(seg.to_string());
            }
        }
    }

    if let Some(last) = clean.rsplit('/').find(|s| !s.is_empty()) {
        return Ok(last.to_string());
    }

    Ok(clean.to_string())
}

fn format_ureq_error(err: ureq::Error) -> String {
    match err {
        ureq::Error::Status(code, resp) => {
            let body = resp.into_string().unwrap_or_default();
            if let Ok(json) = serde_json::from_str::<Value>(&body) {
                let err_code = json
                    .get("error_code")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let msg = json
                    .get("message")
                    .and_then(|v| v.as_str())
                    .or_else(|| json.get("error").and_then(|v| v.as_str()))
                    .or_else(|| {
                        json.get("status")
                            .and_then(|s| s.get("error"))
                            .and_then(|e| e.get("message"))
                            .and_then(|v| v.as_str())
                    })
                    .unwrap_or(body.trim());

                if !err_code.is_empty() {
                    return format!("Databricks HTTP {} ({}): {}", code, err_code, msg);
                } else if !msg.is_empty() {
                    return format!("Databricks HTTP {}: {}", code, msg);
                }
            }
            if body.trim().is_empty() {
                format!("Databricks HTTP Error {}", code)
            } else {
                format!("Databricks HTTP {}: {}", code, body.trim())
            }
        }
        ureq::Error::Transport(t) => format!("Databricks connection error: {}", t),
    }
}

fn check_statement_status_error(json: &Value) -> Result<&str, String> {
    let state = json
        .get("status")
        .and_then(|s| s.get("state"))
        .and_then(|v| v.as_str())
        .unwrap_or("UNKNOWN");

    match state {
        "FAILED" | "CANCELED" | "CLOSED" => {
            let err_obj = json.get("status").and_then(|s| s.get("error"));
            let code = err_obj
                .and_then(|e| e.get("error_code"))
                .and_then(|v| v.as_str())
                .unwrap_or(state);
            let msg = err_obj
                .and_then(|e| e.get("message"))
                .and_then(|v| v.as_str())
                .unwrap_or("Statement execution failed");
            Err(format!("Databricks SQL Error ({}): {}", code, msg))
        }
        _ => Ok(state),
    }
}

fn json_cell_to_string(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        Value::String(s) => Some(s.clone()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_string()),
        other => Some(other.to_string()),
    }
}

pub fn execute_databricks_with_transport<F>(
    meta: &ConnectionMeta,
    sql: &str,
    limit: usize,
    transport: &dyn DatabricksTransport,
    poll_interval: Duration,
    mut on_batch: F,
) -> Result<(), String>
where
    F: FnMut(Option<Vec<String>>, Vec<Vec<Option<String>>>),
{
    let base_url = resolve_base_url(&meta.host)?;
    let warehouse_id = extract_warehouse_id(&meta.path)?;
    let token = meta.get_decrypted_password();
    let token_trimmed = token.trim();
    if token_trimmed.is_empty() {
        return Err("Databricks Access Token is required".to_string());
    }

    let mut payload = serde_json::json!({
        "statement": sql,
        "warehouse_id": warehouse_id,
        "wait_timeout": "30s",
        "on_wait_timeout": "CONTINUE",
        "disposition": "INLINE",
        "format": "JSON_ARRAY",
    });

    if limit > 0 {
        payload["row_limit"] = serde_json::json!(limit);
    }
    if !meta.project.trim().is_empty() {
        payload["catalog"] = serde_json::json!(meta.project.trim());
    }
    if !meta.db.trim().is_empty() {
        payload["schema"] = serde_json::json!(meta.db.trim());
    }

    let submit_url = format!("{}/api/2.0/sql/statements", base_url);
    let body_str = serde_json::to_string(&payload).map_err(|e| e.to_string())?;
    let mut resp_json = transport.post_json(&submit_url, token_trimmed, &body_str)?;

    let statement_id = resp_json
        .get("statement_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // Poll if query is still PENDING or RUNNING
    let mut attempts = 0;
    loop {
        let state = check_statement_status_error(&resp_json)?;
        if state == "SUCCEEDED" {
            break;
        }
        if state != "PENDING" && state != "RUNNING" {
            return Err(format!("Unexpected Databricks statement state: {}", state));
        }
        if statement_id.is_empty() {
            return Err("Databricks returned PENDING/RUNNING without a statement_id".to_string());
        }
        attempts += 1;
        if attempts > 600 {
            return Err("Databricks query timed out waiting for statement completion".to_string());
        }
        thread::sleep(poll_interval);

        let poll_url = format!("{}/api/2.0/sql/statements/{}", base_url, statement_id);
        resp_json = transport.get_json(&poll_url, token_trimmed)?;
    }

    // Extract column names from manifest.schema.columns
    let cols: Vec<String> = resp_json
        .get("manifest")
        .and_then(|m| m.get("schema"))
        .and_then(|s| s.get("columns"))
        .and_then(|c| c.as_array())
        .map(|arr| {
            arr.iter()
                .enumerate()
                .map(|(i, col_obj)| {
                    col_obj
                        .get("name")
                        .and_then(|n| n.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| format!("col_{}", i + 1))
                })
                .collect()
        })
        .unwrap_or_default();

    if cols.is_empty() {
        on_batch(
            Some(vec!["status".to_string()]),
            vec![vec![Some("Statement executed successfully".to_string())]],
        );
        return Ok(());
    }

    let col_count = cols.len();
    let mut first = true;
    let mut total = 0usize;
    let mut batch: Vec<Vec<Option<String>>> = Vec::new();

    let mut current_result = resp_json.get("result").cloned();

    while let Some(res_obj) = current_result {
        if let Some(data_array) = res_obj.get("data_array").and_then(|d| d.as_array()) {
            for row_val in data_array {
                if let Some(cells) = row_val.as_array() {
                    let mut record = Vec::with_capacity(col_count);
                    for i in 0..col_count {
                        let str_val = cells.get(i).and_then(json_cell_to_string);
                        record.push(str_val);
                    }
                    batch.push(record);
                }

                if batch.len() >= 2000 {
                    on_batch(if first { Some(cols.clone()) } else { None }, batch);
                    first = false;
                    batch = Vec::new();
                }

                total += 1;
                if limit > 0 && total >= limit {
                    break;
                }
            }
        }

        if limit > 0 && total >= limit {
            break;
        }

        // Check if there is a next chunk
        let next_link = res_obj
            .get("next_chunk_internal_link")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| {
                if s.starts_with('/') {
                    s.to_string()
                } else {
                    format!("/{}", s)
                }
            })
            .or_else(|| {
                res_obj
                    .get("next_chunk_index")
                    .and_then(|v| v.as_u64())
                    .filter(|_| !statement_id.is_empty())
                    .map(|idx| {
                        format!(
                            "/api/2.0/sql/statements/{}/result/chunks/{}",
                            statement_id, idx
                        )
                    })
            });

        if let Some(link) = next_link {
            let chunk_url = format!("{}{}", base_url, link);
            let chunk_json = transport.get_json(&chunk_url, token_trimmed)?;
            current_result = Some(chunk_json);
        } else {
            current_result = None;
        }
    }

    if !batch.is_empty() || first {
        on_batch(if first { Some(cols) } else { None }, batch);
    }

    Ok(())
}

pub fn execute_databricks_stream<F>(
    meta: &ConnectionMeta,
    sql: &str,
    limit: usize,
    on_batch: F,
) -> Result<(), String>
where
    F: FnMut(Option<Vec<String>>, Vec<Vec<Option<String>>>),
{
    let transport = UreqDatabricksTransport::new();
    execute_databricks_with_transport(
        meta,
        sql,
        limit,
        &transport,
        Duration::from_millis(500),
        on_batch,
    )
}

pub fn test_databricks(meta: &ConnectionMeta) -> Result<(), String> {
    execute_databricks_stream(meta, "SELECT 1", 1, |_, _| {})
}

pub fn get_databricks_schema_with_transport(
    meta: &ConnectionMeta,
    transport: &dyn DatabricksTransport,
    poll_interval: Duration,
) -> Result<SchemaMap, String> {
    let mut schema: SchemaMap = HashMap::new();

    // 1. Try standard Unity Catalog information_schema.columns
    let info_sql = if !meta.db.trim().is_empty() {
        let escaped_schema = meta.db.trim().replace('\'', "''");
        format!(
            "SELECT table_name, column_name FROM information_schema.columns \
             WHERE table_schema = '{}' \
             ORDER BY table_name, ordinal_position LIMIT 5000",
            escaped_schema
        )
    } else {
        "SELECT table_name, column_name FROM information_schema.columns \
         WHERE table_schema NOT IN ('information_schema', 'sys') \
         ORDER BY table_name, ordinal_position LIMIT 5000"
            .to_string()
    };

    let info_res = execute_databricks_with_transport(
        meta,
        &info_sql,
        5000,
        transport,
        poll_interval,
        |_, rows| {
            for r in rows {
                if let (Some(Some(tbl)), Some(Some(col))) = (r.first(), r.get(1)) {
                    if !tbl.is_empty() && !col.is_empty() {
                        schema.entry(tbl.clone()).or_default().push(col.clone());
                    }
                }
            }
        },
    );

    if info_res.is_ok() && !schema.is_empty() {
        return Ok(schema);
    }

    // 2. Fallback to SHOW TABLES (works on hive_metastore and restricted catalogs)
    let mut tbl_names = Vec::new();
    let _ = execute_databricks_with_transport(
        meta,
        "SHOW TABLES",
        2000,
        transport,
        poll_interval,
        |cols, rows| {
            let tbl_idx = cols
                .as_ref()
                .and_then(|c| {
                    c.iter().position(|name| {
                        name.eq_ignore_ascii_case("tableName")
                            || name.eq_ignore_ascii_case("table_name")
                    })
                })
                .unwrap_or(1);

            for r in rows {
                if let Some(Some(tbl)) = r.get(tbl_idx).or_else(|| r.first()) {
                    if !tbl.is_empty() {
                        tbl_names.push(tbl.clone());
                    }
                }
            }
        },
    );

    for t in tbl_names {
        schema.entry(t).or_default();
    }

    Ok(schema)
}

pub fn get_databricks_schema(meta: &ConnectionMeta) -> Result<SchemaMap, String> {
    let transport = UreqDatabricksTransport::new();
    get_databricks_schema_with_transport(meta, &transport, Duration::from_millis(500))
}
