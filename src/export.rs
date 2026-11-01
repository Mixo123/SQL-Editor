//! Export query results to CSV, JSON, XLSX, and Markdown.

use std::fs::File;
use std::io::Write;
use std::path::Path;

pub fn export_csv(path: &Path, columns: &[String], rows: &[Vec<Option<String>>]) -> Result<(), String> {
    let file = File::create(path).map_err(|e| e.to_string())?;
    let mut writer = csv::Writer::from_writer(file);

    writer.write_record(columns).map_err(|e| e.to_string())?;

    for row in rows {
        let record: Vec<String> = row
            .iter()
            .map(|opt| opt.as_deref().unwrap_or("").to_string())
            .collect();
        writer.write_record(&record).map_err(|e| e.to_string())?;
    }

    writer.flush().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn export_json(path: &Path, columns: &[String], rows: &[Vec<Option<String>>]) -> Result<(), String> {
    let mut json_rows = Vec::new();

    for row in rows {
        let mut map = serde_json::Map::new();
        for (i, col) in columns.iter().enumerate() {
            let val = if i < row.len() {
                match &row[i] {
                    Some(s) => serde_json::Value::String(s.clone()),
                    None => serde_json::Value::Null,
                }
            } else {
                serde_json::Value::Null
            };
            map.insert(col.clone(), val);
        }
        json_rows.push(serde_json::Value::Object(map));
    }

    let payload = serde_json::json!({
        "columns": columns,
        "rows": json_rows,
    });

    let s = serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())?;
    let mut file = File::create(path).map_err(|e| e.to_string())?;
    file.write_all(s.as_bytes()).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn export_markdown(path: &Path, columns: &[String], rows: &[Vec<Option<String>>]) -> Result<(), String> {
    fn clean_cell(val: Option<&str>) -> String {
        match val {
            Some(s) => s
                .replace('\\', "\\\\")
                .replace('|', "\\|")
                .replace("\r\n", "<br>")
                .replace('\n', "<br>"),
            None => "NULL".to_string(),
        }
    }

    let mut out = String::new();
    out.push_str("| ");
    out.push_str(&columns.iter().map(|c| clean_cell(Some(c))).collect::<Vec<_>>().join(" | "));
    out.push_str(" |\n");

    out.push_str("| ");
    out.push_str(&columns.iter().map(|_| "---").collect::<Vec<_>>().join(" | "));
    out.push_str(" |\n");

    for row in rows {
        out.push_str("| ");
        let cells: Vec<String> = columns
            .iter()
            .enumerate()
            .map(|(i, _)| {
                let v = if i < row.len() { row[i].as_deref() } else { None };
                clean_cell(v)
            })
            .collect();
        out.push_str(&cells.join(" | "));
        out.push_str(" |\n");
    }

    let mut file = File::create(path).map_err(|e| e.to_string())?;
    file.write_all(out.as_bytes()).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn export_xlsx(path: &Path, columns: &[String], rows: &[Vec<Option<String>>]) -> Result<(), String> {
    use rust_xlsxwriter::*;

    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();

    // Write header
    for (col_idx, col_name) in columns.iter().enumerate() {
        worksheet
            .write_string(0, col_idx as u16, col_name)
            .map_err(|e| e.to_string())?;
    }

    // Write data rows
    for (row_idx, row) in rows.iter().enumerate() {
        let r = (row_idx + 1) as u32;
        for (col_idx, val) in row.iter().enumerate() {
            let c = col_idx as u16;
            match val {
                Some(s) => {
                    // Try parsing as number
                    if let Ok(num) = s.parse::<f64>() {
                        worksheet.write_number(r, c, num).map_err(|e| e.to_string())?;
                    } else if s.eq_ignore_ascii_case("true") || s.eq_ignore_ascii_case("false") {
                        worksheet.write_boolean(r, c, s.eq_ignore_ascii_case("true")).map_err(|e| e.to_string())?;
                    } else {
                        worksheet.write_string(r, c, s).map_err(|e| e.to_string())?;
                    }
                }
                None => {
                    worksheet.write_string(r, c, "NULL").map_err(|e| e.to_string())?;
                }
            }
        }
    }

    workbook.save(path).map_err(|e| e.to_string())?;
    Ok(())
}

