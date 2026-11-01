//! SQL Autocomplete engine.
//! Provides context-aware suggestions for tables, columns, keywords, and functions.

use regex::Regex;
use std::collections::HashMap;

pub const SQL_KEYWORDS: &[&str] = &[
    "SELECT", "FROM", "WHERE", "INSERT", "INTO", "VALUES", "UPDATE", "SET",
    "DELETE", "CREATE", "TABLE", "INDEX", "VIEW", "DROP", "ALTER", "ADD",
    "COLUMN", "PRIMARY", "KEY", "FOREIGN", "REFERENCES", "NOT", "NULL", "DEFAULT",
    "UNIQUE", "CHECK", "CONSTRAINT", "JOIN", "INNER", "LEFT", "RIGHT",
    "OUTER", "FULL", "CROSS", "ON", "AS", "AND", "OR", "IN", "BETWEEN",
    "LIKE", "ILIKE", "IS", "EXISTS", "GROUP", "BY", "ORDER", "ASC", "DESC",
    "HAVING", "LIMIT", "OFFSET", "UNION", "ALL", "DISTINCT", "CASE", "WHEN",
    "THEN", "ELSE", "END", "CAST", "COALESCE", "NULLIF", "BEGIN", "COMMIT",
    "ROLLBACK", "TRANSACTION", "IF", "WITH", "RECURSIVE", "RETURNING",
    "TRUE", "FALSE", "OVER", "PARTITION", "SHOW", "DESCRIBE", "EXPLAIN",
    "TRUNCATE", "CASCADE", "TOP", "EXEC", "EXECUTE", "PROCEDURE", "DECLARE",
    "DUAL", "ROWNUM", "STRUCT", "ARRAY", "UNNEST", "QUALIFY", "EXCEPT", "MERGE", "MATCHED",
];

pub const SQL_FUNCTIONS: &[&str] = &[
    "COUNT", "SUM", "AVG", "MIN", "MAX",
    "COALESCE", "NULLIF", "CONCAT", "SUBSTRING", "LENGTH", "TRIM",
    "LOWER", "UPPER", "REPLACE", "ROUND", "FLOOR", "CEIL", "ABS", "MOD",
    "NOW", "CURRENT_DATE", "CURRENT_TIME", "CURRENT_TIMESTAMP", "DATE",
    "ROW_NUMBER", "RANK", "DENSE_RANK", "LEAD", "LAG",
    "NVL", "NVL2", "SYSDATE", "TO_DATE", "TO_CHAR", "DECODE",
    "SAFE_CAST", "GETDATE", "DATEADD", "DATEDIFF", "ISNULL",
    "PARSE_DATE", "FORMAT_DATE",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    pub text: String,
    pub kind: String, // "keyword", "function", "table", "column"
    pub display: String,
}

pub fn is_in_comment_or_string(text_before: &str) -> bool {
    let last_block_open = text_before.rfind("/*");
    let last_block_close = text_before.rfind("*/");
    if let Some(open) = last_block_open {
        if let Some(close) = last_block_close {
            if open > close {
                return true;
            }
        } else {
            return true;
        }
    }

    if let Some(last_line) = text_before.split('\n').last() {
        if let Some(dash_idx) = last_line.rfind("--") {
            let prefix_line = &last_line[..dash_idx];
            if prefix_line.matches('\'').count() % 2 == 0 {
                return true;
            }
        }
    }

    let no_escaped = text_before.replace("''", "");
    if no_escaped.matches('\'').count() % 2 == 1 {
        return true;
    }

    false
}

pub fn get_current_statement(text_before: &str) -> &str {
    let mut last_semi = None;
    let bytes = text_before.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        if text_before[index..].starts_with("--") || bytes[index] == b'#' {
            let newline = text_before[index..].find('\n');
            index = match newline {
                Some(nl) => index + nl + 1,
                None => bytes.len(),
            };
            continue;
        }
        if text_before[index..].starts_with("/*") {
            let end = text_before[index + 2..].find("*/");
            index = match end {
                Some(e) => index + 2 + e + 2,
                None => bytes.len(),
            };
            continue;
        }
        let char = bytes[index] as char;
        if char == '\'' || char == '"' || char == '`' {
            let quote = char;
            index += 1;
            while index < bytes.len() {
                if bytes[index] == b'\\' && quote == '\'' {
                    index += 2;
                } else if bytes[index] as char == quote {
                    if index + 1 < bytes.len() && bytes[index + 1] as char == quote {
                        index += 2;
                    } else {
                        index += 1;
                        break;
                    }
                } else {
                    index += 1;
                }
            }
            continue;
        }
        if char == ';' {
            last_semi = Some(index);
        }
        index += 1;
    }

    if let Some(semi) = last_semi {
        &text_before[semi + 1..]
    } else {
        text_before
    }
}

pub fn get_autocomplete_suggestions(
    text_before: &str,
    schema: &HashMap<String, Vec<String>>,
    full_text: Option<&str>,
) -> (Vec<Suggestion>, usize) {
    if is_in_comment_or_string(text_before) {
        return (Vec::new(), 0);
    }

    let curr_stmt = get_current_statement(text_before);
    if curr_stmt.trim().is_empty() {
        return (Vec::new(), 0);
    }

    let full = full_text.unwrap_or(text_before);

    let re_block = Regex::new(r"/\*[\s\S]*?\*/").unwrap();
    let re_line = Regex::new(r"--[^\n]*").unwrap();

    let cleaned = re_block.replace_all(curr_stmt, " ");
    let cleaned = re_line.replace_all(&cleaned, " ");
    let cleaned_full = re_block.replace_all(full, " ");
    let cleaned_full = re_line.replace_all(&cleaned_full, " ");

    let all_tables: Vec<String> = schema.keys().cloned().collect();
    let mut all_cols: Vec<String> = Vec::new();
    for tbl_cols in schema.values() {
        for c in tbl_cols {
            if !all_cols.contains(c) {
                all_cols.push(c.clone());
            }
        }
    }

    // 1. Check dot notation: table.prefix or alias.prefix
    let dot_re = Regex::new(r"([A-Za-z0-9_]+)\.([A-Za-z0-9_]*)$").unwrap();
    if let Some(caps) = dot_re.captures(&cleaned) {
        let target = caps.get(1).unwrap().as_str();
        let prefix = caps.get(2).unwrap().as_str();
        let mut target_tbl: Option<String> = None;

        for t in &all_tables {
            if t.eq_ignore_ascii_case(target) {
                target_tbl = Some(t.clone());
                break;
            }
        }

        if target_tbl.is_none() {
            // Check alias in query: FROM table AS alias or FROM table alias
            let alias_pat = format!(
                r"(?i)\b(?:FROM|JOIN)\s+([A-Za-z0-9_]+)\s+(?:AS\s+)?{}\b",
                regex::escape(target)
            );
            if let Ok(alias_re) = Regex::new(&alias_pat) {
                if let Some(ac) = alias_re.captures(&cleaned_full) {
                    let aliased = ac.get(1).unwrap().as_str();
                    for t in &all_tables {
                        if t.eq_ignore_ascii_case(aliased) {
                            target_tbl = Some(t.clone());
                            break;
                        }
                    }
                }
            }
        }

        if let Some(tbl) = target_tbl {
            if let Some(cols) = schema.get(&tbl) {
                let pref_low = prefix.to_lowercase();
                let matches: Vec<Suggestion> = cols
                    .iter()
                    .filter(|c| c.to_lowercase().starts_with(&pref_low))
                    .map(|c| Suggestion {
                        text: c.clone(),
                        kind: "column".to_string(),
                        display: format!("{:<24} [column]", c),
                    })
                    .collect();
                return (matches, prefix.len());
            }
        }
    }

    // 2. Check current word / prefix being typed
    let word_re = Regex::new(r"([A-Za-z0-9_]+)$").unwrap();
    let (prefix, before_word) = if let Some(caps) = word_re.captures(&cleaned) {
        let m = caps.get(1).unwrap();
        (m.as_str(), &cleaned[..m.start()])
    } else {
        ("", cleaned.as_ref())
    };

    let pref_low = prefix.to_lowercase();

    // Find the last significant SQL keyword before the current word / whitespace
    let token_re = Regex::new(r"\b[A-Za-z_][A-Za-z0-9_]*\b|[,\(\)]").unwrap();
    let tokens: Vec<&str> = token_re.find_iter(before_word).map(|m| m.as_str()).collect();

    let mut last_kw = None;
    let mut last_kw_idx = None;

    for (idx, &token) in tokens.iter().enumerate().rev() {
        let u = token.to_uppercase();
        if matches!(
            u.as_str(),
            "FROM" | "JOIN" | "INTO" | "UPDATE" | "TABLE" | "SELECT" | "WHERE" | "SET" | "ON" | "HAVING" | "BY" | "AND" | "OR"
        ) {
            last_kw = Some(u);
            last_kw_idx = Some(idx);
            break;
        }
    }

    let tokens_after_kw = if let Some(idx) = last_kw_idx {
        &tokens[idx + 1..]
    } else {
        &[]
    };

    // Find tables referenced in query to prioritize their columns
    let mut priority_cols = Vec::new();
    let mut other_cols = Vec::new();
    for t in &all_tables {
        let pat = format!(r"(?i)\b{}\b", regex::escape(t));
        if let Ok(r) = Regex::new(&pat) {
            if r.is_match(&cleaned) {
                if let Some(cols) = schema.get(t) {
                    for c in cols {
                        if !priority_cols.contains(c) {
                            priority_cols.push(c.clone());
                        }
                    }
                }
            }
        }
    }
    for c in &all_cols {
        if !priority_cols.contains(c) {
            other_cols.push(c.clone());
        }
    }
    let mut ordered_cols = priority_cols;
    ordered_cols.extend(other_cols);

    let mut results: Vec<Suggestion> = Vec::new();

    match last_kw.as_deref() {
        Some("FROM") | Some("JOIN") | Some("INTO") | Some("UPDATE") | Some("TABLE") => {
            let table_already_specified =
                !tokens_after_kw.is_empty() && *tokens_after_kw.last().unwrap() != ",";

            if !table_already_specified {
                for t in &all_tables {
                    if t.to_lowercase().starts_with(&pref_low) {
                        results.push(Suggestion {
                            text: t.clone(),
                            kind: "table".to_string(),
                            display: format!("{:<24} [table]", t),
                        });
                    }
                }
                if !prefix.is_empty() {
                    for &k in &["JOIN", "LEFT", "RIGHT", "INNER", "WHERE", "ORDER", "GROUP", "LIMIT"] {
                        if k.to_lowercase().starts_with(&pref_low)
                            && !results.iter().any(|r| r.text.eq_ignore_ascii_case(k))
                        {
                            results.push(Suggestion {
                                text: k.to_string(),
                                kind: "keyword".to_string(),
                                display: format!("{:<24} [keyword]", k),
                            });
                        }
                    }
                }
            } else if !prefix.is_empty() {
                for &k in &[
                    "WHERE", "JOIN", "LEFT", "RIGHT", "INNER", "CROSS", "ORDER", "GROUP",
                    "LIMIT", "HAVING", "AS", "ON",
                ] {
                    if k.to_lowercase().starts_with(&pref_low) {
                        results.push(Suggestion {
                            text: k.to_string(),
                            kind: "keyword".to_string(),
                            display: format!("{:<24} [keyword]", k),
                        });
                    }
                }
            }
        }
        Some("SELECT") | Some("WHERE") | Some("SET") | Some("ON") | Some("HAVING") | Some("BY") | Some("AND") | Some("OR") => {
            if last_kw.as_deref() == Some("SELECT")
                && !tokens_after_kw.is_empty()
                && *tokens_after_kw.last().unwrap() != ","
            {
                if !prefix.is_empty() {
                    for &k in &["FROM", "AS", "DISTINCT", "WHERE"] {
                        if k.to_lowercase().starts_with(&pref_low) {
                            results.push(Suggestion {
                                text: k.to_string(),
                                kind: "keyword".to_string(),
                                display: format!("{:<24} [keyword]", k),
                            });
                        }
                    }
                }
            } else {
                for c in &ordered_cols {
                    if c.to_lowercase().starts_with(&pref_low) {
                        results.push(Suggestion {
                            text: c.clone(),
                            kind: "column".to_string(),
                            display: format!("{:<24} [column]", c),
                        });
                    }
                }
                for &f in SQL_FUNCTIONS {
                    if f.to_lowercase().starts_with(&pref_low) {
                        results.push(Suggestion {
                            text: f.to_string(),
                            kind: "function".to_string(),
                            display: format!("{:<24} [function]", f),
                        });
                    }
                }
                if !prefix.is_empty() {
                    for &k in &[
                        "DISTINCT", "AS", "CASE", "WHEN", "THEN", "ELSE", "END", "FROM", "AND",
                        "OR", "IN", "IS", "NOT", "NULL", "LIKE", "BETWEEN",
                    ] {
                        if k.to_lowercase().starts_with(&pref_low)
                            && !results.iter().any(|r| r.text.eq_ignore_ascii_case(k))
                        {
                            results.push(Suggestion {
                                text: k.to_string(),
                                kind: "keyword".to_string(),
                                display: format!("{:<24} [keyword]", k),
                            });
                        }
                    }
                }
            }
        }
        _ => {
            if !prefix.is_empty() {
                for &k in SQL_KEYWORDS {
                    if k.to_lowercase().starts_with(&pref_low) {
                        results.push(Suggestion {
                            text: k.to_string(),
                            kind: "keyword".to_string(),
                            display: format!("{:<24} [keyword]", k),
                        });
                    }
                }
                for &f in SQL_FUNCTIONS {
                    if f.to_lowercase().starts_with(&pref_low)
                        && !results.iter().any(|r| r.text.eq_ignore_ascii_case(f))
                    {
                        results.push(Suggestion {
                            text: f.to_string(),
                            kind: "function".to_string(),
                            display: format!("{:<24} [function]", f),
                        });
                    }
                }
                for t in &all_tables {
                    if t.to_lowercase().starts_with(&pref_low)
                        && !results.iter().any(|r| r.text == *t)
                    {
                        results.push(Suggestion {
                            text: t.clone(),
                            kind: "table".to_string(),
                            display: format!("{:<24} [table]", t),
                        });
                    }
                }
                for c in &ordered_cols {
                    if c.to_lowercase().starts_with(&pref_low)
                        && !results.iter().any(|r| r.text == *c)
                    {
                        results.push(Suggestion {
                            text: c.clone(),
                            kind: "column".to_string(),
                            display: format!("{:<24} [column]", c),
                        });
                    }
                }
            }
        }
    }

    (results, prefix.len())
}

