use std::fmt;

use chrono::{DateTime, Local};

#[derive(Debug)]
pub struct VersionInfo {
    /// Name of the system component
    pub name: &'static str,
    /// Version string (e.g., "1.0.0")
    pub version: &'static str,
    /// Git branch, if available
    pub branch: Option<&'static str>,
    /// Git commit hash, if available
    pub commit_hash: Option<&'static str>,
    /// Compiler used for building
    pub compiler: &'static str,
    /// Compile timestamp
    pub compile_time: &'static str,
}

impl fmt::Display for VersionInfo {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        let dt = DateTime::parse_from_rfc2822(self.compile_time).unwrap_or_default();

        let local_dt = dt.with_timezone(&Local);

        let formatted_compile_time = local_dt.format("%Y-%m-%d %H:%M:%S").to_string();

        write!(
            f,
            "Module Name:   {}
Version:       {}
Branch:        {}
Commit Hash:   {}
Compiler:      {}
Compile Time:  {}",
            self.name,
            self.version,
            self.branch.unwrap_or("None"),
            self.commit_hash.unwrap_or("None"),
            self.compiler,
            formatted_compile_time
        )
    }
}
