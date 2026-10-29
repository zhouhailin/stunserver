//! Build script for stunclient
//! Captures git metadata and build information

use std::process::Command;

fn main() {
    // Get git commit hash
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output();

    let commit_id = match output {
        Ok(out) if out.status.success() => {
            String::from_utf8(out.stdout).unwrap_or_else(|_| "unknown".to_string())
        }
        _ => "unknown".to_string(),
    };
    println!("cargo:rustc-env=GIT_COMMIT_ID={}", commit_id.trim());

    // Get commit timestamp
    let output = Command::new("git")
        .args(["log", "-1", "--format=%cd", "--date=iso-strict"])
        .output();

    let commit_time = match output {
        Ok(out) if out.status.success() => {
            String::from_utf8(out.stdout).unwrap_or_else(|_| "unknown".to_string())
        }
        _ => "unknown".to_string(),
    };
    println!("cargo:rustc-env=GIT_COMMIT_TIME={}", commit_time.trim());

    // Get build timestamp (UTC)
    let build_time = chrono::Utc::now().to_rfc3339();
    println!("cargo:rustc-env=BUILD_TIME={}", build_time);

    // Get version from Cargo.toml
    let version = env!("CARGO_PKG_VERSION");
    println!("cargo:rustc-env=APP_VERSION={}", version);

    // Tell cargo to rerun if git HEAD changes
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/");
}
