use tracing::warn;

pub struct CargoEnv {
    pub pkg_name: String,
    pub pkg_description: String,
    pub pkg_version: String,
}

impl CargoEnv {
    /// Read from current environment.
    pub fn read() -> Self {
        let pkg_name = std::env::var("CARGO_PKG_NAME").unwrap_or_else(|err| {
            warn!(%err, "failed reading CARGO_PKG_NAME");
            "unknown".into()
        });
        let pkg_description = std::env::var("CARGO_PKG_DESCRIPTION").unwrap_or_else(|err| {
            warn!(%err, "failed reading CARGO_PKG_DESCRIPTION");
            "".into()
        });
        let pkg_version = std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|err| {
            warn!(%err, "failed reading CARGO_PKG_VERSION");
            "1.0".into()
        });

        // NOTE: We don't parse the manifest from `CARGO_MANIFEST_PATH`,
        // partly because it's hard to know for sure whether `my_bin-deadbeef`
        // matches a test or a benchmark with the same name.

        Self {
            pkg_name,
            pkg_description,
            pkg_version,
        }
    }
}
