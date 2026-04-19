use std::{path::PathBuf, process::ExitCode};

use anyhow::{Context, Result, bail};
use cargo_apple_runner::{Binary, CargoEnv, Platform, bundle, sign, simctl, util};
use tracing::error;
use tracing_subscriber::filter::{EnvFilter, LevelFilter};

fn main() -> ExitCode {
    // Initialize tracing.
    //
    // We should write to stderr, things like `cargo nextest` assumes that
    // test harnesses write output in a certain format to stdout.
    //
    // TODO: Better error messages using `annotate-snippets`? Or at least
    // something similar, I would like to emit proper `help` notes.
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .with_writer(std::io::stderr)
        .init();

    match run() {
        Ok(code) => code,
        Err(err) => {
            error!("{err}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<ExitCode> {
    let mut args = std::env::args_os().skip(1);
    let Some(executable) = args.next() else {
        bail!("must provide binary as first argument");
    };
    let executable = PathBuf::from(executable);

    let cargo_env = CargoEnv::read();

    let binary =
        Binary::parse(&executable).with_context(|| format!("failed parsing {executable:?}"))?;

    let (bundle_path, exe_path, bundle_identifier) = bundle::write_bundle(&binary, &cargo_env)
        .with_context(|| format!("failed writing bundle {executable:?}"))?;

    let entitlements = if binary.platform().is_simulator() {
        // Don't sign with entitlements on simulator, the binary contains the
        // entitlements that will be used in the `__ents_der` section.
        //
        // TODO: Maybe add just `com.apple.security.get-task-allow`?
        // https://github.com/dotnet/macios/blob/bf580528ffb2af02963c70a4e53859cc3b67eb34/msbuild/Xamarin.MacDev.Tasks/Tasks/CompileEntitlements.cs#L564-L580
        None
    } else {
        binary.entitlements_data.as_deref()
    };
    let entitlements_path = if let Some(entitlements) = entitlements {
        let entitlements_path = bundle_path.with_extension("entitlements");
        std::fs::write(&entitlements_path, entitlements).context("failed writing entitlements")?;
        Some(entitlements_path)
    } else {
        None
    };

    // TODO: `DEVELOPMENT_TEAM`
    let identity = std::env::var_os("CODE_SIGN_IDENTITY").unwrap_or_else(|| "-".into());
    let identity = identity
        .to_str()
        .context("CODE_SIGN_IDENTITY was not UTF-8")?;
    sign::codesign(
        &bundle_path,
        &binary,
        identity,
        entitlements_path.as_deref(),
    )
    .with_context(|| format!("failed signing {executable:?}"))?;

    // TODO: Execution policy exception?

    // TODO: Support "Designed for iPad" run mode somehow?
    match binary.platform() {
        Platform::MACOS | Platform::MACCATALYST => {
            // TODO: lsregister & touch stuff

            let status = std::process::Command::new(&exe_path)
                .args(args)
                .status()
                .with_context(|| format!("failed running executable {exe_path:?}"))?;
            Ok(util::status_to_code(status))
        }
        Platform::IOSSIMULATOR
        | Platform::TVOSSIMULATOR
        | Platform::WATCHOSSIMULATOR
        | Platform::VISIONOSSIMULATOR => {
            // TODO: Allow selecting specific device with `DEVICE="..."` env var?
            let (_runtime, device) = simctl::get_device(&binary)?;
            let status = if binary.gui_like {
                let bundle_identifier = bundle_identifier.context("must have bundle identifier")?;
                simctl::install_and_launch(&device.udid, &bundle_path, &bundle_identifier, args)?
            } else {
                simctl::spawn(&device.udid, &bundle_path, &exe_path, args)?
            };
            Ok(util::status_to_code(status))
        }
        Platform::IOS | Platform::TVOS | Platform::WATCHOS | Platform::VISIONOS => {
            bail!("devicectl / ios-deploy is not yet supported");
        }
        platform => {
            bail!("unsupported platform {platform:?}");
        }
    }
}
