//! Fast SQL syntax highlighting for egui using regex tokenization.

use egui::text::LayoutJob;
use egui::{FontId, TextFormat};
use regex::Regex;
use std::collections::HashSet;
use std::sync::OnceLock;

use crate::config::MAX_HIGHLIGHT_CHARS;
use crate::theme::Theme;

static KEYWORDS: OnceLock<HashSet<&'static str>> = OnceLock::new();

pub fn get_keywords() -> &'static HashSet<&'static str> {
    KEYWORDS.get_or_init(|| {
        [
            "SELECT", "FROM", "WHERE", "INSERT", "INTO", "VALUES", "UPDATE", "SET",
            "DELETE", "CREATE", "TABLE", "INDEX", "VIEW", "DROP", "ALTER", "ADD",
            "COLUMN", "PRIMARY", "KEY", "FOREIGN", "REFERENCES", "NOT", "NULL", "DEFAULT",
            "UNIQUE", "CHECK", "CONSTRAINT", "JOIN", "INNER", "LEFT", "RIGHT",
            "OUTER", "FULL", "CROSS", "ON", "AS", "AND", "OR", "IN", "BETWEEN",
            "LIKE", "ILIKE", "IS", "EXISTS", "GROUP", "BY", "ORDER", "ASC", "DESC",
            "HAVING", "LIMIT", "OFFSET", "UNION", "ALL", "DISTINCT", "CASE", "WHEN",
            "THEN", "ELSE", "END", "CAST", "COALESCE", "NULLIF", "BEGIN", "COMMIT",
            "ROLLBACK", "TRANSACTION", "IF", "WITH", "RECURSIVE", "RETURNING",
            "TRUE", "FALSE", "COUNT", "SUM", "AVG", "MIN", "MAX", "OVER", "PARTITION",
            "SHOW", "DESCRIBE", "EXPLAIN", "TRUNCATE", "CASCADE",
            "TOP", "EXEC", "EXECUTE", "PROCEDURE", "DECLARE", "DUAL", "ROWNUM",
            "STRUCT", "ARRAY", "UNNEST", "QUALIFY", "EXCEPT", "MERGE", "MATCHED",
        ]
        .into_iter()
        .collect()
    })
}

static TOKEN_REGEX: OnceLock<Regex> = OnceLock::new();

fn get_token_regex() -> &'static Regex {
    TOKEN_REGEX.get_or_init(|| {
        Regex::new(concat!(
            r"(?P<comment>--[^\n]*|/\*[\s\S]*?\*/)|",
            r"(?P<string>'(?:''|[^'])*')|",
            r"(?P<number>\b\d+(?:\.\d+)?\b)|",
            r"(?P<word>\b[A-Za-z_][A-Za-z0-9_]*\b)"
        ))
        .unwrap()
    })
}

pub fn highlight_sql(
    text: &str,
    theme: &Theme,
    font_id: FontId,
    enabled: bool,
) -> LayoutJob {
    let mut job = LayoutJob::default();
    let default_color = theme.c_fg();

    if !enabled || text.len() > MAX_HIGHLIGHT_CHARS {
        job.append(
            text,
            0.0,
            TextFormat {
                font_id,
                color: default_color,
                ..Default::default()
            },
        );
        return job;
    }

    let re = get_token_regex();
    let keywords = get_keywords();
    let mut last_idx = 0;

    for caps in re.captures_iter(text) {
        let m = caps.get(0).unwrap();
        let start = m.start();
        let end = m.end();

        // Plain text before token
        if start > last_idx {
            job.append(
                &text[last_idx..start],
                0.0,
                TextFormat {
                    font_id: font_id.clone(),
                    color: default_color,
                    ..Default::default()
                },
            );
        }

        let (token_color, _) = if caps.name("comment").is_some() {
            (theme.c_cmt(), "comment")
        } else if caps.name("string").is_some() {
            (theme.c_str(), "string")
        } else if caps.name("number").is_some() {
            (theme.c_num(), "number")
        } else if let Some(word) = caps.name("word") {
            let upper = word.as_str().to_uppercase();
            if keywords.contains(upper.as_str()) {
                (theme.c_kw(), "kw")
            } else {
                (theme.c_ident(), "ident")
            }
        } else {
            (default_color, "default")
        };

        job.append(
            &text[start..end],
            0.0,
            TextFormat {
                font_id: font_id.clone(),
                color: token_color,
                ..Default::default()
            },
        );

        last_idx = end;
    }

    if last_idx < text.len() {
        job.append(
            &text[last_idx..],
            0.0,
            TextFormat {
                font_id,
                color: default_color,
                ..Default::default()
            },
        );
    }

    job
}
