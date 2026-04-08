use std::io;

use sysinfo::{Pid, ProcessRefreshKind, RefreshKind, System, UpdateKind};

/// Check whether other `cargo-apple-runner` processes are currently running.
///
/// This is used as a hint, where if it returns `false`, after we've checked
/// that a file lock exists, we can reasonably assume that something has gone
/// wrong somewhere, and that said file lock won't ever be released.
pub fn other_cargo_apple_runner_processes() -> io::Result<bool> {
    let executable = std::env::current_exe()?;
    let file_name = executable
        .file_name()
        .ok_or_else(|| io::Error::other("executable had invalid file name"))?;

    // Find cargo-apple-runner processes.
    let system = System::new_with_specifics(
        RefreshKind::nothing()
            .with_processes(ProcessRefreshKind::nothing().with_user(UpdateKind::OnlyIfNotSet)),
    );
    let processes = system
        .processes()
        .into_iter()
        .filter(|(_, process)| process.name() == file_name || process.exe() == Some(&executable));

    // Of these, find the current process' User ID.
    let current_pid = Pid::from_u32(std::process::id());
    let this_process_uid = processes
        .clone()
        .find(|(pid, _)| current_pid == **pid)
        .and_then(|(_, process)| process.user_id());

    // Further filter processes by UID, to have slightly bit better support
    // for multi-user setups.
    let mut processes = processes
        .filter(|(_, process)| process.user_id() == this_process_uid || this_process_uid.is_none());

    // Finally, filter away current process.
    Ok(processes.any(|(pid, _)| current_pid != *pid))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_does_not_panic() {
        let _ = other_cargo_apple_runner_processes().unwrap();
    }
}
