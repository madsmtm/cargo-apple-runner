//! An interface to `xcrun simctl`.
//!
//! # About the simulator
//!
//! The iOS/tvOS/watchOS/visionOS simulator uses the host macOS kernel, which
//! enables easier debugging, higher performance etc. Processes are configured
//! such that they use various frameworks in `$IPHONE_SIMULATOR_ROOT`, but are
//! not otherwise isolated, any process running in the simulator still has
//! access to the host filesystem, peripherals, GPU etc.
//!
//! There are two ways of launching binaries on the simulator: spawning a new
//! process or launching a bundled application.
//!
//! Ideally, we'd just always launch applications, but there's a catch: there
//! can only be a single actively launched application at a time, so launching
//! must be serialized.
//!
//! To make `cargo test` faster, we spawn applications instead of launching
//! them when heuristics tell us it's (probably) safe to do so.

use std::{
    cmp::Ordering,
    collections::HashMap,
    ffi::{OsStr, OsString},
    fs,
    os::unix::process::ExitStatusExt,
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
    str::FromStr,
};

use anyhow::{Context, Result, bail};
use object::Architecture;
use serde::{Deserialize, Deserializer, de::Error as _};
use tracing::{error, trace};

use crate::{Binary, OSVersion, Platform, util};

/// Get the temporary directory of the device.
///
/// This works even if the device isn't booted.
pub fn get_temp_dir(udid: &str) -> Result<PathBuf> {
    let mut cmd = Command::new("xcrun");
    cmd.arg("simctl");
    cmd.arg("getenv");
    cmd.arg(udid);
    cmd.arg("TMPDIR");
    let stdout = util::command_stdout(cmd)?;

    let stdout = stdout.strip_suffix(b"\n").unwrap_or(&stdout);
    #[cfg(unix)]
    let path = <std::ffi::OsStr as std::os::unix::ffi::OsStrExt>::from_bytes(stdout);
    #[cfg(not(unix))]
    let path = std::ffi::OsStr::new(std::str::from_utf8(stdout).unwrap());
    Ok(PathBuf::from(path))
}

/// Find an available device with the correct runtime to run on.
///
/// If none exist, return an error that guides the user towards creating a
/// suitable device (doing that automatically is error-prone).
pub fn get_device(binary: &Binary) -> Result<(Runtime, Device)> {
    // Don't use filter options, only the high-level ones (`runtimes`,
    // `devices`, `devicetypes` or `pairs`) are supported on Xcode 9.2.
    //
    // We also don't pass `--no-escape-slashes`, since that isn't supported on
    // all Xcode versions - we'll have to unescape slashes in paths ourselves.
    let mut cmd = Command::new("xcrun");
    cmd.arg("simctl");
    cmd.arg("list");
    cmd.arg("--json");
    let stdout = util::command_stdout(cmd)?;

    let info: SimulatorInfo = serde_json::from_slice(&stdout).context("failed parsing JSON")?;
    let mut runtimes = info.runtimes;
    if runtimes.is_empty() {
        bail!("no simulator runtimes found? Try running `xcrun simctl list runtimes`");
    }

    // Filter available runtimes.
    runtimes
        .retain(|runtime| runtime.availability == Availability::Available && !runtime.is_internal);
    if runtimes.is_empty() {
        bail!(
            "only unavailable simulator runtimes found? Try running `xcrun simctl list runtimes available`"
        );
    }

    // Filter runtimes by platform.
    let expected_platform_name = match binary.platform() {
        Platform::IOSSIMULATOR => "iOS",
        Platform::TVOSSIMULATOR => "tvOS",
        Platform::WATCHOSSIMULATOR => "watchOS",
        Platform::VISIONOSSIMULATOR => "xrOS",
        _ => unreachable!("unknown simulator platform"),
    };
    runtimes.retain(|runtime| runtime.is_platform(expected_platform_name));
    if runtimes.is_empty() {
        bail!(
            "no simulator runtimes for `{expected_platform_name}` found? Try running `xcrun simctl list runtimes available {expected_platform_name}`"
        );
    }

    // Filter runtimes by architecture.
    let expected_arch = String::from(match binary.arch {
        Architecture::Aarch64 => "arm64",
        Architecture::X86_64 => "x86_64",
        Architecture::I386 => "i386", // probably
        arch => {
            error!(?arch, "unknown simulator architecture");
            ""
        }
    });
    runtimes.retain(|runtime| {
        runtime
            .supported_architectures
            .as_deref()
            .map(|archs| archs.contains(&expected_arch))
            .unwrap_or(true)
    });
    if runtimes.is_empty() {
        bail!(
            "no simulator runtimes for the architecture {expected_arch} found? Ensure that you're running Cargo with the `--target` flag corresponding to your host architecture"
        );
    }

    // Filter runtimes by OS version.
    let minos = binary.minos();
    runtimes.retain(|runtime| match OSVersion::from_str(&runtime.version) {
        Ok(runtime_version) => minos <= runtime_version,
        Err(err) => {
            error!(?runtime.version, "failed parsing: {err}");
            true
        }
    });
    if runtimes.is_empty() {
        bail!(
            "the binary was compiled for {expected_platform_name} {minos}, but no simulator runtimes support that high OS version. Check `xcrun simctl list runtimes available`",
        );
    }

    trace!(?runtimes, "found runtimes");

    // Now that we have a list of suitable runtimes, grab their devices.
    let mut devices = runtimes
        .iter()
        .flat_map(|runtime| {
            info.devices
                .get(&runtime.identifier)
                .or_else(|| info.devices.get(&runtime.name))
                .map(|devices| &**devices)
                .unwrap_or_else(|| {
                    error!("could not find devices for runtime {}", runtime.identifier);
                    &[]
                })
                .iter()
                .map(move |device| (runtime, device))
        })
        .collect::<Vec<_>>();
    if devices.is_empty() {
        bail!(
            "no simulator devices found? Run `xcrun simctl list devices {expected_platform_name}` to debug, and consider running `xcrun simctl create` to create a device"
        );
    }

    // Filter available devices.
    devices.retain(|(_runtime, device)| device.availability == Availability::Available);
    if devices.is_empty() {
        bail!(
            "only unavailable simulator devices found? Run `xcrun simctl list devices available {expected_platform_name}` to debug, and consider running `xcrun simctl create` to create a device"
        );
    }

    // Filter booted devices.
    devices.retain(|(_runtime, device)| device.state == DeviceState::Booted);
    if devices.is_empty() {
        bail!(
            "no booted simulator devices found? Run `xcrun simctl list devices booted {expected_platform_name}` to debug, and consider running `xcrun simctl boot` to boot the device"
        );
    }

    // Sort devices.
    devices.sort_by(|(runtime_a, device_a), (runtime_b, device_b)| {
        // Prefer recently used devices.
        match (&device_a.last_booted_at, &device_b.last_booted_at) {
            // Rely on newer dates sorting higher here (dates are ISO 8601).
            (Some(a), Some(b)) => a.cmp(b),
            (Some(_), None) => Ordering::Greater,
            (None, Some(_)) => Ordering::Less,
            (None, None) => Ordering::Equal,
        }
        .then_with(|| {
            // Otherwise prefer devices with newer runtimes.
            match (
                OSVersion::from_str(&runtime_a.version),
                OSVersion::from_str(&runtime_b.version),
            ) {
                (Ok(a), Ok(b)) => a.cmp(&b),
                _ => Ordering::Equal,
            }
        })
        .then_with(|| {
            // Lastly, sort by name for stability.
            device_a.name.cmp(&device_b.name)
        })
    });

    trace!(?devices, "found devices");

    // And grab the most relevant device.
    let (runtime, device) = devices.first().expect("checked before");
    Ok(((*runtime).clone(), (*device).clone()))
}

pub fn spawn<A: AsRef<OsStr>>(
    udid: &str,
    bundle_path: &Path,
    exe_path: &Path,
    args: impl Iterator<Item = A>,
) -> Result<ExitStatus> {
    let temp_dir = get_temp_dir(udid)?;

    // TODO: Place binary in temporary location on device.
    // DEVICE_EXECUTABLE=$(mktemp $DEVICE_TMPDIR/$(basename $EXECUTABLE).XXXXXX)
    // cp -c $EXECUTABLE $DEVICE_EXECUTABLE
    //
    // This is done to make the executable readable such that accessing
    // `std::env::current_exe()` still works.

    // TODO: Support bundled apps here, by instead of copying, we write the
    // bundled app directly to a temporary location on the device.
    debug_assert_eq!(bundle_path, exe_path);

    // Spawn the executable with the arguments.
    let mut cmd = Command::new("xcrun");
    cmd.arg("simctl");
    cmd.arg("spawn");
    cmd.arg(udid);
    cmd.arg(exe_path);
    cmd.args(args);
    cmd.envs(forwarded_env_vars());
    // Set `CARGO_TARGET_TMPDIR` to `TMPDIR`. See also <https://github.com/rust-lang/cargo/issues/16427>.
    cmd.env("SIMCTL_CHILD_CARGO_TARGET_TMPDIR", temp_dir);

    let status = cmd
        .status()
        .with_context(|| format!("failed spawning executable {exe_path:?}"))?;

    Ok(status)
}

pub fn install_and_launch<A: AsRef<OsStr>>(
    udid: &str,
    bundle_path: &Path,
    bundle_identifier: &str,
    args: impl Iterator<Item = A>,
) -> Result<ExitStatus> {
    let temp_dir = get_temp_dir(udid)?;

    // Only a single application can be launched at a time, so we add a shared
    // lock on the device (in the temporary directory, so no need to clean up
    // the file afterwards), and synchronize with other `cargo-apple-runner`
    // processes to wait launching until the other runners are done.
    //
    // This makes test executors like `cargo nextest` that spawn multiple
    // processes at the same time work.
    let lock_file = fs::File::create(temp_dir.join("cargo-apple-runner.lock"))
        .context("failed creating lock file in simulator")?;
    lock_file.lock()?;

    // Install the application.
    // TODO: Ensure that what's being installed is unique / won't conflict
    // with other processes, and move it above the lock somehow?
    let mut cmd = Command::new("xcrun");
    cmd.arg("simctl");
    cmd.arg("install");
    cmd.arg(udid);
    cmd.arg(bundle_path);

    // Launch the application.
    let mut cmd = Command::new("xcrun");
    cmd.arg("simctl");
    cmd.arg("launch");
    cmd.arg("--console");
    cmd.arg(udid);
    cmd.arg(bundle_identifier);
    cmd.args(args);
    cmd.envs(forwarded_env_vars());
    // Set `CARGO_TARGET_TMPDIR` to `TMPDIR`. See also <https://github.com/rust-lang/cargo/issues/16427>.
    cmd.env("SIMCTL_CHILD_CARGO_TARGET_TMPDIR", temp_dir);

    lock_file.unlock()?;
    Ok(ExitStatus::from_raw(0))
}

/// Environment variables to set for `xcrun` invocations.
///
/// This copies:
/// - All `CARGO_PKG_*` env vars.
/// - The `CARGO_CRATE_NAME`, `CARGO_BIN_NAME` and `CARGO_PRIMARY_PACKAGE` env
///   vars.
///
/// We deliberately don't copy CWD-relative vars like `CARGO_MANIFEST_DIR`, as
/// that won't work reliably if the code is located in a protected directory
/// such as `~/Documents` or `~/Desktop`:
/// <https://support.apple.com/en-US/guide/security/secddd1d86a6/web>.
///
/// TODO: Somehow discourage `Path::new(env!("CARGO_MANIFEST_DIR"))` too?
fn forwarded_env_vars() -> impl IntoIterator<Item = (OsString, OsString)> {
    std::env::vars_os()
        .filter(|(key, _)| {
            let Some(key) = key.to_str() else {
                return false;
            };

            key.starts_with("CARGO_PKG_")
                || matches!(
                    key,
                    "CARGO_CRATE_NAME" | "CARGO_BIN_NAME" | "CARGO_PRIMARY_PACKAGE"
                )
        })
        .map(|(key, value)| {
            let mut new_key = OsString::from("SIMCTL_CHILD_");
            new_key.push(key);
            (new_key, value)
        })
}

#[derive(Deserialize)]
struct SimulatorInfo {
    #[serde(default)]
    runtimes: Vec<Runtime>,
    /// Key is either runtime identifier or runtime name.
    #[serde(default)]
    devices: HashMap<String, Vec<Device>>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Runtime {
    #[allow(dead_code)]
    buildversion: String,
    name: String,
    identifier: String,
    version: String,
    #[serde(deserialize_with = "deserialize_availability", flatten)]
    availability: Availability,

    // The below fields are not available on Xcode 9.2.
    #[serde(rename = "isInternal")]
    #[serde(default)] // Default to `false`
    is_internal: bool,
    platform: Option<String>,
    #[serde(rename = "supportedArchitectures")]
    supported_architectures: Option<Vec<String>>,
}

impl Runtime {
    fn is_platform(&self, platform_name: &str) -> bool {
        if let Some(p) = &self.platform {
            p == platform_name
        } else {
            self.identifier.contains(&platform_name)
        }
    }
}

#[derive(Deserialize, PartialEq, Eq, Hash, Clone, Debug)]
pub struct Device {
    name: String,
    /// Unique device ID (UUID).
    pub udid: String,
    state: DeviceState,
    #[serde(deserialize_with = "deserialize_availability", flatten)]
    availability: Availability,

    // The below fields are not available on Xcode 9.2.
    #[serde(rename = "lastBootedAt")]
    last_booted_at: Option<String>,
}

// The possible states are not documented, but some can be found in:
// https://github.com/facebook/idb/blob/cf3dc8643de10efd57dd10617032455888b8b6f9/FBControlCore/Management/FBiOSTargetConstants.m#L12-L20
#[derive(Deserialize, PartialEq, Eq, Hash, Clone, Debug)]
enum DeviceState {
    Creating,
    Booting,
    Booted,
    #[serde(rename = "Shutting Down")]
    ShuttingDown,
    Shutdown,
    #[serde(other)]
    Unknown,
}

#[derive(PartialEq, Eq, Hash, Clone, Debug)]
enum Availability {
    Available,
    Unavailable(String),
}

/// Deserialize availability information.
///
/// On Xcode 9.2, `availability` is set to `"(available)"` or an error
/// value like `" (unavailable, xyz)"`.
///
/// On newer Xcode, `isAvailable` is present, and `availabilityError` is
/// set if the runtime is not available.
fn deserialize_availability<'de, D: Deserializer<'de>>(d: D) -> Result<Availability, D::Error> {
    #[derive(Deserialize)]
    struct Raw {
        availability: Option<String>,
        #[serde(rename = "isAvailable")]
        is_available: Option<bool>,
        #[serde(rename = "availabilityError")]
        availability_error: Option<String>,
    }

    let raw = Raw::deserialize(d)?;

    match (raw.availability, raw.is_available, raw.availability_error) {
        (Some(s), _, _) if s == "(available)" => Ok(Availability::Available),
        (Some(message), _, _) => Ok(Availability::Unavailable(message)),
        (_, Some(true), _) => Ok(Availability::Available),
        (_, Some(false), Some(message)) => Ok(Availability::Unavailable(message)),
        (_, Some(false), None) => Ok(Availability::Unavailable(String::new())),
        _ => Err(D::Error::custom("missing availability field")),
    }
}

#[cfg(test)]
mod tests;
