use chrono::Local;

/// Build metadata captured at compile time
pub struct BuildMetadata {
    pub app_name: &'static str,
    pub version: &'static str,
    pub commit_id: &'static str,
    pub compile_time: &'static str,
    pub is_dirty: bool,
}

impl BuildMetadata {
    /// Capture build metadata from environment variables set by build.rs
    pub fn capture() -> Self {
        Self {
            app_name: env!("CARGO_PKG_NAME"),
            version: env!("CARGO_PKG_VERSION"),
            commit_id: env!("BUILD_COMMIT_ID"),
            compile_time: env!("BUILD_COMPILE_TIME"),
            is_dirty: env!("BUILD_IS_DIRTY") == "true",
        }
    }

    /// Output startup log in plain text format with fixed positions
    /// Format: "{app_name} v{version} commit={commit_id} compiled={compile_time} started={startup_time}"
    pub fn log_startup(&self) {
        let startup_time = Local::now().format("%Y-%m-%dT%H:%M:%S%:z").to_string();
        let version_str = if self.is_dirty {
            format!("{}-dirty", self.version)
        } else {
            self.version.to_string()
        };

        let short_commit_id = if self.commit_id.len() > 7 {
            &self.commit_id[..7]
        } else {
            self.commit_id
        };

        println!(
            "{} v{} commit={} compiled={} started={}",
            self.app_name, version_str, short_commit_id, self.compile_time, startup_time
        );
    }

    /// Format version string for display in --help output
    /// Format: "{app_name} {version} (commit {commit_id}, compiled {compile_time})"
    pub fn format_version_line(&self) -> String {
        let version_str = if self.is_dirty {
            format!("{}-dirty", self.version)
        } else {
            self.version.to_string()
        };

        let short_commit_id = if self.commit_id.len() > 7 {
            &self.commit_id[..7]
        } else {
            self.commit_id
        };

        format!(
            "{} {} (commit {}, compiled {})",
            self.app_name, version_str, short_commit_id, self.compile_time
        )
    }
}
