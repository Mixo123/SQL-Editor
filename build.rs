//! Build script to dynamically provide APP_VERSION from GitHub Actions tag or Cargo.toml.

fn main() {
    println!("cargo:rerun-if-env-changed=GITHUB_REF_NAME");

    let version = if let Ok(ref_name) = std::env::var("GITHUB_REF_NAME") {
        let clean = ref_name.trim().trim_start_matches(|c| c == 'v' || c == 'V');
        if !clean.is_empty() && clean.chars().next().map_or(false, |c| c.is_ascii_digit()) {
            clean.to_string()
        } else {
            std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.5.0".to_string())
        }
    } else {
        std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.5.0".to_string())
    };

    println!("cargo:rustc-env=APP_VERSION={}", version);
}

