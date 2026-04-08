use std::process::{Command, ExitStatus};
use std::{fmt::Write, process::ExitCode};

use anyhow::{Context, Result, bail};

pub(crate) fn command_stdout(mut cmd: Command) -> Result<Vec<u8>> {
    let output = cmd
        .output()
        .with_context(|| format!("failed spawning {cmd:?}"))?;

    if !output.status.success() {
        let mut message = String::new();
        if !output.stdout.is_empty() {
            write!(
                &mut message,
                "\nstdout: {}",
                String::from_utf8_lossy(&output.stdout)
            )
            .unwrap();
        }
        if !output.stderr.is_empty() {
            write!(
                &mut message,
                "\nstderr: {}",
                String::from_utf8_lossy(&output.stderr)
            )
            .unwrap();
        }
        bail!("failed running `{cmd:?}`: {}{message}", output.status);
    }

    Ok(output.stdout)
}

pub fn status_to_code(status: ExitStatus) -> ExitCode {
    if let Some(code) = status.code() {
        if let Ok(code) = u8::try_from(code) {
            ExitCode::from(code)
        } else {
            ExitCode::FAILURE
        }
    } else {
        ExitCode::FAILURE
    }
}
