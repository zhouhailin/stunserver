use chrono::Local;
use std::process::Command;

fn main() {
    // Capture git commit ID
    let commit_id = Command::new("git")
        .args(&["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "unknown".to_string());

    // Check for dirty working tree
    let is_dirty = Command::new("git")
        .args(&["status", "--porcelain"])
        .output()
        .map(|o| !o.stdout.is_empty())
        .unwrap_or(false);

    // Capture compilation timestamp
    let compile_time = Local::now().format("%Y-%m-%dT%H:%M:%S%:z").to_string();

    // Pass to Rust code via environment variables
    println!("cargo:rustc-env=BUILD_COMMIT_ID={}", commit_id);
    println!("cargo:rustc-env=BUILD_COMPILE_TIME={}", compile_time);
    println!("cargo:rustc-env=BUILD_IS_DIRTY={}", is_dirty);

    // Re-run build script if .git/HEAD changes
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/index");
}
