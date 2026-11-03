//! GitHub release update checker, platform asset detection, and in-app downloader.

use serde::Deserialize;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

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
    pub size_bytes: u64,
    pub size_formatted: String,
    pub download_url: String,
    pub recommended: bool,
}

#[derive(Debug, Clone)]
pub struct DownloadProgress {
    pub asset_name: String,
    pub target_path: PathBuf,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub speed_bytes_per_sec: f64,
    pub finished: bool,
    pub error: Option<String>,
}

#[derive(Clone)]
pub struct ActiveDownload {
    pub progress: Arc<Mutex<DownloadProgress>>,
    pub cancel_flag: Arc<AtomicBool>,
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
        return "0 B".to_string();
    }
    if size_bytes < 1024 * 1024 {
        format!("{:.1} KB", size_bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", size_bytes as f64 / (1024.0 * 1024.0))
    }
}

pub fn format_speed(bytes_per_sec: f64) -> String {
    if bytes_per_sec < 1024.0 {
        format!("{:.0} B/s", bytes_per_sec.max(0.0))
    } else if bytes_per_sec < 1024.0 * 1024.0 {
        format!("{:.1} KB/s", bytes_per_sec / 1024.0)
    } else {
        format!("{:.1} MB/s", bytes_per_sec / (1024.0 * 1024.0))
    }
}

pub fn get_download_dir() -> PathBuf {
    if let Some(user_dirs) = directories::UserDirs::new() {
        if let Some(dl) = user_dirs.download_dir() {
            if dl.exists() {
                return dl.to_path_buf();
            }
        }
    }
    std::env::temp_dir()
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
                    size_bytes: a.size,
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

pub fn start_download(
    url: &str,
    asset_name: &str,
    known_size: Option<u64>,
) -> ActiveDownload {
    let target_dir = get_download_dir();
    let target_path = target_dir.join(asset_name);
    start_download_to(url, target_path, asset_name, known_size)
}

pub fn start_download_to(
    url: &str,
    target_path: PathBuf,
    asset_name: &str,
    known_size: Option<u64>,
) -> ActiveDownload {
    let progress = Arc::new(Mutex::new(DownloadProgress {
        asset_name: asset_name.to_string(),
        target_path: target_path.clone(),
        downloaded_bytes: 0,
        total_bytes: known_size.filter(|&s| s > 0),
        speed_bytes_per_sec: 0.0,
        finished: false,
        error: None,
    }));

    let cancel_flag = Arc::new(AtomicBool::new(false));

    let p_clone = Arc::clone(&progress);
    let c_clone = Arc::clone(&cancel_flag);
    let url = url.to_string();

    thread::spawn(move || {
        download_worker(&url, &target_path, p_clone, c_clone);
    });

    ActiveDownload {
        progress,
        cancel_flag,
    }
}

pub fn download_worker(
    url: &str,
    target_path: &Path,
    progress: Arc<Mutex<DownloadProgress>>,
    cancel: Arc<AtomicBool>,
) {
    let resp = match ureq::get(url)
        .set("User-Agent", "SQL-Editor-Rust-App")
        .timeout(std::time::Duration::from_secs(60))
        .call()
    {
        Ok(r) => r,
        Err(e) => {
            let mut p = progress.lock().unwrap();
            p.error = Some(format!("Błąd połączenia: {}", e));
            return;
        }
    };

    let content_len = resp.header("content-length").and_then(|s| s.parse::<u64>().ok());
    download_from_reader(resp.into_reader(), target_path, content_len, progress, cancel);
}

pub fn download_from_reader<R: Read>(
    mut reader: R,
    target_path: &Path,
    content_len: Option<u64>,
    progress: Arc<Mutex<DownloadProgress>>,
    cancel: Arc<AtomicBool>,
) {
    if content_len.is_some() {
        let mut p = progress.lock().unwrap();
        p.total_bytes = content_len;
    }

    let part_path = target_path.with_extension(format!(
        "{}.part",
        target_path.extension().and_then(|e| e.to_str()).unwrap_or("tmp")
    ));

    if let Some(parent) = part_path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let mut file = match fs::File::create(&part_path) {
        Ok(f) => f,
        Err(e) => {
            let mut p = progress.lock().unwrap();
            p.error = Some(format!("Nie można utworzyć pliku: {}", e));
            return;
        }
    };

    let mut buf = [0u8; 64 * 1024];
    let mut downloaded: u64 = 0;
    let start_time = Instant::now();
    let mut last_sample_time = Instant::now();
    let mut last_sample_bytes = 0u64;

    loop {
        if cancel.load(Ordering::Relaxed) {
            drop(file);
            let _ = fs::remove_file(&part_path);
            return;
        }

        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                if let Err(e) = file.write_all(&buf[..n]) {
                    drop(file);
                    let _ = fs::remove_file(&part_path);
                    let mut p = progress.lock().unwrap();
                    p.error = Some(format!("Błąd zapisu danych: {}", e));
                    return;
                }
                downloaded += n as u64;

                let now = Instant::now();
                let sample_elapsed = now.duration_since(last_sample_time).as_secs_f64();
                if sample_elapsed >= 0.25 {
                    let bytes_diff = downloaded.saturating_sub(last_sample_bytes);
                    let speed = (bytes_diff as f64) / sample_elapsed;
                    last_sample_time = now;
                    last_sample_bytes = downloaded;

                    let mut p = progress.lock().unwrap();
                    p.downloaded_bytes = downloaded;
                    p.speed_bytes_per_sec = speed;
                }
            }
            Err(e) => {
                drop(file);
                let _ = fs::remove_file(&part_path);
                let mut p = progress.lock().unwrap();
                p.error = Some(format!("Błąd transmisji danych: {}", e));
                return;
            }
        }
    }

    if let Err(e) = file.flush() {
        drop(file);
        let _ = fs::remove_file(&part_path);
        let mut p = progress.lock().unwrap();
        p.error = Some(format!("Błąd zapisu pliku: {}", e));
        return;
    }
    drop(file);

    if target_path.exists() {
        let _ = fs::remove_file(target_path);
    }

    if let Err(e) = fs::rename(&part_path, target_path) {
        let mut p = progress.lock().unwrap();
        p.error = Some(format!("Błąd finalizowania pliku: {}", e));
        return;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if target_path.extension().map_or(false, |ext| ext.eq_ignore_ascii_case("appimage")) {
            if let Ok(metadata) = fs::metadata(target_path) {
                let mut perms = metadata.permissions();
                perms.set_mode(0o755);
                let _ = fs::set_permissions(target_path, perms);
            }
        }
    }

    let mut p = progress.lock().unwrap();
    p.downloaded_bytes = downloaded;
    if p.total_bytes.is_none() {
        p.total_bytes = Some(downloaded);
    }
    let total_elapsed = start_time.elapsed().as_secs_f64();
    if total_elapsed > 0.0 {
        p.speed_bytes_per_sec = (downloaded as f64) / total_elapsed;
    }
    p.finished = true;
}
