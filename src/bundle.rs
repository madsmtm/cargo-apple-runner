//! <https://developer.apple.com/go/?id=bundle-structure>

use std::{fs, path::PathBuf, process::Command};

use anyhow::{Context, Result};
use tracing::{debug, error, warn};

use crate::{Binary, CargoEnv, Platform};

pub fn write_bundle(
    binary: &Binary,
    cargo_env: &CargoEnv,
) -> Result<(PathBuf, PathBuf, Option<String>)> {
    // As an optimization, avoid bundling the binary if we don't need to.
    //
    // TODO: Which places in UIKit assume a bundled binary? And how is it
    // detected? Is it enough to have an `__info_plist`, or do we need the
    // binary to actually be bundled? Are there places in AppKit that also
    // work differently?
    if !binary.gui_like {
        return Ok((binary.path.to_path_buf(), binary.path.to_path_buf(), None));
    }

    let bundle_path = binary.path.with_added_extension("app");

    // Remove previous bundle if it exists.
    //
    // TODO: Could we do more fine-grained updates instead, to be faster?
    // Or is this the wrong level to do this at (and maybe we should instead
    // do it if the modification time of the binary hasn't changed compared
    // to when it was copied to the bundle last?)
    if bundle_path.exists() {
        fs::remove_dir_all(&bundle_path).context("failed to remove previous app bundle")?;
    }

    // Create dirs.
    let exe_dir = if binary.platform() == Platform::MACOS {
        bundle_path.join("Contents").join("MacOS")
    } else {
        bundle_path.clone()
    };
    debug!("mkdir -p {exe_dir:?}");
    fs::create_dir_all(&exe_dir).context("failed creating bundle directories")?;

    // Copy binary.
    let exe_path = exe_dir.join(binary.path.file_name().expect("binary cannot be root"));
    debug!("cp -c {exe_dir:?} {exe_path:?}");
    fs::copy(&binary.path, &exe_path).context("failed copying executable")?;

    // Create Info.plist.
    //
    // TODO: How should we handle this? Do we just not bundle if this is
    // present? Or do we extract it? Which takes preference, `Info.plist` or
    // the embedded plist?
    //
    // Note that `Info.plist` takes preference over the embedded plist, so if
    // we add fields automatically, we don't need to remove the embedded one!
    let info_plist = binary
        .info_plist_data
        .clone()
        .unwrap_or_else(|| default_info_plist(binary, cargo_env).into_bytes());
    let info_plist_path = if binary.platform() == Platform::MACOS {
        bundle_path.join("Contents").join("Info.plist")
    } else {
        bundle_path.join("Info.plist")
    };
    debug!("createInfoPlist {info_plist_path:?}");
    fs::write(info_plist_path, &info_plist).context("failed writing Info.plist")?;
    let plist: plist::Value = plist::from_bytes(&info_plist).context("invalid Info.plist")?;
    let bundle_identifier = plist
        .as_dictionary()
        .context("invalid")?
        .get("CFBundleIdentifier")
        .and_then(|v| v.as_string())
        .map(|s| s.to_string());

    // TODO: Write assets etc. from Manganis

    // Remove extra attributes, which could cause codesign to fail
    // https://developer.apple.com/library/archive/qa/qa1940/_index.html
    //
    // Errors here aren't fatal.
    debug!("xattr -crs {bundle_path:?}");
    match Command::new("xattr").arg("-crs").arg(&bundle_path).status() {
        Ok(status) => {
            if !status.success() {
                warn!(?status, "failed removing excess attributes");
            }
        }
        Err(err) => {
            // Possibly because the `xattr` command doesn't exist?
            warn!(%err, "failed removing excess attributes");
        }
    }

    Ok((bundle_path, exe_path, bundle_identifier))
}

fn default_info_plist(binary: &Binary, cargo_env: &CargoEnv) -> String {
    let name = binary
        .path
        .file_name()
        .unwrap()
        .to_str()
        .expect("non-UTF8 in file name");
    let identifier = format!("{}.{name}", cargo_env.pkg_name);
    let display_name = binary
        .name
        .as_deref()
        .map(|bytes| String::from_utf8_lossy(bytes))
        .unwrap_or_else(|| name.into());

    // We could make keys in here platform or device-dependent if need be:
    // https://developer.apple.com/documentation/bundleresources/managing-your-app-s-information-property-list#Add-platform-and-device-specific-properties
    if binary.platform() == Platform::MACOS {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple Computer//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleName</key>
  <string>{name}</string>
  <key>CFBundleDisplayName</key>
  <string>{display_name}</string>
  <key>CFBundleIdentifier</key>
  <string>{identifier}</string>
  <key>CFBundleVersion</key>
  <string>{version}</string>
  <key>CFBundleShortVersionString</key>
  <string>{short_version_string}</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleExecutable</key>
  <string>{name}</string>
  <key>LSMinimumSystemVersion</key>
  <string>{minimum_system_version}</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
</dict>
</plist>
"#,
            // TODO: Increasing date like cargo-bundle does?
            version = "1.0",
            short_version_string = cargo_env.pkg_version,
            minimum_system_version = binary.minos(),
        )
    } else {
        // https://developer.apple.com/library/archive/documentation/General/Reference/InfoPlistKeyReference/Articles/iPhoneOSKeys.html#//apple_ref/doc/uid/TP40009252-SW11
        let mut family_string = String::new();
        for family in device_families(binary.platform()) {
            family_string.push_str(&format!("    <integer>{family}</integer>\n"));
        }

        // TODO: Does watchOS need `MinimumOSVersion~ipad`? And `WKApplication`?
        // Note: LSRequiresIPhoneOS seems to be set for all non-macOS plists?
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple Computer//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleName</key>
  <string>{name}</string>
  <key>CFBundleDisplayName</key>
  <string>{display_name}</string>
  <key>CFBundleIdentifier</key>
  <string>{identifier}</string>
  <key>CFBundleVersion</key>
  <string>{version}</string>
  <key>CFBundleShortVersionString</key>
  <string>{short_version_string}</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleExecutable</key>
  <string>{name}</string>
  <key>MinimumOSVersion</key>
  <string>{minimum_os_version}</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>LSRequiresIPhoneOS</key>
  <true/>
  <key>CFBundleSupportedPlatforms</key>
  <array>
    <string>{supported_platform}</string>
  </array>
  <key>UIDeviceFamily</key>
  <array>
{family_string}  </array>
</dict>
</plist>
"#,
            // TODO: Increasing date like cargo-bundle does?
            version = "1.0",
            short_version_string = cargo_env.pkg_version,
            minimum_os_version = binary.minos(),
            supported_platform = sdk_name(binary.platform()),
        )
    }
}

/// The SDK / platform name for a give platform.
fn sdk_name(platform: Platform) -> &'static str {
    match platform {
        Platform::MACOS | Platform::MACCATALYST => "MacOSX",
        Platform::IOS => "iPhoneOS",
        Platform::IOSSIMULATOR => "iPhoneSimulator",
        Platform::TVOS => "AppleTVOS",
        Platform::TVOSSIMULATOR => "AppleTVSimulator",
        Platform::WATCHOS => "WatchOS",
        Platform::WATCHOSSIMULATOR => "WatchSimulator",
        Platform::VISIONOS => "XROS",
        Platform::VISIONOSSIMULATOR => "XRSimulator",
        Platform::DRIVERKIT => "DriverKit",
        platform => {
            error!(?platform, "unknown platform");
            "Unknown"
        }
    }
}

fn device_families(platform: Platform) -> &'static [u32] {
    match platform {
        Platform::IOS | Platform::IOSSIMULATOR => &[1, 2],
        Platform::TVOS | Platform::TVOSSIMULATOR => &[3],
        Platform::WATCHOS | Platform::WATCHOSSIMULATOR => &[4],
        Platform::VISIONOS | Platform::VISIONOSSIMULATOR => &[7],
        _ => &[],
    }
}
