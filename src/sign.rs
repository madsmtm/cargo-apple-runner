//! Docs on code signing:
//! - <https://developer.apple.com/library/archive/technotes/tn2206/_index.html#//apple_ref/doc/uid/DTS40007919>
//! - <https://developer.apple.com/help/account/>
//!
//! Various tech notes:
//! - <https://developer.apple.com/documentation/technotes/tn3125-inside-code-signing-provisioning-profiles>
//! - <https://developer.apple.com/documentation/technotes/tn3126-inside-code-signing-hashes>
//! - <https://developer.apple.com/documentation/technotes/tn3127-inside-code-signing-requirements>
//! - <https://developer.apple.com/documentation/technotes/tn3161-inside-code-signing-certificates>
use std::{path::Path, process::Command};

use anyhow::Result;
use tracing::debug;

use crate::{Binary, Platform, util};

/// Sign the given path.
///
/// NOTE: We don't provide a way to set `--identifier` here, the user should
/// do that with their `CFBundleIdentifier` in their embedded `Info.plist`
/// instead. Similar for `--options`, the user should set `CSFlags` instead.
pub fn codesign(
    path: &Path,
    binary: &Binary,
    signing_identity: &str,
    entitlements_path: Option<&Path>,
) -> Result<()> {
    // if matches!(
    //     binary.version().platform,
    //     Platform::MACOS | Platform::MACCATALYST
    // ) && binary.signed
    //     && binary.entitlements_data.is_none()
    // {
    //     warn!("avoided signing, we'd just have done a dummy resigning");
    //     return Ok(());
    // }

    // TODO: Use `tauri-macos-sign`?
    let mut cmd = Command::new("codesign");

    // Resign.
    cmd.arg("--force");

    // Use given signing identity.
    cmd.arg("--sign");
    cmd.arg(signing_identity);

    if let Some(entitlements_path) = entitlements_path {
        cmd.arg("--entitlements");
        cmd.arg(entitlements_path);

        // Unsure if required, Xcode adds this by default.
        cmd.arg("--generate-entitlement-der");
    }

    // Don't contact Apple's servers.
    if binary.platform() == Platform::MACOS {
        // TODO: Is this the right condition? Do we need to allow users to
        // control this?
        cmd.arg("--timestamp=none");
    }

    // The thing to sign.
    cmd.arg(path);

    debug!("{cmd:?}");

    let stdout = util::command_stdout(cmd)?;
    debug_assert_eq!(stdout, b"");

    Ok(())
}
