//! GitHub release update checker and platform asset detection.

use serde::Deserialize;
use std::sync::mpsc::{channel, Receiver};
use std::thread;

use crate::version::{GITHUB_REPO, VERSION};

#[derive(Debug, Clone, Deserialize)]
pub struct ReleaseAsset {
    pub name: String,
    pub size: u64,
    pub browser_download_url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GitHubRelease {
    pub tag_name: String,
    pub name: Option<String>,
    pub body: Option<String>,
    pub html_url: String,
    pub assets: Vec<ReleaseAsset>,
}

#[derive(Debug, Clone)]
pub struct UpdateCheckResult {
    pub success: bool,
    pub update_available: bool,
    pub current_version: String,
    pub latest_version: String,
    pub release_name: String,
    pub release_notes: String,
    pub html_url: String,
    pub recommended_assets: Vec<PlatformAsset>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PlatformAsset {
    pub label: String,
    pub name: String,
    pub size_formatted: String,
    pub download_url: String,
    pub recommended: bool,
}

pub fn parse_version(v: &str) -> (u32, u32, u32) {
    let clean = v.trim().trim_start_matches(|c| c == 'v' || c == 'V');
    let re = regex::Regex::new(r"\d+").unwrap();
    let parts: Vec<u32> = re
        .find_iter(clean)
        .filter_map(|m| m.as_str().parse().ok())
        .collect();

    let p0 = parts.get(0).copied().unwrap_or(0);
    let p1 = parts.get(1).copied().unwrap_or(0);
    let p2 = parts.get(2).copied().unwrap_or(0);
    (p0, p1, p2)
}

pub fn format_size(size_bytes: u64) -> String {
    if size_bytes == 0 {
        return String::new();
    }
    if size_bytes < 1024 * 1024 {
        format!("{:.1} KB", size_bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", size_bytes as f64 / (1024.0 * 1024.0))
    }
}

pub fn get_platform_assets(assets: &[ReleaseAsset]) -> Vec<PlatformAsset> {
    let mut results = Vec::new();

    let rules: &[(&str, &str, bool)] = if cfg!(target_os = "macos") {
        &[
            (".dmg", "Obraz dysku macOS (.dmg) [Zalecany]", true),
            (".pkg", "Pakiet instalatora macOS (.pkg)", false),
            (".zip", "Archiwum ZIP macOS (.zip)", false),
        ]
    } else if cfg!(target_os = "windows") {
        &[
            (".msi", "Instalator Windows (.msi) [Zalecany]", true),
            ("-portable.zip", "Wersja przenośna Windows (.zip)", false),
            (".zip", "Archiwum ZIP Windows (.zip)", false),
            (".exe", "Plik wykonywalny Windows (.exe)", false),
        ]
    } else {
        &[
            (".appimage", "Pakiet uniwersalny Linux (.AppImage) [Zalecany]", true),
            (".deb", "Pakiet Debian / Ubuntu (.deb)", false),
            (".rpm", "Pakiet Fedora / RHEL (.rpm)", false),
            (".tar.gz", "Archiwum Linux (.tar.gz)", false),
        ]
    };

    let ignore = ["checksum", "sha256", "md5", ".txt", ".asc", ".sig"];

    for &(ext, label, is_rec) in rules {
        for a in assets {
            let low = a.name.to_lowercase();
            if ignore.iter().any(|&p| low.contains(p)) {
                continue;
            }
            if low.ends_with(ext) && !results.iter().any(|r: &PlatformAsset| r.name == a.name) {
                results.push(PlatformAsset {
                    label: label.to_string(),
                    name: a.name.clone(),
                    size_formatted: format_size(a.size),
                    download_url: a.browser_download_url.clone(),
                    recommended: is_rec,
                });
            }
        }
    }

    results
}

pub fn check_for_updates() -> UpdateCheckResult {
    let url = format!("https://api.github.com/repos/{}/releases/latest", GITHUB_REPO);
    let resp = ureq::get(&url)
        .set("User-Agent", "SQL-Editor-Rust-App")
        .set("Accept", "application/vnd.github+json")
        .timeout(std::time::Duration::from_secs(10))
        .call();

    let resp = match resp {
        Ok(r) => r,
        Err(e) => {
            return UpdateCheckResult {
                success: false,
                update_available: false,
                current_version: VERSION.to_string(),
                latest_version: String::new(),
                release_name: String::new(),
                release_notes: String::new(),
                html_url: format!("https://github.com/{}/releases", GITHUB_REPO),
                recommended_assets: Vec::new(),
                error: Some(e.to_string()),
            };
        }
    };

    let release: GitHubRelease = match resp.into_json() {
        Ok(rel) => rel,
        Err(e) => {
            return UpdateCheckResult {
                success: false,
                update_available: false,
                current_version: VERSION.to_string(),
                latest_version: String::new(),
                release_name: String::new(),
                release_notes: String::new(),
                html_url: format!("https://github.com/{}/releases", GITHUB_REPO),
                recommended_assets: Vec::new(),
                error: Some(e.to_string()),
            };
        }
    };

    let curr_tuple = parse_version(VERSION);
    let latest_tuple = parse_version(&release.tag_name);
    let update_available = latest_tuple > curr_tuple;
    let rec_assets = get_platform_assets(&release.assets);

    UpdateCheckResult {
        success: true,
        update_available,
        current_version: VERSION.to_string(),
        latest_version: release.tag_name.trim_start_matches('v').to_string(),
        release_name: release.name.unwrap_or_else(|| release.tag_name.clone()),
        release_notes: release.body.unwrap_or_default(),
        html_url: release.html_url,
        recommended_assets: rec_assets,
        error: None,
    }
}

pub fn check_for_updates_async() -> Receiver<UpdateCheckResult> {
    let (tx, rx) = channel();
    thread::spawn(move || {
        let res = check_for_updates();
        let _ = tx.send(res);
    });
    rx
}
