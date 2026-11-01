//! Data structures for connections, query streaming, and SQL statement analysis.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::crypto::{decrypt_str, encrypt_str};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionMeta {
    pub name: String,
    pub kind: String, // "sqlite", "postgres", "mysql", "oracle", "mssql", "bigquery"
    #[serde(default)]
    pub host: String,
    #[serde(default)]
    pub port: String,
    #[serde(default)]
    pub db: String,
    #[serde(default)]
    pub user: String,
    #[serde(default)]
    pub password: String, // encrypted in json
    #[serde(default)]
    pub ssl: bool,
    #[serde(default)]
    pub path: String, // sqlite file or bigquery json credentials
    #[serde(default)]
    pub project: String,
    #[serde(default)]
    pub location: String,
}

impl ConnectionMeta {
    pub fn get_decrypted_password(&self) -> String {
        decrypt_str(&self.password)
    }

    pub fn set_password_plain(&mut self, plain: &str) {
        self.password = encrypt_str(plain);
    }

    pub fn build_dsn(&self) -> String {
        let plain_pwd = self.get_decrypted_password();
        match self.kind.as_str() {
            "sqlite" => self.path.clone(),
            "mysql" => {
                let host = if self.host.is_empty() { "localhost" } else { &self.host };
                let port = if self.port.is_empty() { "3306" } else { &self.port };
                let mut auth = urlencoding::encode(&self.user).to_string();
                if !plain_pwd.is_empty() {
                    auth.push(':');
                    auth.push_str(&urlencoding::encode(&plain_pwd));
                }
                let ssl = if self.ssl { "?ssl=true" } else { "" };
                format!("mysql://{}@{}:{}/{}{}", auth, host, port, self.db, ssl)
            }
            "oracle" => {
                let host = if self.host.is_empty() { "localhost" } else { &self.host };
                let port = if self.port.is_empty() { "1521" } else { &self.port };
                let mut auth = urlencoding::encode(&self.user).to_string();
                if !plain_pwd.is_empty() {
                    auth.push(':');
                    auth.push_str(&urlencoding::encode(&plain_pwd));
                }
                let ssl = if self.ssl { "?ssl=true" } else { "" };
                format!("oracle://{}@{}:{}/{}{}", auth, host, port, self.db, ssl)
            }
            "mssql" => {
                let host = if self.host.is_empty() { "localhost" } else { &self.host };
                let port = if self.port.is_empty() { "1433" } else { &self.port };
                let mut auth = urlencoding::encode(&self.user).to_string();
                if !plain_pwd.is_empty() {
                    auth.push(':');
                    auth.push_str(&urlencoding::encode(&plain_pwd));
                }
                let ssl = if self.ssl { "?ssl=true" } else { "" };
                let db = if self.db.is_empty() { "master" } else { &self.db };
                format!("mssql://{}@{}:{}/{}{}", auth, host, port, db, ssl)
            }
            "bigquery" => {
                let project = if !self.project.is_empty() {
                    &self.project
                } else {
                    &self.host
                };
                let mut params = Vec::new();
                if !self.path.is_empty() {
                    params.push(format!("credentials={}", urlencoding::encode(&self.path)));
                }
                if !self.location.is_empty() {
                    params.push(format!("location={}", urlencoding::encode(&self.location)));
                }
                let q = if params.is_empty() {
                    String::new()
                } else {
                    format!("?{}", params.join("&"))
                };
                let ds = if self.db.is_empty() {
                    String::new()
                } else {
                    format!("/{}", self.db)
                };
                format!("bigquery://{}{}{}", project, ds, q)
            }
            _ => {
                // postgres
                let host = if self.host.is_empty() { "localhost" } else { &self.host };
                let port = if self.port.is_empty() { "5432" } else { &self.port };
                let mut auth = urlencoding::encode(&self.user).to_string();
                if !plain_pwd.is_empty() {
                    auth.push(':');
                    auth.push_str(&urlencoding::encode(&plain_pwd));
                }
                let ssl = if self.ssl { "?sslmode=require" } else { "" };
                format!("postgresql://{}@{}:{}/{}{}", auth, host, port, self.db, ssl)
            }
        }
    }
}

pub type SchemaMap = HashMap<String, Vec<String>>;

pub fn split_sql_statements(sql: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut start = 0;
    let mut index = 0;
    let mut has_content = false;
    let bytes = sql.as_bytes();

    while index < bytes.len() {
        if sql[index..].starts_with("--") || bytes[index] == b'#' {
            let newline = sql[index..].find('\n');
            index = match newline {
                Some(nl) => index + nl + 1,
                None => bytes.len(),
            };
            continue;
        }
        if sql[index..].starts_with("/*") {
            let end = sql[index + 2..].find("*/");
            index = match end {
                Some(e) => index + 2 + e + 2,
                None => bytes.len(),
            };
            continue;
        }

        let char = bytes[index] as char;
        if char == '$' {
            let re_delim = Regex::new(r"^\$[A-Za-z_0-9]*\$").unwrap();
            if let Some(m) = re_delim.find(&sql[index..]) {
                has_content = true;
                let token = m.as_str();
                let end = sql[index + token.len()..].find(token);
                index = match end {
                    Some(e) => index + token.len() + e + token.len(),
                    None => bytes.len(),
                };
                continue;
            }
        }
        if char == '\'' || char == '"' || char == '`' {
            has_content = true;
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
        if char == '[' {
            has_content = true;
            index += 1;
            while index < bytes.len() {
                if index + 1 < bytes.len() && &sql[index..index + 2] == "]]" {
                    index += 2;
                } else if bytes[index] as char == ']' {
                    index += 1;
                    break;
                } else {
                    index += 1;
                }
            }
            continue;
        }
        if char == ';' {
            let stmt = sql[start..index].trim();
            if has_content && !stmt.is_empty() {
                statements.push(stmt.to_string());
            }
            start = index + 1;
            has_content = false;
        } else if !char.is_whitespace() {
            has_content = true;
        }
        index += 1;
    }

    let stmt = sql[start..].trim();
    if has_content && !stmt.is_empty() {
        statements.push(stmt.to_string());
    }

    statements
}

pub fn is_unbounded_select(sql: &str) -> bool {
    let mut keywords = Vec::new();
    let mut depth: i32 = 0;
    let mut index = 0;
    let bytes = sql.as_bytes();
    let word_re = Regex::new(r"^[A-Za-z_][A-Za-z_0-9$]*").unwrap();

    while index < bytes.len() {
        if sql[index..].starts_with("--") || bytes[index] == b'#' {
            let newline = sql[index..].find('\n');
            index = match newline {
                Some(nl) => index + nl + 1,
                None => bytes.len(),
            };
            continue;
        }
        if sql[index..].starts_with("/*") {
            let end = sql[index + 2..].find("*/");
            index = match end {
                Some(e) => index + 2 + e + 2,
                None => bytes.len(),
            };
            continue;
        }

        let char = bytes[index] as char;
        if char == '$' {
            let re_delim = Regex::new(r"^\$[A-Za-z_0-9]*\$").unwrap();
            if let Some(m) = re_delim.find(&sql[index..]) {
                let token = m.as_str();
                let end = sql[index + token.len()..].find(token);
                index = match end {
                    Some(e) => index + token.len() + e + token.len(),
                    None => bytes.len(),
                };
                continue;
            }
        }
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
        if char == '[' {
            index += 1;
            while index < bytes.len() {
                if index + 1 < bytes.len() && &sql[index..index + 2] == "]]" {
                    index += 2;
                } else if bytes[index] as char == ']' {
                    index += 1;
                    break;
                } else {
                    index += 1;
                }
            }
            continue;
        }
        if char == '(' {
            depth += 1;
            index += 1;
            continue;
        }
        if char == ')' {
            depth = (depth - 1).max(0);
            index += 1;
            continue;
        }

        if let Some(m) = word_re.find(&sql[index..]) {
            if depth == 0 {
                keywords.push(m.as_str().to_uppercase());
            }
            index += m.len();
        } else {
            index += 1;
        }
    }

    if keywords.is_empty() || (keywords[0] != "SELECT" && keywords[0] != "WITH") {
        return false;
    }
    if keywords[0] == "WITH" && !keywords[1..].contains(&"SELECT".to_string()) {
        return false;
    }

    for (i, kw) in keywords.iter().enumerate() {
        if kw == "LIMIT" {
            return false;
        }
        if kw == "FETCH" && i + 1 < keywords.len() && (keywords[i + 1] == "FIRST" || keywords[i + 1] == "NEXT") {
            return false;
        }
    }

    true
}

